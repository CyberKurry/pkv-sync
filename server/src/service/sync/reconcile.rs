use crate::api::error::ApiError;
use crate::service::AppState;
use serde::Serialize;

use super::reconcile_vault_metadata_unlocked;

#[derive(Debug, Serialize)]
pub struct ReconcileReport {
    pub vault_id: String,
    pub head: Option<String>,
    pub size_bytes: i64,
    pub file_count: i64,
    pub blob_refs: usize,
}

pub async fn reconcile_vault_metadata(
    state: &AppState,
    vault_id: &str,
) -> Result<ReconcileReport, ApiError> {
    let push_lock = state.vault_push_lock(vault_id);
    let _push_guard = push_lock.lock().await;
    reconcile_vault_metadata_unlocked(state, vault_id).await
}
