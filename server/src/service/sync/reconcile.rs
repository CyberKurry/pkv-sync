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

/// The single timeout producer for push-lock acquisition (R9-1): push,
/// reconcile, cleanup, rollback and delete all bound their lock waits
/// with this. Tests drive a short value; production uses 30s.
pub fn vault_push_lock_timeout() -> Duration {
    if cfg!(test) {
        Duration::from_millis(50)
    } else {
        VAULT_PUSH_LOCK_TIMEOUT
    }
}

pub async fn reconcile_vault_metadata(
    state: &AppState,
    vault_id: &str,
) -> Result<ReconcileReport, ApiError> {
    reconcile_vault_metadata_with_timeout(state, vault_id, vault_push_lock_timeout()).await
}

pub async fn reconcile_vault_metadata_with_timeout(
    state: &AppState,
    vault_id: &str,
    timeout: Duration,
) -> Result<ReconcileReport, ApiError> {
    // R8-7: an unbounded lock acquire here meant one vault whose lock
    // holder is permanently stuck (pathological I/O inside a git commit,
    // an NFS stall) froze the whole scheduled cleanup forever — with no
    // log line saying cleanup stopped. A stuck holder keeps its vault
    // unreconciled (returned as an error, warned by the caller); every
    // other vault and every other cleanup duty keeps running.
    let _push_guard =
        match super::lock::acquire_vault_push_lock_bounded(state, vault_id, "reconcile").await {
            Ok(guard) => guard,
            Err(e) => {
                debug_assert_eq!(e.timeout, timeout, "caller-injected timeout must match");
                return Err(ApiError::internal(e.message));
            }
        };
    let _storage_guard = crate::service::acquire_storage_mutation_guard(state).await?;
    reconcile_vault_metadata_unlocked(state, vault_id).await
}
