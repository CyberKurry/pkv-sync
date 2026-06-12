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
    let user_excludes =
        match crate::service::exclude::EffectiveExcludes::compile(runtime_exclude_globs) {
            Ok(set) => set,
            Err(err) => {
                tracing::warn!(
                    vault_id = %vault_id,
                    error = %err,
                    "extra_exclude_globs failed to compile; ignoring all configured exclude globs"
                );
                crate::service::exclude::EffectiveExcludes::compile(&[]).unwrap()
            }
        };
    let vault_allowlist =
        match crate::service::exclude::EffectiveExcludes::compile(&settings.extra_sync_globs) {
            Ok(set) => set,
            Err(err) => {
                tracing::warn!(
                    vault_id = %vault_id,
                    error = %err,
                    "extra_sync_globs failed to compile; ignoring vault allowlist"
                );
                crate::service::exclude::EffectiveExcludes::compile(&[]).unwrap()
            }
        };
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

pub(crate) async fn ensure_path_visible_for_sync_api(
    state: &AppState,
    vault_id: &str,
    path: &str,
) -> Result<String, ApiError> {
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
}
