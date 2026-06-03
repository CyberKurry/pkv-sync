use crate::admin::session;
use crate::api::error::ApiError;
use crate::auth::{password, AdminUser};
use crate::db::repos::{NewUser, TokenRepo, TokenRow, User, UserRepo, VaultRepo};
use crate::service::auth::validate_username;
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
    validate_username(&req.username)?;
    if state.users.find_by_username(&req.username).await?.is_some() {
        return Err(ApiError::conflict("username_taken", "username exists"));
    }
    let password_hash = password::hash(&req.password).map_err(|e| match e {
        password::PasswordError::TooShort { .. } | password::PasswordError::TooLong { .. } => {
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
        if !state
            .users
            .set_admin_preserving_last_admin(&id, is_admin)
            .await?
        {
            return Err(ApiError::bad_request(
                "last_admin",
                "cannot demote the last admin",
            ));
        }
    }
    if let Some(password) = req.password {
        let password_hash = password::hash(&password).map_err(|e| match e {
            password::PasswordError::TooShort { .. } | password::PasswordError::TooLong { .. } => {
                ApiError::bad_request("weak_password", e.to_string())
            }
            _ => ApiError::internal(e.to_string()),
        })?;
        state.users.update_password(&id, &password_hash).await?;
        state
            .tokens
            .revoke_all_for_user(&id, chrono::Utc::now().timestamp(), None)
            .await?;
        session::delete_sessions_for_user(&state, &id).await?;
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
    let vaults = state.vaults.list_for_user(&id).await?;
    for vault in vaults {
        crate::service::vault::delete_vault_for_user(&state, &id, &vault.id).await?;
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
    Path((user_id, token_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let tokens = state.tokens.list_for_user(&user_id).await?;
    if !tokens.iter().any(|token| token.id == token_id) {
        return Err(ApiError::not_found("token not found"));
    }
    state
        .tokens
        .revoke(&token_id, chrono::Utc::now().timestamp())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use crate::admin::session;
    use crate::auth::{password, token};
    use crate::db::pool;
    use crate::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
    use crate::service::{vault, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use tower::ServiceExt;

    async fn setup() -> (Router, String) {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into(), true)
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
                device_id: "device-admin-users",
                device_name: "x",
            })
            .await
            .unwrap();
        (super::router().with_state(state), raw)
    }

    async fn setup_with_second_user_state() -> (Router, AppState, String, String, String, String) {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "t".into(), true)
            .await
            .unwrap();
        let h = password::hash("passw0rd!!").unwrap();
        let admin = state
            .users
            .create(NewUser {
                username: "admin".into(),
                password_hash: h.clone(),
                is_admin: true,
            })
            .await
            .unwrap();
        let other = state
            .users
            .create(NewUser {
                username: "other".into(),
                password_hash: h,
                is_admin: false,
            })
            .await
            .unwrap();
        let raw = token::generate();
        let admin_token = state
            .tokens
            .create(NewToken {
                user_id: &admin.id,
                token_hash: &token::hash(&raw),
                device_id: "device-admin-users",
                device_name: "x",
            })
            .await
            .unwrap();
        let other_token = state
            .tokens
            .create(NewToken {
                user_id: &other.id,
                token_hash: &token::hash(&token::generate()),
                device_id: "device-other",
                device_name: "other",
            })
            .await
            .unwrap();
        (
            super::router().with_state(state.clone()),
            state,
            raw,
            other.id,
            admin_token.id,
            other_token.id,
        )
    }

    async fn setup_with_second_user() -> (Router, String, String, String, String) {
        let (app, _state, raw, other_id, admin_token_id, other_token_id) =
            setup_with_second_user_state().await;
        (app, raw, other_id, admin_token_id, other_token_id)
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
    async fn admin_create_user_rejects_invalid_username() {
        let (app, raw) = setup().await;
        let resp = app
            .oneshot(req_json(
                "POST",
                "/api/admin/users",
                &raw,
                serde_json::json!({"username":"","password":"passw0rd!!"}),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn admin_token_revoke_requires_token_to_belong_to_path_user() {
        let (app, raw, other_id, admin_token_id, _other_token_id) = setup_with_second_user().await;
        let resp = app
            .clone()
            .oneshot(auth_request(
                "DELETE",
                format!("/api/admin/users/{other_id}/tokens/{admin_token_id}"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let still_authenticated = app
            .oneshot(auth_request("GET", "/api/admin/users", &raw))
            .await
            .unwrap();
        assert_eq!(still_authenticated.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn admin_can_revoke_another_users_token() {
        let (app, raw, other_id, _admin_token_id, other_token_id) = setup_with_second_user().await;
        let resp = app
            .oneshot(auth_request(
                "DELETE",
                format!("/api/admin/users/{other_id}/tokens/{other_token_id}"),
                &raw,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn admin_password_reset_deletes_target_admin_sessions() {
        let (app, state, raw, other_id, _admin_token_id, _other_token_id) =
            setup_with_second_user_state().await;
        state.users.set_admin(&other_id, true).await.unwrap();
        session::create_session(&state, &other_id).await.unwrap();

        let resp = app
            .oneshot(req_json(
                "PATCH",
                &format!("/api/admin/users/{other_id}"),
                &raw,
                serde_json::json!({"password":"newpassw0rd!!"}),
            ))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        let (remaining_sessions,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM admin_sessions WHERE user_id = ?")
                .bind(&other_id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(remaining_sessions, 0);
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
    async fn admin_delete_user_removes_vault_storage() {
        let (app, state, raw, other_id, _admin_token_id, _other_token_id) =
            setup_with_second_user_state().await;
        let vault = vault::create_vault(&state, &other_id, "main")
            .await
            .unwrap();
        let repo_dir = state.default_vault_root().join(&vault.id);
        tokio::fs::create_dir_all(&repo_dir).await.unwrap();
        tokio::fs::write(repo_dir.join("HEAD"), b"ref: main")
            .await
            .unwrap();

        let resp = app
            .oneshot(auth_request(
                "DELETE",
                format!("/api/admin/users/{other_id}"),
                &raw,
            ))
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        assert!(!tokio::fs::try_exists(&repo_dir).await.unwrap());
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
