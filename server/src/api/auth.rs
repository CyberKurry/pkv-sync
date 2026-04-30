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
    if let Err(remaining) = limiter.check(ip) {
        return Err(ApiError::too_many(format!(
            "locked for {}s",
            remaining.as_secs()
        )));
    }
    match login(&state, req).await {
        Ok(resp) => {
            limiter.record_success(ip);
            Ok(Json(resp))
        }
        Err(e) if e.status == axum::http::StatusCode::UNAUTHORIZED => {
            limiter.record_failure(ip);
            Err(e)
        }
        Err(e) => Err(e),
    }
}

async fn register_handler(
    State(state): State<AppState>,
    Json(req): Json<RegisterReq>,
) -> Result<impl IntoResponse, ApiError> {
    let resp = register(&state, req).await?;
    Ok((axum::http::StatusCode::CREATED, Json(resp)))
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

    async fn make_app(mode: RegistrationMode) -> Router {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into())
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
            rcfg.login_failure_threshold,
            Duration::from_secs(rcfg.login_window_seconds),
            Duration::from_secs(rcfg.login_lock_seconds),
        );
        router()
            .with_state(state)
            .layer(Extension(limiter))
            .layer(Extension(ClientIp("127.0.0.1".parse().unwrap())))
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
                    "username":"alice","password":"passw0rd!!","device_name":"d"
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
                    "username":"alice","password":"passw0rd!!","device_name":"d"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn login_returns_200_with_token() {
        let app = make_app(RegistrationMode::Open).await;
        let _ = app
            .clone()
            .oneshot(json(
                "/api/auth/register",
                serde_json::json!({
                    "username":"alice","password":"passw0rd!!","device_name":"d"
                }),
            ))
            .await
            .unwrap();
        let resp = app
            .oneshot(json(
                "/api/auth/login",
                serde_json::json!({
                    "username":"alice","password":"passw0rd!!","device_name":"d2"
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
                    "username":"alice","password":"passw0rd!!","device_name":"d"
                }),
            ))
            .await
            .unwrap();
        let resp = app
            .oneshot(json(
                "/api/auth/login",
                serde_json::json!({
                    "username":"alice","password":"wrong","device_name":"d"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
