use crate::middleware::rate_limit;
use crate::service::AppState;
use axum::Router;

pub mod invites;
pub mod system;
pub mod users;

pub fn router() -> Router<AppState> {
    router_with_rate_limiter(rate_limit::RequestRateLimiter::api_auth())
}

fn router_with_rate_limiter(limiter: rate_limit::RequestRateLimiter) -> Router<AppState> {
    Router::new()
        .merge(users::router())
        .merge(invites::router())
        .merge(system::router())
        .route_layer(axum::middleware::from_fn_with_state(
            limiter,
            rate_limit::api_auth_middleware,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::password;
    use crate::db::pool;
    use crate::db::repos::{NewUser, UserRepo};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::time::Duration;
    use tower::ServiceExt;

    async fn setup() -> Router {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into(), true)
            .await
            .unwrap();
        let password_hash = password::hash("passw0rd!!").unwrap();
        state
            .users
            .create(NewUser {
                username: "admin".into(),
                password_hash,
                is_admin: true,
            })
            .await
            .unwrap();
        router_with_rate_limiter(rate_limit::RequestRateLimiter::new(
            1,
            Duration::from_secs(60),
        ))
        .with_state(state)
    }

    fn invalid_admin_req(idx: u32) -> Request<Body> {
        Request::builder()
            .uri("/api/admin/users")
            .header("authorization", format!("Bearer pks_{idx:064x}"))
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn admin_routes_rate_limit_rotating_invalid_bearer_attempts() {
        let app = setup().await;
        let mut saw_rate_limit = false;

        for idx in 0..130 {
            let resp = app.clone().oneshot(invalid_admin_req(idx)).await.unwrap();
            if resp.status() == StatusCode::TOO_MANY_REQUESTS {
                saw_rate_limit = true;
                break;
            }
            assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        }

        assert!(
            saw_rate_limit,
            "rotating invalid admin bearer attempts were not rate limited"
        );
    }
}
