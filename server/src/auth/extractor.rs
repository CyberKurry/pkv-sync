use crate::api::error::ApiError;
use crate::auth::token;
use crate::db::repos::{TokenRepo, UserRepo};
use crate::service::AppState;
use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub username: String,
    pub is_admin: bool,
    pub token_id: String,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let raw = parts
            .headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or_else(|| ApiError::unauthorized("missing bearer token"))?;
        if !token::looks_valid(raw) {
            return Err(ApiError::unauthorized("invalid token format"));
        }
        let h = token::hash(raw);
        let (row, user_id) = state
            .tokens
            .find_by_hash(&h)
            .await?
            .ok_or_else(|| ApiError::unauthorized("invalid or revoked token"))?;
        let user = state
            .users
            .find_by_id(&user_id)
            .await?
            .ok_or_else(|| ApiError::unauthorized("user no longer exists"))?;
        if !user.is_active {
            return Err(ApiError::forbidden("account disabled"));
        }
        let _ = state
            .tokens
            .touch_used(&row.id, chrono::Utc::now().timestamp())
            .await;
        Ok(AuthenticatedUser {
            user_id: user.id,
            username: user.username,
            is_admin: user.is_admin,
            token_id: row.id,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AdminUser(pub AuthenticatedUser);

#[async_trait]
impl FromRequestParts<AppState> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let u = AuthenticatedUser::from_request_parts(parts, state).await?;
        if !u.is_admin {
            return Err(ApiError::forbidden("admin only"));
        }
        Ok(AdminUser(u))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::password;
    use crate::auth::token;
    use crate::db::pool;
    use crate::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
    use crate::service::AppState;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::get;
    use axum::Router;
    use tower::ServiceExt;

    async fn make_state() -> AppState {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        AppState::new(pool, tmp.path().to_path_buf(), "test".into())
            .await
            .unwrap()
    }

    async fn make_user_with_token(state: &AppState, is_admin: bool) -> (String, String) {
        let h = password::hash("password1234").unwrap();
        let u = state
            .users
            .create(NewUser {
                username: format!("u{}", uuid::Uuid::new_v4().simple()),
                password_hash: h,
                is_admin,
            })
            .await
            .unwrap();
        let raw = token::generate();
        state
            .tokens
            .create(NewToken {
                user_id: &u.id,
                token_hash: &token::hash(&raw),
                device_name: "d",
            })
            .await
            .unwrap();
        (u.id, raw)
    }

    fn router(state: AppState) -> Router {
        Router::new()
            .route(
                "/me",
                get(|user: AuthenticatedUser| async move { user.username }),
            )
            .route(
                "/admin",
                get(|admin: AdminUser| async move { admin.0.username }),
            )
            .with_state(state)
    }

    #[tokio::test]
    async fn rejects_no_header() {
        let state = make_state().await;
        let resp = router(state)
            .oneshot(Request::builder().uri("/me").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rejects_invalid_token() {
        let state = make_state().await;
        let resp = router(state)
            .oneshot(
                Request::builder()
                    .uri("/me")
                    .header("authorization", "Bearer garbage")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn accepts_valid_token() {
        let state = make_state().await;
        let (_, raw) = make_user_with_token(&state, false).await;
        let resp = router(state)
            .oneshot(
                Request::builder()
                    .uri("/me")
                    .header("authorization", format!("Bearer {raw}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn admin_route_rejects_non_admin() {
        let state = make_state().await;
        let (_, raw) = make_user_with_token(&state, false).await;
        let resp = router(state)
            .oneshot(
                Request::builder()
                    .uri("/admin")
                    .header("authorization", format!("Bearer {raw}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn admin_route_accepts_admin() {
        let state = make_state().await;
        let (_, raw) = make_user_with_token(&state, true).await;
        let resp = router(state)
            .oneshot(
                Request::builder()
                    .uri("/admin")
                    .header("authorization", format!("Bearer {raw}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_disabled_user() {
        let state = make_state().await;
        let (uid, raw) = make_user_with_token(&state, false).await;
        state.users.set_active(&uid, false).await.unwrap();
        let resp = router(state)
            .oneshot(
                Request::builder()
                    .uri("/me")
                    .header("authorization", format!("Bearer {raw}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
