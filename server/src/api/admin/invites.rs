use crate::api::error::ApiError;
use crate::auth::AdminUser;
use crate::db::repos::{Invite, InviteRepo};
use crate::service::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use serde::Deserialize;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/admin/invites", get(list).post(create))
        .route("/api/admin/invites/:code", delete(remove))
}

#[derive(Deserialize)]
struct CreateReq {
    expires_at: Option<i64>,
}

async fn list(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<Invite>>, ApiError> {
    let invites = state
        .invites
        .list_active(chrono::Utc::now().timestamp())
        .await?;
    Ok(Json(invites))
}

async fn create(
    admin: AdminUser,
    State(state): State<AppState>,
    Json(req): Json<CreateReq>,
) -> Result<(StatusCode, Json<Invite>), ApiError> {
    let invite = state
        .invites
        .create(&admin.0.user_id, req.expires_at)
        .await?;
    Ok((StatusCode::CREATED, Json(invite)))
}

async fn remove(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<StatusCode, ApiError> {
    let invite = state
        .invites
        .find(&code)
        .await?
        .ok_or_else(|| ApiError::not_found("invite not found"))?;
    if invite.used_at.is_some() {
        return Err(ApiError::bad_request(
            "already_used",
            "cannot delete used invite",
        ));
    }
    state.invites.delete(&code).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
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
                device_name: "d",
            })
            .await
            .unwrap();
        (super::router().with_state(state), raw)
    }

    fn admin_req(method: &str, uri: &str, raw: &str, body: Body) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {raw}"))
            .header("content-type", "application/json")
            .body(body)
            .unwrap()
    }

    #[tokio::test]
    async fn admin_create_then_list() {
        let (app, raw) = setup().await;
        let resp = app
            .clone()
            .oneshot(admin_req(
                "POST",
                "/api/admin/invites",
                &raw,
                Body::from("{}"),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let resp = app
            .oneshot(admin_req("GET", "/api/admin/invites", &raw, Body::empty()))
            .await
            .unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 4096).await.unwrap())
                .unwrap();
        assert_eq!(body.as_array().unwrap().len(), 1);
    }
}
