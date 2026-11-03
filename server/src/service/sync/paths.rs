use crate::api::error::ApiError;
use crate::service::vault_settings;
use crate::service::AppState;
use crate::storage::path;
use std::time::Duration;

use super::is_generated_conflict_sidecar;

const VAULT_PATH_FILTER_CACHE_TTL: Duration = Duration::from_secs(300);

pub(super) async fn sync_path_filter(
    state: &AppState,
    vault_id: &str,
    runtime_exclude_globs: &[String],
) -> Result<crate::service::exclude::SyncPathFilter, ApiError> {
    if let Some(filter) =
        state.cached_vault_path_filter(vault_id, runtime_exclude_globs, VAULT_PATH_FILTER_CACHE_TTL)
    {
        return Ok(filter);
    }
    let settings = vault_settings::load(state, vault_id).await?;
    // Both glob sources fail closed. An allowlist that silently degrades
    // to an empty set would hide every hidden file the admin explicitly
    // allowlisted (and reject their pushes with path_excluded) while the
    // only trace was a warn log (ultrareview M-7); the same holds for the
    // runtime exclude set. The admin/runtime-config save paths compile
    // before persisting, so reaching this branch means the stored data is
    // corrupt or was written by an older unvalidated path — the message
    // names the setting and the violated limit (never glob contents) so
    // the misconfiguration is visible and fixable.
    let user_excludes = crate::service::exclude::EffectiveExcludes::compile(runtime_exclude_globs)
        .map_err(|err| {
            tracing::error!(
                vault_id = %vault_id,
                error = %err,
                "extra_exclude_globs failed to compile; failing closed"
            );
            ApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "server_excludes_invalid",
                format!(
                    "server extra_exclude_globs failed to compile; \
                     fix or clear the server exclude configuration: {err}"
                ),
            )
        })?;
    let vault_allowlist = crate::service::exclude::EffectiveExcludes::compile(
        &settings.extra_sync_globs,
    )
    .map_err(|err| {
        tracing::error!(
            vault_id = %vault_id,
            error = %err,
            "vault extra_sync_globs failed to compile; failing closed"
        );
        ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "vault_settings_invalid",
            format!(
                "vault extra_sync_globs failed to compile; \
                         fix or clear the vault allowlist in settings: {err}"
            ),
        )
    })?;
    let filter = crate::service::exclude::SyncPathFilter::new(user_excludes, vault_allowlist);
    state.cache_vault_path_filter(vault_id, runtime_exclude_globs, filter.clone());
    Ok(filter)
}

/// Build the SyncPathFilter for a vault using the current runtime exclude globs.
/// Read surfaces (REST and MCP) use this to hide filter-rejected paths.
pub(crate) async fn vault_path_filter(
    state: &AppState,
    vault_id: &str,
) -> Result<crate::service::exclude::SyncPathFilter, ApiError> {
    let rc = state.runtime_cfg.snapshot().await;
    sync_path_filter(state, vault_id, &rc.extra_exclude_globs).await
}

/// Normalize `path` and enforce, in order: vault ownership, then read
/// visibility under the vault's sync filter.
///
/// R9-3: the ownership check lives HERE, not at each caller. The REST
/// handlers used to run the visibility check first, so a request for
/// another user's vault id answered from that vault's configuration — a
/// cross-tenant oracle distinguishing "excluded by their allowlist"
/// (file not found) from "not yours / not present" (vault not found), and
/// surfacing `vault_settings_invalid` for a stranger's broken config.
/// Order is ownership first; the answer for someone else's vault is
/// always `vault not found`.
pub(crate) async fn ensure_path_visible_for_sync_api(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    path: &str,
) -> Result<String, ApiError> {
    crate::service::vault::ensure_user_vault(state, user_id, vault_id).await?;
    let normalized =
        path::normalize(path).map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
    let rc = state.runtime_cfg.snapshot().await;
    let path_filter = sync_path_filter(state, vault_id, &rc.extra_exclude_globs).await?;
    if path_visible_on_read(&path_filter, &normalized) {
        Ok(normalized)
    } else {
        Err(ApiError::not_found("file not found"))
    }
}

