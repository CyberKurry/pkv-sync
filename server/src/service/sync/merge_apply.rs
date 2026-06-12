use crate::api::error::ApiError;
use crate::service::merge::MergeOutcome;
use crate::storage::git::{FileChange, Git2VaultStore, GitStoreError, GitVaultStore, StoredFile};
use crate::storage::path;

use super::events::{text_event_with_budget, SseInlineBudget};
use super::paths::{
    ensure_generated_push_path, generated_push_path_is_valid, reject_filtered_push_path,
};
use super::push::commit_prepared_push;
use super::{AutoMergePushInput, CommitPushInput, PreparedPush, PushChange, PushReq, PushResp};

pub(super) async fn try_auto_merge_push(
    input: AutoMergePushInput<'_>,
) -> Result<Option<PushResp>, ApiError> {
    let git = input.state.git_store();
    let inline_max = input.runtime_cfg.inline_content_max_bytes as usize;
    let mut inline_budget = SseInlineBudget::new(inline_max);
    let mut git_changes = Vec::new();
    let mut event_changes = Vec::new();
    let mut clean_merges = 0;
    let mut conflict_merges = 0;
    let PushReq {
        changes,
        device_name,
    } = input.req;
    let conflict_device_name = device_name.as_deref().unwrap_or(&input.user.username);

    for change in changes {
        let PushChange::Text { path, content } = change else {
            return Ok(None);
        };

        let normalized = path::normalize(&path)
            .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
        reject_filtered_push_path(input.path_filter, &normalized)?;
        if content.len() as u64 > input.runtime_cfg.max_file_size {
            return Err(ApiError::bad_request(
                "file_too_large",
                format!(
                    "file exceeds max_file_size of {} bytes",
                    input.runtime_cfg.max_file_size
                ),
            ));
        }
        if !input.classifier.is_text_path(&normalized) {
            return Err(ApiError::bad_request(
                "wrong_file_kind",
                "non-text path sent as text",
            ));
        }

        let Some(base_bytes) =
            read_merge_text(&git, input.vault_id, &normalized, input.base_commit).await?
        else {
            return Ok(None);
        };
        let Some(remote_bytes) =
            read_merge_text(&git, input.vault_id, &normalized, input.current_head).await?
        else {
            return Ok(None);
        };

        match crate::service::merge::three_way_merge_bytes(
            &base_bytes,
            content.as_bytes(),
            &remote_bytes,
        ) {
            MergeOutcome::Clean(merged) => {
                event_changes.push(text_event_with_budget(
                    &normalized,
                    &merged,
                    &mut inline_budget,
                ));
                git_changes.push(FileChange::Upsert {
                    path: normalized,
                    file: StoredFile::Text {
                        bytes: merged.into_bytes(),
                    },
                });
                clean_merges += 1;
            }
            MergeOutcome::Conflicted(marked) => {
                let conflict_path = conflict_path_for(&normalized, conflict_device_name);
                ensure_generated_push_path(&conflict_path)?;
                event_changes.push(text_event_with_budget(
                    &conflict_path,
                    &marked,
                    &mut inline_budget,
                ));
                git_changes.push(FileChange::Upsert {
                    path: conflict_path,
                    file: StoredFile::Text {
                        bytes: marked.into_bytes(),
                    },
                });
                conflict_merges += 1;
            }
            MergeOutcome::Binary => {
                let conflict_path = conflict_path_for(&normalized, conflict_device_name);
                ensure_generated_push_path(&conflict_path)?;
                event_changes.push(text_event_with_budget(
                    &conflict_path,
                    &content,
                    &mut inline_budget,
                ));
                git_changes.push(FileChange::Upsert {
                    path: conflict_path,
                    file: StoredFile::Text {
                        bytes: content.into_bytes(),
                    },
                });
                conflict_merges += 1;
            }
        }
    }

    if git_changes.is_empty() {
        return Ok(None);
    }

    let resp = commit_prepared_push(CommitPushInput {
        state: input.state,
        user: input.user,
        vault_id: input.vault_id,
        parent: Some(input.current_head.to_string()),
        device_name,
        prepared: PreparedPush {
            text_changes: git_changes.len() as u64,
            blob_changes: 0,
            delete_changes: 0,
            git_changes,
            blob_hashes: Vec::new(),
            event_changes,
        },
        idempotency_key: input.idempotency_key,
        request_hash: input.request_hash,
        request_metadata: input.request_metadata,
    })
    .await?;

    if clean_merges > 0 {
        input
            .state
            .metrics
            .auto_merge_clean_total
            .inc_by(clean_merges);
    }
    if conflict_merges > 0 {
        input
            .state
            .metrics
            .auto_merge_conflict_total
            .inc_by(conflict_merges);
    }

    tracing::info!(
        user_id = %input.user.user_id,
        vault_id = %input.vault_id,
        clean_merges,
        conflict_merges,
        "stale push handled by auto-merge"
    );
    Ok(Some(resp))
}

