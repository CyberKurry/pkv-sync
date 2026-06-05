use crate::api::error::ApiError;
use crate::auth::LoginRateLimiter;
use crate::middleware::real_ip::ClientIp;
use crate::service::auth::{login, register, AuthResp, LoginReq, RegisterReq};
use crate::service::AppState;
use axum::extract::{Extension, State};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/auth/login", post(login_handler))
        .route("/api/auth/register", post(register_handler))
}

async fn login_handler(
    State(state): State<AppState>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    Extension(limiter): Extension<LoginRateLimiter>,
    Json(req): Json<LoginReq>,
) -> Result<Json<AuthResp>, ApiError> {
    // Reserve an attempt slot atomically. try_acquire counts in-flight
    // reservations toward the threshold, so a burst of concurrent guesses
    // is rejected before any of them reaches argon2 verification.
    let reservation = match limiter.try_acquire(ip) {
        Ok(r) => r,
        Err(remaining) => {
            return Err(ApiError::too_many(format!(
                "locked for {}s",
                remaining.as_secs()
            )));
        }
    };
    match login(&state, req).await {
        Ok(resp) => {
            reservation.success();
            Ok(Json(resp))
        }
        Err(e) if e.status == axum::http::StatusCode::UNAUTHORIZED => {
            reservation.failure();
            Err(e)
        }
        Err(e) => {
            reservation.release();
            Err(e)
        }
    }
}

async fn register_handler(
    State(state): State<AppState>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
    Extension(limiter): Extension<LoginRateLimiter>,
    Json(req): Json<RegisterReq>,
) -> Result<impl IntoResponse, ApiError> {
    let reservation = match limiter.try_acquire(ip) {
        Ok(r) => r,
        Err(remaining) => {
            return Err(ApiError::too_many(format!(
                "locked for {}s",
                remaining.as_secs()
            )));
        }
    };
    match register(&state, req).await {
        Ok(resp) => {
            reservation.release();
            Ok((axum::http::StatusCode::CREATED, Json(resp)))
        }
        Err(e) if register_failure_consumes_budget(&e) => {
            reservation.failure();
            Err(e)
        }
        Err(e) => {
            reservation.release();
            Err(e)
        }
    }
}

