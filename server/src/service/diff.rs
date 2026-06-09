use crate::api::error::ApiError;
use crate::service::{vault, AppState};
use crate::storage::git::{GitVaultStore, StoredFile};
use crate::storage::path;
use serde::Serialize;

const MAX_PATCH_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitChange {
    pub path: String,
    pub change_type: ChangeType,
    pub old_path: Option<String>,
    pub binary: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UnifiedDiff {
    pub from: Option<String>,
    pub to: Option<String>,
    pub path: String,
    pub binary: bool,
    pub truncated: bool,
    pub patch: String,
}

/// Build a unified diff for a vault path after the caller has enforced read visibility.
///
/// REST handlers must call `sync::ensure_path_visible_for_sync_api` before this
/// helper so user exclude globs remain hidden from diff/read surfaces.
pub async fn unified_diff(
    state: &AppState,
    user_id: &str,
    vault_id: &str,
    from: Option<&str>,
    to: &str,
    file_path: &str,
) -> Result<UnifiedDiff, ApiError> {
    let _ = vault::ensure_user_vault(state, user_id, vault_id).await?;
    let file_path = path::normalize(file_path)
        .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
    let store = state.git_store();
    let from = match from {
        Some(commit) if !commit.is_empty() => Some(commit.to_string()),
        _ => store
            .commit_parent(vault_id, to)
            .await
            .map_err(|e| ApiError::bad_request("bad_commit", e.to_string()))?,
    };
    let to = to.to_string();

    let old_file = match from.as_deref() {
        Some(parent) => store
            .read_file(vault_id, &file_path, Some(parent))
            .await
            .map_err(|e| ApiError::bad_request("bad_commit", e.to_string()))?,
        None => None,
    };
    let new_file = store
        .read_file(vault_id, &file_path, Some(&to))
        .await
        .map_err(|e| ApiError::bad_request("bad_commit", e.to_string()))?;

    if old_file.is_none() && new_file.is_none() {
        return Err(ApiError::not_found("file not found"));
    }

    if matches!(old_file, Some(StoredFile::BlobPointer { .. }))
        || matches!(new_file, Some(StoredFile::BlobPointer { .. }))
    {
        return Ok(UnifiedDiff {
            from,
            to: Some(to),
            path: file_path,
            binary: true,
            truncated: false,
            patch: String::new(),
        });
    }

    let old_text = text_for_diff(old_file)?;
    let new_text = text_for_diff(new_file)?;
    let from_label = from.as_deref().unwrap_or("empty");
    let patch = similar::TextDiff::from_lines(&old_text, &new_text)
        .unified_diff()
        .context_radius(3)
        .header(from_label, &to)
        .to_string();
    let (patch, truncated) = truncate_patch(patch);

    Ok(UnifiedDiff {
        from,
        to: Some(to),
        path: file_path,
        binary: false,
        truncated,
        patch,
    })
}

fn text_for_diff(file: Option<StoredFile>) -> Result<String, ApiError> {
    match file {
        Some(StoredFile::Text { bytes }) => Ok(String::from_utf8(bytes)
            .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())),
        Some(StoredFile::BlobPointer { .. }) => Err(ApiError::bad_request(
            "binary_file",
            "binary files do not have inline diffs",
        )),
        None => Ok(String::new()),
    }
}

fn truncate_patch(mut patch: String) -> (String, bool) {
    if patch.len() <= MAX_PATCH_BYTES {
        return (patch, false);
    }
    let mut cutoff = MAX_PATCH_BYTES;
    while !patch.is_char_boundary(cutoff) {
        cutoff -= 1;
    }
    patch.truncate(cutoff);
    (patch, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_on_utf8_boundary() {
        let (patch, truncated) = truncate_patch("a".repeat(MAX_PATCH_BYTES + 1));
        assert!(truncated);
        assert_eq!(patch.len(), MAX_PATCH_BYTES);
    }

    #[test]
    fn text_for_diff_reuses_valid_utf8_text() {
        let text = text_for_diff(Some(StoredFile::Text {
            bytes: "hello".as_bytes().to_vec(),
        }))
        .unwrap();

        assert_eq!(text, "hello");
    }

    #[test]
    fn text_for_diff_replaces_invalid_utf8_text() {
        let text = text_for_diff(Some(StoredFile::Text {
            bytes: vec![b'a', 0xff, b'b'],
        }))
        .unwrap();

        assert_eq!(text, "a\u{fffd}b");
    }
}
