use crate::api::error::ApiError;
use crate::service::AppState;
use serde::Serialize;
use std::time::Duration;

use super::reconcile_vault_metadata_unlocked;

#[derive(Debug, Serialize)]
pub struct ReconcileReport {
    pub vault_id: String,
    pub head: Option<String>,
    pub size_bytes: i64,
    pub file_count: i64,
    pub blob_refs: usize,
}

/// R8-7: how long cleanup/admin reconcile waits on a vault push lock
/// before skipping that vault (warned) instead of hanging forever.
pub const VAULT_PUSH_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn test_lock_timeout() -> Duration {
    Duration::from_millis(50)
}

pub async fn reconcile_vault_metadata(
    state: &AppState,
    vault_id: &str,
) -> Result<ReconcileReport, ApiError> {
    reconcile_vault_metadata_with_timeout(state, vault_id, VAULT_PUSH_LOCK_TIMEOUT).await
}

pub async fn reconcile_vault_metadata_with_timeout(
    state: &AppState,
    vault_id: &str,
    timeout: Duration,
) -> Result<ReconcileReport, ApiError> {
    let push_lock = state.vault_push_lock(vault_id);
    // R8-7: an unbounded lock acquire here meant one vault whose lock
    // holder is permanently stuck (pathological I/O inside a git commit,
    // an NFS stall) froze the whole scheduled cleanup forever — with no
    // log line saying cleanup stopped. A stuck holder keeps its vault
    // unreconciled (returned as an error, warned by the caller); every
    // other vault and every other cleanup duty keeps running.
    let _push_guard = match tokio::time::timeout(timeout, push_lock.lock()).await {
        Ok(guard) => guard,
        Err(_) => {
            return Err(ApiError::internal(format!(
                "vault {vault_id}: push lock busy for over {timeout:?}, skipping reconcile"
            )));
        }
    };
    let _storage_guard = crate::service::acquire_storage_mutation_guard(state).await?;
    reconcile_vault_metadata_unlocked(state, vault_id).await
}