/// Decide whether a register() failure should consume the login rate-limit
/// budget. We distinguish abuse signals from honest client-side typos so a
/// user mistyping a username does not get locked out (preserving the v0.1.12
/// fix), while attackers cannot freely probe invite codes or registration
/// mode.
fn register_failure_consumes_budget(err: &ApiError) -> bool {
    use axum::http::StatusCode;
    // Server-side mode probing or username enumeration → always counts.
    if matches!(
        err.status,
        StatusCode::FORBIDDEN | StatusCode::CONFLICT | StatusCode::UNAUTHORIZED
    ) {
        return true;
    }
    // BAD_REQUEST is dual-use: validation typos vs invite brute force.
    // Match on the error code to count only the abuse signals.
    if err.status == StatusCode::BAD_REQUEST {
        return matches!(
            err.code.as_str(),
            "invite_required" | "invalid_invite" | "registration_disabled"
        );
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::LoginRateLimiter;
    use crate::db::pool;
    use crate::db::repos::{RegistrationMode, RuntimeConfigRepo};
    use crate::middleware::real_ip::ClientIp;
    use crate::service::AppState;
    use axum::body::Body;
    use axum::extract::Extension;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use std::time::Duration;
    use tower::ServiceExt;

    async fn make_app_with_threshold(mode: RegistrationMode, threshold: u32) -> Router {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap();
        state
            .runtime_cfg_repo
            .set_registration_mode(mode, None)
            .await
            .unwrap();
        let cfg = state.runtime_cfg_repo.load().await.unwrap();
        state.runtime_cfg.replace(cfg).await;
        let rcfg = state.runtime_cfg.snapshot().await;
        let limiter = LoginRateLimiter::new(
            threshold,
            Duration::from_secs(rcfg.login_window_seconds),
            Duration::from_secs(rcfg.login_lock_seconds),
        );
        router()
            .with_state(state)
            .layer(Extension(limiter))
            .layer(Extension(ClientIp("127.0.0.1".parse().unwrap())))
    }

    async fn make_app(mode: RegistrationMode) -> Router {
        make_app_with_threshold(mode, 10).await
    }

    fn json(uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    #[tokio::test]
    async fn register_open_returns_201() {
        let app = make_app(RegistrationMode::Open).await;
        let resp = app
            .oneshot(json(
                "/api/auth/register",
                serde_json::json!({
                    "username":"alice","password":"Passw0rdStrong","device_id":"device-register","device_name":"d"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 4096).await.unwrap())
                .unwrap();
        assert!(body["token"].as_str().unwrap().starts_with("pks_"));
    }

    #[tokio::test]
    async fn register_disabled_returns_403() {
        let app = make_app(RegistrationMode::Disabled).await;
        let resp = app
            .oneshot(json(
                "/api/auth/register",
                serde_json::json!({
                    "username":"alice","password":"passw0rd!!","device_id":"device-disabled","device_name":"d"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn register_validation_failures_do_not_consume_login_limiter() {
        // Username "ab" fails validation with code "username_too_short" (or
        // similar typo-class code). Per the dual-use BAD_REQUEST classification
        // in register_failure_consumes_budget, typos must NOT consume the
        // limiter, so an honest user mistyping their username can still
        // recover and register correctly afterwards.
        let app = make_app(RegistrationMode::Open).await;
        for _ in 0..10 {
            let resp = app
                .clone()
                .oneshot(json(
                    "/api/auth/register",
                    serde_json::json!({
                        "username":"ab","password":"passw0rd!!","device_id":"device-bad-register","device_name":"d"
                    }),
                ))
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        }

        let resp = app
            .oneshot(json(
                "/api/auth/register",
                serde_json::json!({
                    "username":"alice","password":"Passw0rdStrong","device_id":"device-register","device_name":"d"
                }),
            ))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    /// Regression: invite-required failures and
    /// registration-disabled failures are abuse signals (invite enumeration,
    /// mode probing) and must consume rate-limit budget so the endpoint can
    /// be locked under sustained attack.
    #[tokio::test]
    async fn register_invite_required_failures_consume_limiter() {
        let app = make_app(RegistrationMode::InviteOnly).await;
        let body = serde_json::json!({
            "username": "alice",
            "password": "passw0rd!!",
            "device_id": "device-no-invite",
            "device_name": "d"
        });

        // Default login_failure_threshold is 10; after exceeding it, the
        // limiter should respond with 429 (TooMany) on subsequent requests.
        let mut bad_request_count = 0;
        let mut too_many_count = 0;
        for _ in 0..20 {
            let resp = app
                .clone()
                .oneshot(json("/api/auth/register", body.clone()))
                .await
                .unwrap();
            match resp.status() {
                StatusCode::BAD_REQUEST => bad_request_count += 1,
                StatusCode::TOO_MANY_REQUESTS => too_many_count += 1,
                other => panic!("unexpected status {other:?}"),
            }
        }
        assert!(
            bad_request_count > 0 && too_many_count > 0,
            "expected the limiter to eventually trip; bad={bad_request_count} too_many={too_many_count}"
        );
    }

    /// Regression: registration-disabled mode is also
    /// an abuse-probing signal and must consume rate-limit budget.
    #[tokio::test]
    async fn register_disabled_mode_failures_consume_limiter() {
        let app = make_app(RegistrationMode::Disabled).await;
        let body = serde_json::json!({
            "username": "alice",
            "password": "passw0rd!!",
            "device_id": "device-disabled-probe",
            "device_name": "d"
        });
        let mut forbidden_count = 0;
        let mut too_many_count = 0;
        for _ in 0..20 {
            let resp = app
                .clone()
                .oneshot(json("/api/auth/register", body.clone()))
                .await
                .unwrap();
            match resp.status() {
                StatusCode::FORBIDDEN => forbidden_count += 1,
                StatusCode::TOO_MANY_REQUESTS => too_many_count += 1,
                other => panic!("unexpected status {other:?}"),
            }
        }
        assert!(
            forbidden_count > 0 && too_many_count > 0,
            "expected the limiter to eventually trip; forbidden={forbidden_count} too_many={too_many_count}"
        );
    }

    #[tokio::test]
    async fn register_success_does_not_reset_abuse_budget() {
        let app = make_app_with_threshold(RegistrationMode::Open, 2).await;
        let taken = serde_json::json!({
            "username": "taken",
            "password": "Passw0rdStrong",
            "device_id": "device-taken-0",
            "device_name": "d"
        });
        let success = serde_json::json!({
            "username": "alice",
            "password": "Passw0rdStrong",
            "device_id": "device-alice",
            "device_name": "d"
        });
        let conflict1 = serde_json::json!({
            "username": "taken",
            "password": "passw0rd!!",
            "device_id": "device-taken-1",
            "device_name": "d"
        });
        let conflict2 = serde_json::json!({
            "username": "taken",
            "password": "passw0rd!!",
            "device_id": "device-taken-2",
            "device_name": "d"
        });
        let conflict3 = serde_json::json!({
            "username": "taken",
            "password": "passw0rd!!",
            "device_id": "device-taken-3",
            "device_name": "d"
        });

        let created = app
            .clone()
            .oneshot(json("/api/auth/register", taken))
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);

        let first_conflict = app
            .clone()
            .oneshot(json("/api/auth/register", conflict1))
            .await
            .unwrap();
        assert_eq!(first_conflict.status(), StatusCode::CONFLICT);

        let inserted_success = app
            .clone()
            .oneshot(json("/api/auth/register", success))
            .await
            .unwrap();
        assert_eq!(inserted_success.status(), StatusCode::CREATED);

        let second_conflict = app
            .clone()
            .oneshot(json("/api/auth/register", conflict2))
            .await
            .unwrap();
        assert_eq!(second_conflict.status(), StatusCode::CONFLICT);

        let third_conflict = app
            .oneshot(json("/api/auth/register", conflict3))
            .await
            .unwrap();
        assert_eq!(third_conflict.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn login_returns_200_with_token() {
        let app = make_app(RegistrationMode::Open).await;
        let _ = app
            .clone()
            .oneshot(json(
                "/api/auth/register",
                serde_json::json!({
                    "username":"alice","password":"Passw0rdStrong","device_id":"device-login-register","device_name":"d"
                }),
            ))
            .await
            .unwrap();
        let resp = app
            .oneshot(json(
                "/api/auth/login",
                serde_json::json!({
                    "username":"alice","password":"Passw0rdStrong","device_id":"device-login","device_name":"d2"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn login_wrong_password_401() {
        let app = make_app(RegistrationMode::Open).await;
        let _ = app
            .clone()
            .oneshot(json(
                "/api/auth/register",
                serde_json::json!({
                    "username":"alice","password":"Passw0rdStrong","device_id":"device-wrong-register","device_name":"d"
                }),
            ))
            .await
            .unwrap();
        let resp = app
            .oneshot(json(
                "/api/auth/login",
                serde_json::json!({
                    "username":"alice","password":"wrong","device_id":"device-wrong","device_name":"d"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    /// Regression: a disabled account hitting login
    /// must return UNAUTHORIZED (NOT FORBIDDEN) — same status as wrong
    /// password — so the HTTP status cannot be used to enumerate which
    /// usernames correspond to disabled accounts. Login handler's existing
    /// 401-only record_failure path therefore automatically charges these
    /// attempts to the rate-limit budget, closing the bypass where 403
    /// responses skipped record_failure.
    #[tokio::test]
    async fn login_disabled_account_returns_401_and_consumes_limiter() {
        use crate::db::repos::UserRepo;
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap();
        state
            .runtime_cfg_repo
            .set_registration_mode(RegistrationMode::Open, None)
            .await
            .unwrap();
        let cfg = state.runtime_cfg_repo.load().await.unwrap();
        state.runtime_cfg.replace(cfg.clone()).await;
        let limiter = LoginRateLimiter::new(
            cfg.login_failure_threshold,
            Duration::from_secs(cfg.login_window_seconds),
            Duration::from_secs(cfg.login_lock_seconds),
        );
        let app = router()
            .with_state(state.clone())
            .layer(Extension(limiter))
            .layer(Extension(ClientIp("127.0.0.1".parse().unwrap())));

        // Create an account, then disable it.
        let create_resp = app
            .clone()
            .oneshot(json(
                "/api/auth/register",
                serde_json::json!({
                    "username":"disabled_user","password":"Passw0rdStrong","device_id":"device-disabled","device_name":"d"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::CREATED);
        let user = state
            .users
            .find_by_username("disabled_user")
            .await
            .unwrap()
            .unwrap();
        state.users.set_active(&user.id, false).await.unwrap();

        // First attempt: 401, not 403.
        let resp = app
            .clone()
            .oneshot(json(
                "/api/auth/login",
                serde_json::json!({
                    "username":"disabled_user","password":"Passw0rdStrong","device_id":"device-disabled","device_name":"d"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "disabled account must look identical to wrong-password (no enumeration)"
        );

        // Repeated attempts must consume the limiter and eventually 429.
        let mut unauthorized = 0;
        let mut too_many = 0;
        for _ in 0..30 {
            let resp = app
                .clone()
                .oneshot(json(
                    "/api/auth/login",
                    serde_json::json!({
                        "username":"disabled_user","password":"Passw0rdStrong","device_id":"device-disabled","device_name":"d"
                    }),
                ))
                .await
                .unwrap();
            match resp.status() {
                StatusCode::UNAUTHORIZED => unauthorized += 1,
                StatusCode::TOO_MANY_REQUESTS => too_many += 1,
                other => panic!("unexpected status {other:?}"),
            }
        }
        assert!(
            too_many > 0,
            "disabled-account login attempts must trip rate limit; unauthorized={unauthorized} too_many={too_many}"
        );
    }
}