pub(crate) fn path_visible_on_read(
    filter: &crate::service::exclude::SyncPathFilter,
    path: &str,
) -> bool {
    // Read APIs may expose vault-allowlisted hidden paths and generated
    // conflict sidecars. MCP mutating/graph/history tools layer on a stricter
    // hidden-path check so LLM agents cannot address hidden files for actions.
    filter.path_accepts(path) || is_generated_conflict_sidecar(path)
}

pub(super) fn reject_filtered_push_path(
    filter: &crate::service::exclude::SyncPathFilter,
    path: &str,
) -> Result<(), ApiError> {
    if filter.path_accepts(path) {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "path_excluded",
            format!("path '{}' is excluded by server configuration", path),
        ))
    }
}

pub(super) fn generated_push_path_is_valid(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.as_bytes().contains(&0) || path.len() > 512
    {
        return false;
    }
    path.split('/').all(|part| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && !part.eq_ignore_ascii_case(".git")
            && !part.contains('\\')
            && part.len() <= 255
    })
}

pub(super) fn ensure_generated_push_path(path: &str) -> Result<(), ApiError> {
    if generated_push_path_is_valid(path) {
        Ok(())
    } else {
        Err(ApiError::internal("generated path is invalid"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::UserRepo;

    #[test]
    fn generated_push_path_rejects_backslash_parts() {
        assert!(!generated_push_path_is_valid("notes\\daily.md"));
        assert!(!generated_push_path_is_valid("notes/daily\\todo.md"));
    }

    #[test]
    fn sync_path_filter_uses_vault_filter_cache() {
        let source = include_str!("paths.rs");
        let fn_start = source.find("async fn sync_path_filter").unwrap();
        let next_fn = source[fn_start + 1..]
            .find("\n/// Build the SyncPathFilter")
            .map(|idx| fn_start + 1 + idx)
            .unwrap();
        let implementation = &source[fn_start..next_fn];

        assert!(implementation.contains("cached_vault_path_filter"));
        assert!(implementation.contains("cache_vault_path_filter"));
    }

    #[tokio::test]
    async fn visibility_check_answers_vault_not_found_for_a_foreign_vault() {
        // R9-3: the REST handlers answered from the TARGET vault's sync
        // filter before checking ownership, so a cross-tenant probe could
        // distinguish "path inside their allowlist" (file not found) from
        // "vault not yours" (vault not found). Ownership must run FIRST;
        // the intruder gets the same answer whether the path would be
        // excluded or accepted, and the vault owner's configuration is
        // never consulted for a stranger.
        let tmp = tempfile::tempdir().unwrap();
        let pool = crate::db::pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state =
            crate::service::AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
                .await
                .unwrap();
        let owner = state
            .users
            .create(crate::db::repos::NewUser {
                username: "r9-owner".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let intruder = state
            .users
            .create(crate::db::repos::NewUser {
                username: "r9-intruder".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let vault = crate::service::vault::create_vault(&state, &owner.id, "main")
            .await
            .unwrap();

        for path in [".hidden/secret.md", "secret.md"] {
            let err = ensure_path_visible_for_sync_api(&state, &intruder.id, &vault.id, path)
                .await
                .unwrap_err();
            assert_eq!(
                err.message, "vault not found",
                "foreign-vault probe of {path:?} must answer vault not found, got: {} ({})",
                err.message, err.code
            );
        }
    }

    #[tokio::test]
    async fn visibility_check_still_applies_for_the_owner() {
        let tmp = tempfile::tempdir().unwrap();
        let pool = crate::db::pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        let state =
            crate::service::AppState::new(pool, tmp.path().to_path_buf(), "test".into(), true)
                .await
                .unwrap();
        let owner = state
            .users
            .create(crate::db::repos::NewUser {
                username: "r9b-owner".into(),
                password_hash: "h".into(),
                is_admin: false,
            })
            .await
            .unwrap();
        let vault = crate::service::vault::create_vault(&state, &owner.id, "main")
            .await
            .unwrap();

        let ok = ensure_path_visible_for_sync_api(&state, &owner.id, &vault.id, "note.md")
            .await
            .unwrap();
        assert_eq!(ok, "note.md");
        let hidden = ensure_path_visible_for_sync_api(&state, &owner.id, &vault.id, ".hidden/n.md")
            .await
            .unwrap_err();
        assert_eq!(hidden.message, "file not found");
    }
}
