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

/// Outcome of revalidating a session token (R2-3). "The database did not
/// answer" is not an answer about the token: a transient failure must not be
/// cached, and must not be acted on as a revocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TokenValidity {
    Valid,
    /// Terminal: the row is gone or revoked, belongs to another user or
    /// token, the account is disabled, or the token expired.
    Revoked,
    /// The lookup itself failed (database unreachable, pool closed,
    /// connection reset). Deliberately not cached, so the next call retries.
    Transient,
}

/// Revalidate the session token, distinguishing "no longer valid" from
/// "could not tell" (R2-3). Only definite answers are cached: caching a
/// transient error for [`TOKEN_VALIDITY_CACHE_TTL`] turned one database
/// hiccup into 5 seconds of a revoked-looking session, which the stdio drain
/// answered by silently wiping every listen subscription.
pub(crate) async fn mcp_token_validity(
    state: &AppState,
    token_hash: &str,
    user: &AuthenticatedUser,
    cache: &mut TokenValidityCache,
) -> TokenValidity {
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
        return if cache.valid {
            TokenValidity::Valid
        } else {
            TokenValidity::Revoked
        };
    }

    let (row, user_id) = match state.tokens.find_by_hash(token_hash).await {
        Ok(Some(found)) => found,
        Ok(None) => return revoke(cache, token_hash, token_epoch, user_epoch, now),
        Err(err) => {
            // debug, not warn: the stdio drain re-checks every 50ms, so a
            // database outage would otherwise log twice per tick. The caller
            // that has the loop context decides how loudly to report it.
            tracing::debug!(
                error = %err,
                "token revalidation could not read the token row"
            );
            return TokenValidity::Transient;
        }
    };
    if row.id != user.token_id || user_id != user.user_id {
        return revoke(cache, token_hash, token_epoch, user_epoch, now);
    }
    let db_user = match state.users.find_by_id(&user.user_id).await {
        Ok(Some(db_user)) => db_user,
        Ok(None) => return revoke(cache, token_hash, token_epoch, user_epoch, now),
        Err(err) => {
            tracing::debug!(
                error = %err,
                "token revalidation could not read the user row"
            );
            return TokenValidity::Transient;
        }
    };
    if !db_user.is_active {
        return revoke(cache, token_hash, token_epoch, user_epoch, now);
    }
    cache.store(
        token_hash,
        true,
        token_epoch,
        user_epoch,
        token_cache_deadline(&row, now),
    );
    TokenValidity::Valid
}

/// Thin boolean wrapper for the request path, where the answer is used as a
/// yes/no gate and there is no loop to retry in. A transient lookup failure
/// reads as "not valid" there (the caller answers with the generic auth
/// error), exactly as before R2-3.
pub(crate) async fn mcp_token_still_valid(
    state: &AppState,
    token_hash: &str,
    user: &AuthenticatedUser,
    cache: &mut TokenValidityCache,
) -> bool {
    matches!(
        mcp_token_validity(state, token_hash, user, cache).await,
        TokenValidity::Valid
    )
}

/// Records a definite "no" (and only a definite "no") in the validity cache.
fn revoke(
    cache: &mut TokenValidityCache,
    token_hash: &str,
    token_epoch: u64,
    user_epoch: u64,
    now: Instant,
) -> TokenValidity {
    cache.store(
        token_hash,
        false,
        token_epoch,
        user_epoch,
        ttl_deadline(now),
    );
    TokenValidity::Revoked
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

    #[tokio::test]
    async fn transient_lookup_errors_are_distinct_and_never_cached() {
        // R2-3: a database error is not an answer about the token. The old
        // `let Ok(Some(..)) = ... else { cache false; return false }` folded
        // `Err` into "revoked" and cached it for 5s, so one hiccup made the
        // session look revoked for the whole TTL — enough for the stdio
        // drain to wipe every listen it had.
        let tmp = tempfile::tempdir().unwrap();
        let pool = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
            .await
            .unwrap();
        let user = state
            .users
            .create(NewUser {
                username: "mcp-transient".into(),
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
                device_id: "mcp-transient",
                device_name: "MCP Transient",
            })
            .await
            .unwrap();
        let auth_user = AuthenticatedUser {
            user_id: user.id,
            username: user.username,
            is_admin: false,
            token_id: row.id,
            device_id: row.device_id,
        };
        let mut cache = TokenValidityCache::default();

        // Force a genuine repository error (not "row missing"): the lookup
        // table itself is gone for the duration of the outage.
        sqlx::query("ALTER TABLE tokens RENAME TO tokens_transient_outage")
            .execute(&state.pool)
            .await
            .unwrap();
        assert_eq!(
            mcp_token_validity(&state, &token_hash, &auth_user, &mut cache).await,
            TokenValidity::Transient,
            "a failed lookup must not be reported as a revocation"
        );

        // Restore the table. The very next check runs inside the 5s cache
        // window, so a cached "false" would still answer false here.
        sqlx::query("ALTER TABLE tokens_transient_outage RENAME TO tokens")
            .execute(&state.pool)
            .await
            .unwrap();
        assert_eq!(
            mcp_token_validity(&state, &token_hash, &auth_user, &mut cache).await,
            TokenValidity::Valid,
            "a transient error must not be cached; the retry must reach the database"
        );
    }
}
