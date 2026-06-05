use crate::api::error::ApiError;
use crate::db::repos::{User, UserRepo};
use crate::service::AppState;
use async_trait::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use rand::{rngs::OsRng, RngCore};
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

pub async fn delete_sessions_for_user(state: &AppState, user_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM admin_sessions WHERE user_id = ?")
        .bind(user_id)
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

#[derive(Debug, PartialEq, Eq)]
enum SessionRefresh {
    Active { user_id: String },
    Expired,
    Missing,
}

async fn refresh_session(
    state: &AppState,
    session_id: &str,
    now: i64,
) -> Result<SessionRefresh, sqlx::Error> {
    let active: Option<(String,)> = sqlx::query_as(
        "UPDATE admin_sessions
         SET last_seen_at = ?
         WHERE id = ? AND expires_at > ?
         RETURNING user_id",
    )
    .bind(now)
    .bind(session_id)
    .bind(now)
    .fetch_optional(&state.pool)
    .await?;
    if let Some((user_id,)) = active {
        return Ok(SessionRefresh::Active { user_id });
    }

    let deleted = sqlx::query("DELETE FROM admin_sessions WHERE id = ? AND expires_at <= ?")
        .bind(session_id)
        .bind(now)
        .execute(&state.pool)
        .await?;
    if deleted.rows_affected() > 0 {
        Ok(SessionRefresh::Expired)
    } else {
        Ok(SessionRefresh::Missing)
    }
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
        let session_cookie = cookies
            .get(COOKIE_NAME)
            .ok_or_else(|| ApiError::unauthorized("missing admin session"))?;
        let session_id = session_cookie.value();

        let now = chrono::Utc::now().timestamp();
        let user_id = match refresh_session(state, session_id, now).await? {
            SessionRefresh::Active { user_id } => user_id,
            SessionRefresh::Expired => {
                return Err(ApiError::unauthorized("session expired"));
            }
            SessionRefresh::Missing => {
                return Err(ApiError::unauthorized("invalid session"));
            }
        };

        let user = state
            .users
            .find_by_id(&user_id)
            .await?
            .ok_or_else(|| ApiError::unauthorized("user missing"))?;
        if !user.is_active {
            return Err(ApiError::unauthorized("invalid session"));
        }
        if !user.is_admin {
            return Err(ApiError::unauthorized("invalid session"));
        }

        Ok(AdminSession {
            session_id: session_id.to_string(),
            user,
        })
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
    cookie.set_http_only(true);
    cookie.set_secure(secure);
    cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
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
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
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

    #[tokio::test]
    async fn refresh_session_updates_active_session_and_returns_user_id() {
        let (state, user_id, _tmp) = state_with_admin().await;
        let session_id = create_session(&state, &user_id).await.unwrap();
        let refreshed_at = chrono::Utc::now().timestamp() + 10;

        let refreshed = refresh_session(&state, &session_id, refreshed_at)
            .await
            .unwrap();

        assert_eq!(refreshed, SessionRefresh::Active { user_id });
        let (last_seen_at,): (i64,) =
            sqlx::query_as("SELECT last_seen_at FROM admin_sessions WHERE id = ?")
                .bind(&session_id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert_eq!(last_seen_at, refreshed_at);
    }

    #[tokio::test]
    async fn refresh_session_deletes_expired_session() {
        let (state, user_id, _tmp) = state_with_admin().await;
        let session_id = create_session(&state, &user_id).await.unwrap();
        sqlx::query("UPDATE admin_sessions SET expires_at = ? WHERE id = ?")
            .bind(10_i64)
            .bind(&session_id)
            .execute(&state.pool)
            .await
            .unwrap();

        let refreshed = refresh_session(&state, &session_id, 11).await.unwrap();

        assert_eq!(refreshed, SessionRefresh::Expired);
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM admin_sessions WHERE id = ?")
            .bind(&session_id)
            .fetch_one(&state.pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn extractor_borrows_cookie_value_until_return() {
        let source = include_str!("session.rs");
        let impl_start = source
            .find("impl FromRequestParts<AppState> for AdminSession")
            .expect("extractor impl exists");
        let impl_end = source[impl_start..]
            .find("\npub fn make_cookie")
            .map(|idx| impl_start + idx)
            .expect("make_cookie follows extractor");
        let impl_source = &source[impl_start..impl_end];

        assert!(
            !impl_source.contains(".map(|c| c.value().to_string())"),
            "extractor should not allocate a session id before SQL validation"
        );
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

    #[test]
    fn expired_cookie_keeps_session_cookie_security_attributes() {
        let cookie = expired_cookie(true);
        assert_eq!(cookie.http_only(), Some(true));
        assert_eq!(cookie.secure(), Some(true));
        assert_eq!(
            cookie.same_site(),
            Some(tower_cookies::cookie::SameSite::Lax)
        );
        assert_eq!(cookie.path(), Some("/admin"));
    }
}