async fn read_merge_text(
    git: &Git2VaultStore,
    vault_id: &str,
    path: &str,
    at: &str,
) -> Result<Option<Vec<u8>>, ApiError> {
    let file = match git.read_file(vault_id, path, Some(at)).await {
        Ok(file) => file,
        Err(GitStoreError::Git(_)) => return Ok(None),
        Err(err) => return Err(ApiError::bad_request("bad_commit", err.to_string())),
    };
    match file {
        Some(StoredFile::Text { bytes }) => Ok(Some(bytes)),
        Some(StoredFile::BlobPointer { .. }) => Ok(None),
        None => Ok(Some(Vec::new())),
    }
}

fn conflict_path_for(original: &str, device_name: &str) -> String {
    let stamp = chrono::Utc::now().format("%Y-%m-%d-%H%M%S");
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let nonce = &nonce[..8];
    let device = safe_conflict_device_name(device_name);
    let slash = original.rfind('/');
    let (dir, file) = match slash {
        Some(idx) => (&original[..=idx], &original[idx + 1..]),
        None => ("", original),
    };
    let candidate = match file.rfind('.') {
        Some(dot) if dot > 0 => format!(
            "{}{}.conflict-{}-{}-{}{}",
            dir,
            &file[..dot],
            stamp,
            nonce,
            device,
            &file[dot..]
        ),
        _ => format!("{dir}{file}.conflict-{stamp}-{nonce}-{device}"),
    };
    if generated_push_path_is_valid(&candidate) {
        return candidate;
    }
    let ext = match file.rfind('.') {
        Some(dot) if dot > 0 && file[dot..].len() <= 16 => &file[dot..],
        _ => ".md",
    };
    let fallback = format!("{dir}conflict.conflict-{stamp}-{nonce}{ext}");
    if generated_push_path_is_valid(&fallback) {
        fallback
    } else {
        format!("conflict.conflict-{stamp}-{nonce}{ext}")
    }
}

/// A server-generated auto-merge conflict sidecar must remain visible on
/// read/pull surfaces even when its name matches a user exclude glob.
pub(crate) fn is_generated_conflict_sidecar(path: &str) -> bool {
    let file = path.rsplit('/').next().unwrap_or(path);
    match file.find(".conflict-") {
        Some(idx) => {
            let after = &file[idx + ".conflict-".len()..];
            after.len() > 4 && after.as_bytes()[..4].iter().all(u8::is_ascii_digit)
        }
        None => false,
    }
}

fn safe_conflict_device_name(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in name.trim().chars() {
        let safe = ch.is_ascii_alphanumeric() || ch == '_' || ch == '-';
        let next = if safe { ch } else { '-' };
        if next == '-' {
            if !last_dash {
                out.push(next);
            }
            last_dash = true;
        } else {
            out.push(next);
            last_dash = false;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "device".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_paths_are_unique_even_for_same_second_inputs() {
        let first = conflict_path_for("notes/daily.md", "Laptop");
        let second = conflict_path_for("notes/daily.md", "Laptop");

        assert_ne!(first, second);
        assert!(first.starts_with("notes/daily.conflict-"));
        assert!(first.ends_with(".md"));
    }

    #[test]
    fn conflict_path_for_long_input_stays_within_storage_limits() {
        let original = format!("{}/{}.md", "d".repeat(255), "a".repeat(252));
        assert_eq!(path::normalize(&original).unwrap().len(), 511);

        let conflict = conflict_path_for(&original, &"device".repeat(100));

        assert!(
            conflict.len() <= 512,
            "conflict path should fit storage path limit: {}",
            conflict.len()
        );
        path::normalize(&conflict).expect("generated conflict path should remain valid");
        assert!(conflict.contains(".conflict-"));
        assert!(conflict.ends_with(".md"));
    }

    #[test]
    fn conflict_sidecar_validation_accepts_normalized_percent_literals() {
        for (raw, normalized) in [
            ("note%252E.md", "note%2E.md"),
            ("%252E%252E/foo.md", "%2E%2E/foo.md"),
            ("%252Egit/foo.md", "%2Egit/foo.md"),
        ] {
            let original = path::normalize(raw).unwrap();
            assert_eq!(original, normalized);

            let conflict = conflict_path_for(&original, "Laptop");

            assert!(
                conflict.starts_with(normalized.trim_end_matches(".md"))
                    || conflict.starts_with("%2E%2E/foo.conflict-")
                    || conflict.starts_with("%2Egit/foo.conflict-"),
                "conflict path should preserve normalized literal percent escape near original file: {conflict}"
            );
            assert!(
                !conflict.starts_with("conflict.conflict-"),
                "conflict path should not fall back to vault root for {normalized}: {conflict}"
            );
            ensure_generated_push_path(&conflict)
                .expect("generated sidecar with literal percent escape should remain valid");
        }
    }

    #[test]
    fn generated_conflict_sidecar_detection_is_narrow() {
        assert!(is_generated_conflict_sidecar(
            "notes/daily.conflict-2026-06-09-123456-abcd1234-Laptop.md"
        ));
        assert!(is_generated_conflict_sidecar(
            "conflict.conflict-2026-06-09-123456-abcd1234.md"
        ));
        assert!(!is_generated_conflict_sidecar(
            "notes/daily.conflict-Laptop.md"
        ));
        assert!(!is_generated_conflict_sidecar(
            "notes/conflict-2026-06-09.md"
        ));
    }
}
