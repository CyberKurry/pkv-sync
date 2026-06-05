use crate::api::error::ApiError;
use crate::auth::AuthenticatedUser;
use crate::db::repos::{TokenRepo, Vault, VaultRepo};
use crate::middleware::rate_limit;
use crate::service::auth::{change_password, ChangePasswordReq};
use crate::service::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Serialize;

pub fn router() -> Router<AppState> {
    router_with_rate_limiters(
        rate_limit::RequestRateLimiter::api_auth(),
        rate_limit::RequestRateLimiter::password_change(),
    )
}

fn router_with_rate_limiters(
    api_auth_limiter: rate_limit::RequestRateLimiter,
    password_limiter: rate_limit::RequestRateLimiter,
) -> Router<AppState> {
    let account_routes = Router::new()
        .route("/api/me", get(me))
        .route("/api/me/logout", post(logout))
        .route("/api/me/tokens", get(list_tokens))
        .route("/api/me/tokens/:id", delete(revoke_token))
        .route_layer(axum::middleware::from_fn_with_state(
            api_auth_limiter.clone(),
            rate_limit::api_auth_middleware,
        ));
    let password_route = Router::new()
        .route("/api/me/password", post(change_password_handler))
        .route_layer(axum::middleware::from_fn_with_state(
            password_limiter,
            rate_limit::password_change_middleware,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            api_auth_limiter,
            rate_limit::api_auth_middleware,
        ));

    account_routes.merge(password_route)
}

#[derive(Serialize)]
struct MeResp {
    user_id: String,
    username: String,
    is_admin: bool,
    vaults: Vec<Vault>,
}

async fn me(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<MeResp>, ApiError> {
    let vaults = state.vaults.list_for_user(&user.user_id).await?;
    Ok(Json(MeResp {
        user_id: user.user_id,
        username: user.username,
        is_admin: user.is_admin,
        vaults,
    }))
}

async fn change_password_handler(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(req): Json<ChangePasswordReq>,
) -> Result<StatusCode, ApiError> {
    change_password(&state, &user.user_id, &user.token_id, req).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn logout(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<StatusCode, ApiError> {
    state
        .tokens
        .revoke(&user.token_id, chrono::Utc::now().timestamp())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
struct TokenView {
    id: String,
    device_id: String,
    device_name: String,
    created_at: i64,
    last_used_at: Option<i64>,
    current: bool,
}

async fn list_tokens(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Result<Json<Vec<TokenView>>, ApiError> {
    let rows = state.tokens.list_active_for_user(&user.user_id).await?;
    let tokens = rows
        .into_iter()
        .map(|r| TokenView {
            current: r.id == user.token_id,
            id: r.id,
            device_id: r.device_id,
            device_name: r.device_name,
            created_at: r.created_at,
            last_used_at: r.last_used_at,
        })
        .collect();
    Ok(Json(tokens))
}

async fn revoke_token(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let rows = state.tokens.list_for_user(&user.user_id).await?;
    if !rows.iter().any(|r| r.id == id) {
        return Err(ApiError::not_found("token not found"));
    }
    state
        .tokens
        .revoke(&id, chrono::Utc::now().timestamp())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::token;
    use crate::db::pool;
    use crate::db::repos::{NewToken, NewUser, TokenRepo, UserRepo, VaultRepo};
    use crate::service::AppState;
    use axum::body::Body;
    use axum::http::{header, Request};
    use tower::ServiceExt;

    async fn setup() -> (Router, String, String) {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into(), true)
            .await
            .unwrap();
        let h = crate::auth::password::hash("passw0rd!!").unwrap();
        let user = state
            .users
            .create(NewUser {
                username: "alice".into(),
                password_hash: h,
                is_admin: false,
            })
            .await
            .unwrap();
        let raw = token::generate();
        state
            .tokens
            .create(NewToken {
                user_id: &user.id,
                token_hash: &token::hash(&raw),
                device_id: "device-me",
                device_name: "d",
            })
            .await
            .unwrap();
        state.vaults.create(&user.id, "main").await.unwrap();
        (router().with_state(state), user.id, raw)
    }

    fn auth_get(uri: &str, raw: &str) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .header("authorization", format!("Bearer {raw}"))
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn me_returns_user_info() {
        let (app, _uid, raw) = setup().await;
        let resp = app.oneshot(auth_get("/api/me", &raw)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 4096).await.unwrap())
                .unwrap();
        assert_eq!(body["username"], "alice");
        assert_eq!(body["vaults"].as_array().unwrap().len(), 1);
        assert_eq!(body["vaults"][0]["name"], "main");
    }

    #[tokio::test]
    async fn list_tokens_marks_current() {
        let (app, _uid, raw) = setup().await;
        let resp = app.oneshot(auth_get("/api/me/tokens", &raw)).await.unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 4096).await.unwrap())
                .unwrap();
        assert_eq!(body.as_array().unwrap().len(), 1);
        assert_eq!(body[0]["current"], true);
    }

    #[tokio::test]
    async fn logout_revokes_token() {
        let (app, _uid, raw) = setup().await;
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/me/logout")
                    .header("authorization", format!("Bearer {raw}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        let resp2 = app.oneshot(auth_get("/api/me", &raw)).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn me_rejects_rotating_invalid_bearer_attempts_with_rate_limit() {
        let (app, _uid, _raw) = setup().await;
        let mut saw_rate_limit = false;

        for idx in 0..130 {
            let fake = format!("pks_{idx:064x}");
            let resp = app
                .clone()
                .oneshot(auth_get("/api/me", &fake))
                .await
                .unwrap();
            if resp.status() == StatusCode::TOO_MANY_REQUESTS {
                saw_rate_limit = true;
                break;
            }
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        }

        assert!(
            saw_rate_limit,
            "rotating invalid bearer attempts were not rate limited"
        );
    }

    #[tokio::test]
    async fn change_password_wrong_current_password_is_rate_limited() {
        let (app, _uid, raw) = setup().await;
        let mut saw_rate_limit = false;

        for _ in 0..15 {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/me/password")
                        .header("authorization", format!("Bearer {raw}"))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            serde_json::json!({
                                "current_password": "wrong",
                                "new_password": "newpass1234"
                            })
                            .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            if resp.status() == StatusCode::TOO_MANY_REQUESTS {
                saw_rate_limit = true;
                break;
            }
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        }

        assert!(
            saw_rate_limit,
            "wrong current-password attempts were not rate limited"
        );
    }
}
