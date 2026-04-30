use crate::api::error::ApiError;
use crate::db::repos::{User, UserRepo};
use crate::service::AppState;
use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use rand::{rngs::OsRng, RngCore};
use sqlx::Row;
use tower_cookies::{Cookie, Cookies};

pub const COOKIE_NAME: &str = "pkv_admin_session";
const SESSION_TTL_SECONDS: i64 = 60 * 60 * 12;

#[derive(Debug, Clone)]
pub struct AdminSession {
    pub session_id: String,
    pub user: User,
}

pub fn generate_session_id() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("s_{}", hex::encode(bytes))
}

pub async fn create_session(state: &AppState, user_id: &str) -> Result<String, sqlx::Error> {
    let id = generate_session_id();
    let now = chrono::Utc::now().timestamp();
    let expires_at = now + SESSION_TTL_SECONDS;
    sqlx::query(
        "INSERT INTO admin_sessions (id, user_id, created_at, expires_at, last_seen_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(now)
    .bind(expires_at)
    .bind(now)
    .execute(&state.pool)
    .await?;
    Ok(id)
}

pub async fn delete_session(state: &AppState, id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM admin_sessions WHERE id = ?")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

pub async fn cleanup_expired_sessions(state: &AppState) -> Result<u64, sqlx::Error> {
    let now = chrono::Utc::now().timestamp();
    let deleted = sqlx::query("DELETE FROM admin_sessions WHERE expires_at < ?")
        .bind(now)
        .execute(&state.pool)
        .await?;
    Ok(deleted.rows_affected())
}

#[async_trait]
impl FromRequestParts<AppState> for AdminSession {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let cookies = Cookies::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::unauthorized("missing cookies"))?;
        let session_id = cookies
            .get(COOKIE_NAME)
            .map(|c| c.value().to_string())
            .ok_or_else(|| ApiError::unauthorized("missing admin session"))?;

        let now = chrono::Utc::now().timestamp();
        let row = sqlx::query("SELECT user_id, expires_at FROM admin_sessions WHERE id = ?")
            .bind(&session_id)
            .fetch_optional(&state.pool)
            .await?;
        let Some(row) = row else {
            return Err(ApiError::unauthorized("invalid session"));
        };
        let user_id: String = row.get("user_id");
        let expires_at: i64 = row.get("expires_at");
        if expires_at <= now {
            let _ = delete_session(state, &session_id).await;
            return Err(ApiError::unauthorized("session expired"));
        }

        let user = state
            .users
            .find_by_id(&user_id)
            .await?
            .ok_or_else(|| ApiError::unauthorized("user missing"))?;
        if !user.is_active {
            return Err(ApiError::forbidden("account disabled"));
        }
        if !user.is_admin {
            return Err(ApiError::forbidden("admin required"));
        }

        sqlx::query("UPDATE admin_sessions SET last_seen_at = ? WHERE id = ?")
            .bind(now)
            .bind(&session_id)
            .execute(&state.pool)
            .await?;
        Ok(AdminSession { session_id, user })
    }
}

pub fn make_cookie(session_id: String, secure: bool) -> Cookie<'static> {
    let mut cookie = Cookie::new(COOKIE_NAME, session_id);
    cookie.set_http_only(true);
    cookie.set_secure(secure);
    cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
    cookie.set_path("/admin");
    cookie
}

pub fn expired_cookie(secure: bool) -> Cookie<'static> {
    let mut cookie = Cookie::new(COOKIE_NAME, "");
    cookie.set_secure(secure);
    cookie.set_path("/admin");
    cookie.make_removal();
    cookie
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::password;
    use crate::db::pool;
    use crate::db::repos::{NewUser, UserRepo};

    async fn state_with_admin() -> (AppState, String, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into())
            .await
            .unwrap();
        let admin = state
            .users
            .create(NewUser {
                username: "admin".into(),
                password_hash: password::hash("passw0rd!!").unwrap(),
                is_admin: true,
            })
            .await
            .unwrap();
        (state, admin.id, tmp)
    }

    #[tokio::test]
    async fn create_session_inserts_row() {
        let (state, user_id, _tmp) = state_with_admin().await;
        let session_id = create_session(&state, &user_id).await.unwrap();
        assert!(session_id.starts_with("s_"));
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM admin_sessions")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn delete_session_removes_row() {
        let (state, user_id, _tmp) = state_with_admin().await;
        let session_id = create_session(&state, &user_id).await.unwrap();
        delete_session(&state, &session_id).await.unwrap();
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM admin_sessions")
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn cookie_is_httponly_secure_lax_when_requested() {
        let cookie = make_cookie("s_x".into(), true);
        assert_eq!(cookie.http_only(), Some(true));
        assert_eq!(cookie.secure(), Some(true));
        assert_eq!(
            cookie.same_site(),
            Some(tower_cookies::cookie::SameSite::Lax)
        );
        assert_eq!(cookie.path(), Some("/admin"));
    }

    #[test]
    fn cookie_can_be_insecure_for_local_http() {
        let cookie = make_cookie("s_x".into(), false);
        assert_eq!(cookie.http_only(), Some(true));
        assert_eq!(cookie.secure(), Some(false));
        assert_eq!(cookie.path(), Some("/admin"));
    }
}
