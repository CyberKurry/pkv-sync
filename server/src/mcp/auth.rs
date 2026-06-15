use crate::auth::token;
use crate::auth::AuthenticatedUser;
use crate::db::repos::{TokenRepo, TokenRow, UserRepo};
use crate::service::AppState;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const TOKEN_VALIDITY_CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Debug, Default)]
pub(crate) struct TokenValidityCache {
    token_hash: String,
    valid: bool,
    token_epoch: u64,
    user_epoch: u64,
    cached_until: Option<Instant>,
}

pub(crate) async fn mcp_token_still_valid(
    state: &AppState,
    token_hash: &str,
    user: &AuthenticatedUser,
    cache: &mut TokenValidityCache,
) -> bool {
    let token_epoch = state.tokens.validity_epoch();
    let user_epoch = state.users.auth_epoch();
    let now = Instant::now();
    if cache.token_hash == token_hash
        && cache.token_epoch == token_epoch
        && cache.user_epoch == user_epoch
        && cache
            .cached_until
            .is_some_and(|cached_until| now < cached_until)
    {
        return cache.valid;
    }

    let Ok(Some((row, user_id))) = state.tokens.find_by_hash(token_hash).await else {
        cache.store(
            token_hash,
            false,
            token_epoch,
            user_epoch,
            ttl_deadline(now),
        );
        return false;
    };
    if row.id != user.token_id || user_id != user.user_id {
        cache.store(
            token_hash,
            false,
            token_epoch,
            user_epoch,
            ttl_deadline(now),
        );
        return false;
    }
    let Ok(Some(db_user)) = state.users.find_by_id(&user.user_id).await else {
        cache.store(
            token_hash,
            false,
            token_epoch,
            user_epoch,
            ttl_deadline(now),
        );
        return false;
    };
    let valid = db_user.is_active;
    let cached_until = if valid {
        token_cache_deadline(&row, now)
    } else {
        ttl_deadline(now)
    };
    cache.store(token_hash, valid, token_epoch, user_epoch, cached_until);
    valid
}

fn ttl_deadline(now: Instant) -> Instant {
    now + TOKEN_VALIDITY_CACHE_TTL
}

fn token_cache_deadline(row: &TokenRow, now: Instant) -> Instant {
    let absolute_expires_at = row
        .created_at
        .saturating_add(token::TOKEN_ABSOLUTE_LIFETIME_SECONDS);
    let expires_at = row.expires_at.min(absolute_expires_at);
    ttl_deadline(now).min(unix_timestamp_deadline(expires_at, now))
}

fn unix_timestamp_deadline(expires_at: i64, now: Instant) -> Instant {
    let Ok(expires_at) = u64::try_from(expires_at) else {
        return now;
    };
    let now_since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let remaining = Duration::from_secs(expires_at)
        .checked_sub(now_since_epoch)
        .unwrap_or_default();
    now.checked_add(remaining)
        .unwrap_or_else(|| ttl_deadline(now))
}

impl TokenValidityCache {
    fn store(
        &mut self,
        token_hash: &str,
        valid: bool,
        token_epoch: u64,
        user_epoch: u64,
        cached_until: Instant,
    ) {
        self.token_hash.clear();
        self.token_hash.push_str(token_hash);
        self.valid = valid;
        self.token_epoch = token_epoch;
        self.user_epoch = user_epoch;
        self.cached_until = Some(cached_until);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{password, token};
    use crate::db::pool;
    use crate::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};

    #[tokio::test]
    async fn cached_token_validity_rechecks_after_revocation_epoch_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap();
        let user = state
            .users
            .create(NewUser {
                username: "mcp-cache".into(),
                password_hash: password::hash("passw0rd!!").unwrap(),
                is_admin: false,
            })
            .await
            .unwrap();
        let raw = token::generate();
        let token_hash = token::hash(&raw);
        let row = state
            .tokens
            .create(NewToken {
                user_id: &user.id,
                token_hash: &token_hash,
                device_id: "mcp",
                device_name: "MCP",
            })
            .await
            .unwrap();
        let auth_user = AuthenticatedUser {
            user_id: user.id,
            username: user.username,
            is_admin: false,
            token_id: row.id.clone(),
            device_id: row.device_id,
        };
        let mut cache = TokenValidityCache::default();

        assert!(mcp_token_still_valid(&state, &token_hash, &auth_user, &mut cache).await);
        state
            .tokens
            .revoke(&row.id, chrono::Utc::now().timestamp())
            .await
            .unwrap();

        assert!(
            !mcp_token_still_valid(&state, &token_hash, &auth_user, &mut cache).await,
            "revocation epoch changes must invalidate the fresh token validity cache"
        );
    }

    #[tokio::test]
    async fn cached_token_validity_rechecks_after_user_active_epoch_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap();
        let user = state
            .users
            .create(NewUser {
                username: "mcp-user-cache".into(),
                password_hash: password::hash("passw0rd!!").unwrap(),
                is_admin: false,
            })
            .await
            .unwrap();
        let raw = token::generate();
        let token_hash = token::hash(&raw);
        let row = state
            .tokens
            .create(NewToken {
                user_id: &user.id,
                token_hash: &token_hash,
                device_id: "mcp",
                device_name: "MCP",
            })
            .await
            .unwrap();
        let auth_user = AuthenticatedUser {
            user_id: user.id.clone(),
            username: user.username,
            is_admin: false,
            token_id: row.id,
            device_id: row.device_id,
        };
        let mut cache = TokenValidityCache::default();

        assert!(mcp_token_still_valid(&state, &token_hash, &auth_user, &mut cache).await);
        state.users.set_active(&user.id, false).await.unwrap();

        assert!(
            !mcp_token_still_valid(&state, &token_hash, &auth_user, &mut cache).await,
            "user active epoch changes must invalidate the fresh token validity cache"
        );
    }

    #[tokio::test]
    async fn cached_token_validity_rechecks_after_natural_expiry() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap();
        let user = state
            .users
            .create(NewUser {
                username: "mcp-expiry-cache".into(),
                password_hash: password::hash("passw0rd!!").unwrap(),
                is_admin: false,
            })
            .await
            .unwrap();
        let raw = token::generate();
        let token_hash = token::hash(&raw);
        let row = state
            .tokens
            .create(NewToken {
                user_id: &user.id,
                token_hash: &token_hash,
                device_id: "mcp-expiry",
                device_name: "MCP Expiry",
            })
            .await
            .unwrap();
        let auth_user = AuthenticatedUser {
            user_id: user.id,
            username: user.username,
            is_admin: false,
            token_id: row.id.clone(),
            device_id: row.device_id,
        };
        let expires_at = chrono::Utc::now().timestamp() + 2;
        sqlx::query("UPDATE tokens SET expires_at = ? WHERE id = ?")
            .bind(expires_at)
            .bind(&row.id)
            .execute(&state.pool)
            .await
            .unwrap();
        let mut cache = TokenValidityCache::default();

        assert!(mcp_token_still_valid(&state, &token_hash, &auth_user, &mut cache).await);
        while chrono::Utc::now().timestamp() < expires_at {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        assert!(
            !mcp_token_still_valid(&state, &token_hash, &auth_user, &mut cache).await,
            "natural token expiry must not be hidden by the fresh validity cache"
        );
    }
}
