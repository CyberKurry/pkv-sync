use crate::api::error::ApiError;
use crate::auth::{password, AdminUser};
use crate::db::repos::{NewUser, TokenRepo, TokenRow, User, UserRepo};
use crate::service::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, patch};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/admin/users", get(list).post(create))
        .route("/api/admin/users/:id", patch(update).delete(remove))
        .route("/api/admin/users/:id/tokens", get(list_user_tokens))
        .route(
            "/api/admin/users/:id/tokens/:tid",
            delete(revoke_user_token),
        )
}

#[derive(Deserialize)]
struct CreateReq {
    username: String,
    password: String,
    is_admin: Option<bool>,
}

#[derive(Deserialize)]
struct PatchReq {
    is_active: Option<bool>,
    is_admin: Option<bool>,
    password: Option<String>,
}

#[derive(Serialize)]
struct UserView {
    id: String,
    username: String,
    is_admin: bool,
    is_active: bool,
    created_at: i64,
    last_login_at: Option<i64>,
}

impl From<User> for UserView {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            is_admin: u.is_admin,
            is_active: u.is_active,
            created_at: u.created_at,
            last_login_at: u.last_login_at,
        }
    }
}

async fn list(
    _admin: AdminUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<UserView>>, ApiError> {
    let users = state.users.list().await?;
    Ok(Json(users.into_iter().map(UserView::from).collect()))
}

async fn create(
    _admin: AdminUser,
    State(state): State<AppState>,
    Json(req): Json<CreateReq>,
) -> Result<(StatusCode, Json<UserView>), ApiError> {
    if state.users.find_by_username(&req.username).await?.is_some() {
        return Err(ApiError::conflict("username_taken", "username exists"));
    }
    let password_hash = password::hash(&req.password).map_err(|e| match e {
        password::PasswordError::TooShort { .. } => {
            ApiError::bad_request("weak_password", e.to_string())
        }
        _ => ApiError::internal(e.to_string()),
    })?;
    let user = state
        .users
        .create(NewUser {
            username: req.username,
            password_hash,
            is_admin: req.is_admin.unwrap_or(false),
        })
        .await?;
    Ok((StatusCode::CREATED, Json(UserView::from(user))))
}

async fn update(
    admin: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PatchReq>,
) -> Result<StatusCode, ApiError> {
    state
        .users
        .find_by_id(&id)
        .await?
        .ok_or_else(|| ApiError::not_found("user not found"))?;

    if admin.0.user_id == id {
        if let Some(false) = req.is_admin {
            if state.users.count_admins().await? <= 1 {
                return Err(ApiError::bad_request(
                    "last_admin",
                    "cannot demote the last admin",
                ));
            }
        }
        if let Some(false) = req.is_active {
            return Err(ApiError::bad_request("self_disable", "cannot disable self"));
        }
    }

    if let Some(active) = req.is_active {
        state.users.set_active(&id, active).await?;
    }
    if let Some(is_admin) = req.is_admin {
        state.users.set_admin(&id, is_admin).await?;
    }
    if let Some(password) = req.password {
        let password_hash = password::hash(&password).map_err(|e| match e {
            password::PasswordError::TooShort { .. } => {
                ApiError::bad_request("weak_password", e.to_string())
            }
            _ => ApiError::internal(e.to_string()),
        })?;
        state.users.update_password(&id, &password_hash).await?;
        state
            .tokens
            .revoke_all_for_user(&id, chrono::Utc::now().timestamp(), None)
            .await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn remove(
    admin: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    if admin.0.user_id == id {
        return Err(ApiError::bad_request("self_delete", "cannot delete self"));
    }
    if state.users.find_by_id(&id).await?.is_none() {
        return Err(ApiError::not_found("user not found"));
    }
    state.users.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_user_tokens(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<TokenRow>>, ApiError> {
    let tokens = state.tokens.list_for_user(&id).await?;
    Ok(Json(tokens))
}

async fn revoke_user_token(
    _admin: AdminUser,
    State(state): State<AppState>,
    Path((_user_id, token_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    state
        .tokens
        .revoke(&token_id, chrono::Utc::now().timestamp())
        .await?;
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
                device_name: "x",
            })
            .await
            .unwrap();
        (super::router().with_state(state), raw)
    }

    fn auth_request(method: &str, uri: impl Into<String>, raw: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri.into())
            .header("authorization", format!("Bearer {raw}"))
            .body(Body::empty())
            .unwrap()
    }

    fn req_json(method: &str, uri: &str, raw: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("authorization", format!("Bearer {raw}"))
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    async fn first_user_id(app: Router, raw: &str) -> String {
        let resp = app
            .oneshot(auth_request("GET", "/api/admin/users", raw))
            .await
            .unwrap();
        let body: serde_json::Value =
            serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 4096).await.unwrap())
                .unwrap();
        body[0]["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn admin_can_create_user() {
        let (app, raw) = setup().await;
        let resp = app
            .oneshot(req_json(
                "POST",
                "/api/admin/users",
                &raw,
                serde_json::json!({"username":"bob","password":"passw0rd!!"}),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn admin_cannot_self_delete() {
        let (app, raw) = setup().await;
        let admin_id = first_user_id(app.clone(), &raw).await;
        let resp = app
            .oneshot(auth_request(
                "DELETE",
                format!("/api/admin/users/{admin_id}"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn admin_cannot_demote_last_admin() {
        let (app, raw) = setup().await;
        let admin_id = first_user_id(app.clone(), &raw).await;
        let resp = app
            .oneshot(req_json(
                "PATCH",
                &format!("/api/admin/users/{admin_id}"),
                &raw,
                serde_json::json!({"is_admin": false}),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
