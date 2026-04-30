use crate::api::error::ApiError;
use crate::auth::AdminUser;
use crate::service::{gc, AppState};
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/admin/system", get(system))
        .route("/api/admin/gc", post(run_gc))
}

#[derive(Serialize)]
struct SystemResp {
    users: i64,
    vaults: i64,
}

async fn system(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<SystemResp>, ApiError> {
    let (users,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await?;
    let (vaults,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM vaults")
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(SystemResp { users, vaults }))
}

async fn run_gc(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<gc::GcReport>, ApiError> {
    Ok(Json(gc::run_blob_gc(&state).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{password, token};
    use crate::db::pool;
    use crate::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
    use crate::service::AppState;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use tower::ServiceExt;

    async fn setup() -> (Router, String) {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into())
            .await
            .unwrap();
        let h = password::hash("passw0rd!!").unwrap();
        let admin = state
            .users
            .create(NewUser {
                username: "admin".into(),
                password_hash: h,
                is_admin: true,
            })
            .await
            .unwrap();
        let raw = token::generate();
        state
            .tokens
            .create(NewToken {
                user_id: &admin.id,
                token_hash: &token::hash(&raw),
                device_name: "x",
            })
            .await
            .unwrap();
        (router().with_state(state), raw)
    }

    fn auth_request(method: &str, uri: &str, raw: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {raw}"))
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn admin_can_read_system_and_run_gc() {
        let (app, raw) = setup().await;
        let system = app
            .clone()
            .oneshot(auth_request("GET", "/api/admin/system", &raw))
            .await
            .unwrap();
        assert_eq!(system.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(system.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(body["users"], 1);

        let gc = app
            .oneshot(auth_request("POST", "/api/admin/gc", &raw))
            .await
            .unwrap();
        assert_eq!(gc.status(), StatusCode::OK);
    }
}
