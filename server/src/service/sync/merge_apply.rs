use crate::api::error::ApiError;
use crate::db::repos::{BlobRefRepo, BlobUploadRepo};
use crate::service::events::EventChange;
use crate::service::merge::MergeOutcome;
use crate::storage::blob::BlobStore;
use crate::storage::git::{FileChange, Git2VaultStore, GitStoreError, GitVaultStore, StoredFile};
use crate::storage::path;

use super::events::{text_event_with_budget, SseInlineBudget};
use super::paths::{
    ensure_generated_push_path, generated_push_path_is_valid, reject_filtered_push_path,
};
use super::push::commit_prepared_push;
use super::{
    AutoMergePushInput, CommitPushInput, MergeOutcomeEntry, MergeOutcomeKind, PreparedPush,
    PushChange, PushReq, PushResp,
};

pub(super) async fn try_auto_merge_push(
    input: AutoMergePushInput<'_>,
) -> Result<Option<PushResp>, ApiError> {
    let git = input.state.git_store();
    let inline_max = input.runtime_cfg.inline_content_max_bytes as usize;
    let mut inline_budget = SseInlineBudget::new(inline_max);
    let mut git_changes = Vec::new();
    let mut event_changes = Vec::new();
    let mut merge_outcomes = Vec::new();
    let mut blob_hashes = Vec::new();
    let mut clean_merges = 0;
    let mut conflict_merges = 0;
    let PushReq {
        changes,
        device_name,
    } = input.req;
    let conflict_device_name = device_name.as_deref().unwrap_or(&input.user.username);

    for change in changes {
        match change {
            PushChange::Text { path, content } => {
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

                let base_text =
                    read_merge_text(&git, input.vault_id, &normalized, input.base_commit).await?;
                let remote_text =
                    read_merge_text(&git, input.vault_id, &normalized, input.current_head).await?;

                match (base_text, remote_text) {
                    (MergeTextRead::Unmergeable, _) | (_, MergeTextRead::Unmergeable) => {
                        return Ok(None);
                    }
                    (MergeTextRead::Missing, MergeTextRead::Missing) => {
                        event_changes.push(text_event_with_budget(
                            &normalized,
                            &content,
                            &mut inline_budget,
                        ));
                        git_changes.push(FileChange::Upsert {
                            path: normalized.clone(),
                            file: StoredFile::Text {
                                bytes: content.into_bytes(),
                            },
                        });
                        merge_outcomes.push(MergeOutcomeEntry {
                            path: normalized,
                            outcome: MergeOutcomeKind::Clean,
                            conflict_path: None,
                        });
                        clean_merges += 1;
                    }
                    (MergeTextRead::Missing, MergeTextRead::Text(remote_bytes))
                        if remote_bytes == content.as_bytes() =>
                    {
                        merge_outcomes.push(MergeOutcomeEntry {
                            path: normalized,
                            outcome: MergeOutcomeKind::Clean,
                            conflict_path: None,
                        });
                        clean_merges += 1;
                    }
                    (MergeTextRead::Missing, MergeTextRead::Text(_))
                    | (MergeTextRead::Text(_), MergeTextRead::Missing) => {
                        write_text_conflict_sidecar(
                            &normalized,
                            content,
                            conflict_device_name,
                            &mut event_changes,
                            &mut git_changes,
                            &mut merge_outcomes,
                            &mut inline_budget,
                        )?;
                        conflict_merges += 1;
                    }
                    (MergeTextRead::Text(base_bytes), MergeTextRead::Text(remote_bytes)) => {
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
                                    path: normalized.clone(),
                                    file: StoredFile::Text {
                                        bytes: merged.into_bytes(),
                                    },
                                });
                                merge_outcomes.push(MergeOutcomeEntry {
                                    path: normalized,
                                    outcome: MergeOutcomeKind::Merged,
                                    conflict_path: None,
                                });
                                clean_merges += 1;
                            }
                            MergeOutcome::Conflicted(marked) => {
                                let cp = conflict_path_for(&normalized, conflict_device_name);
                                ensure_generated_push_path(&cp)?;
                                event_changes.push(text_event_with_budget(
                                    &cp,
                                    &marked,
                                    &mut inline_budget,
                                ));
                                git_changes.push(FileChange::Upsert {
                                    path: cp.clone(),
                                    file: StoredFile::Text {
                                        bytes: marked.into_bytes(),
                                    },
                                });
                                merge_outcomes.push(MergeOutcomeEntry {
                                    path: normalized,
                                    outcome: MergeOutcomeKind::Conflict,
                                    conflict_path: Some(cp),
                                });
                                conflict_merges += 1;
                            }
                            MergeOutcome::Binary => {
                                return Ok(None);
                            }
                        }
                    }
                }
            }
            PushChange::Delete { path } => {
                let normalized = path::normalize(&path)
                    .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
                reject_filtered_push_path(input.path_filter, &normalized)?;

                // Compare the file at base_commit vs current_head.
                let base_file = git
                    .read_file(input.vault_id, &normalized, Some(input.base_commit))
                    .await
                    .map_err(|e| ApiError::bad_request("bad_commit", e.to_string()))?;
                let head_file = git
                    .read_file(input.vault_id, &normalized, Some(input.current_head))
                    .await
                    .map_err(|e| ApiError::bad_request("bad_commit", e.to_string()))?;

                match (base_file.as_ref(), head_file.as_ref()) {
                    // Both absent or same content at base and head → delete is clean.
                    (None, None) => {
                        merge_outcomes.push(MergeOutcomeEntry {
                            path: normalized,
                            outcome: MergeOutcomeKind::Clean,
                            conflict_path: None,
                        });
                        clean_merges += 1;
                    }
                    // File existed at base but gone at head: remote already deleted it.
                    (Some(_), None) => {
                        merge_outcomes.push(MergeOutcomeEntry {
                            path: normalized,
                            outcome: MergeOutcomeKind::Clean,
                            conflict_path: None,
                        });
                        clean_merges += 1;
                    }
                    // File absent at base but present at head: the file was added by
                    // remote. Report conflict because the user's intent to ensure the
                    // file is gone cannot be honored.
                    (None, Some(_)) => {
                        merge_outcomes.push(MergeOutcomeEntry {
                            path: normalized,
                            outcome: MergeOutcomeKind::Conflict,
                            conflict_path: None,
                        });
                        conflict_merges += 1;
                    }
                    // Both present: check if remote modified it.
                    (Some(base_stored), Some(head_stored)) => {
                        let base_signature = stored_signature(base_stored);
                        let head_signature = stored_signature(head_stored);
                        if base_signature == head_signature {
                            // Remote didn't change it → emit the delete.
                            event_changes.push(crate::service::events::EventChange::Delete {
                                path: normalized.clone(),
                            });
                            git_changes.push(FileChange::Delete {
                                path: normalized.clone(),
                            });
                            merge_outcomes.push(MergeOutcomeEntry {
                                path: normalized,
                                outcome: MergeOutcomeKind::Clean,
                                conflict_path: None,
                            });
                            clean_merges += 1;
                        } else {
                            // Remote modified it → drop the delete, report conflict.
                            merge_outcomes.push(MergeOutcomeEntry {
                                path: normalized,
                                outcome: MergeOutcomeKind::Conflict,
                                conflict_path: None,
                            });
                            conflict_merges += 1;
                        }
                    }
                }
            }
            PushChange::Blob {
                path,
                blob_hash,
                size,
                mime,
            } => {
                let normalized = path::normalize(&path)
                    .map_err(|e| ApiError::bad_request("invalid_path", e.to_string()))?;
                reject_filtered_push_path(input.path_filter, &normalized)?;
                if size > input.runtime_cfg.max_file_size {
                    return Err(ApiError::bad_request(
                        "file_too_large",
                        format!(
                            "file exceeds max_file_size of {} bytes",
                            input.runtime_cfg.max_file_size
                        ),
                    ));
                }
                if !crate::storage::blob::is_sha256_hex(&blob_hash) {
                    return Err(ApiError::bad_request("invalid_hash", "invalid hash"));
                }

                // Check blob availability — same logic as fast path.
                let referenced = input
                    .state
                    .blob_refs
                    .referenced_hashes_for_vault(input.vault_id, std::slice::from_ref(&blob_hash))
                    .await
                    .map_err(|e| ApiError::internal(e.to_string()))?;
                let uploaded = input
                    .state
                    .blob_uploads
                    .uploaded_hashes_for_vault(input.vault_id, std::slice::from_ref(&blob_hash))
                    .await
                    .map_err(|e| ApiError::internal(e.to_string()))?;
                if !referenced.contains(&blob_hash) && !uploaded.contains(&blob_hash) {
                    return Err(ApiError::bad_request(
                        "missing_blob",
                        format!("blob {blob_hash} not uploaded for this vault"),
                    ));
                }
                let store = input.state.blob_store();
                let actual_size = match store
                    .size_bytes(&blob_hash)
                    .await
                    .map_err(|e| ApiError::bad_request("invalid_hash", e.to_string()))?
                {
                    Some(s) => s,
                    None => {
                        return Err(ApiError::bad_request(
                            "missing_blob",
                            format!("blob {blob_hash} not uploaded"),
                        ))
                    }
                };
                if actual_size != size {
                    return Err(ApiError::bad_request(
                        "blob_size_mismatch",
                        format!(
                            "declared size {size} does not match uploaded blob size {actual_size}"
                        ),
                    ));
                }

                // Compare base-tree vs head-tree at the blob's path.
                let base_file = git
                    .read_file(input.vault_id, &normalized, Some(input.base_commit))
                    .await
                    .map_err(|e| ApiError::bad_request("bad_commit", e.to_string()))?;
                let head_file = git
                    .read_file(input.vault_id, &normalized, Some(input.current_head))
                    .await
                    .map_err(|e| ApiError::bad_request("bad_commit", e.to_string()))?;

                let head_changed = match (base_file.as_ref(), head_file.as_ref()) {
                    // Both absent at this path → remote didn't change it.
                    (None, None) => false,
                    // Existed at base but gone at head → remote changed it.
                    (Some(_), None) => true,
                    // Added by remote → remote changed it.
                    (None, Some(_)) => true,
                    // Both present → compare bytes.
                    (Some(base_stored), Some(head_stored)) => {
                        stored_signature(base_stored) != stored_signature(head_stored)
                    }
                };

                if !head_changed {
                    // Remote didn't touch this path → adopt the blob.
                    blob_hashes.push(blob_hash.clone());
                    event_changes.push(crate::service::events::EventChange::Blob {
                        path: normalized.clone(),
                        blob_hash: blob_hash.clone(),
                        size,
                    });
                    git_changes.push(FileChange::Upsert {
                        path: normalized.clone(),
                        file: StoredFile::BlobPointer {
                            hash: blob_hash,
                            size,
                            mime,
                        },
                    });
                    merge_outcomes.push(MergeOutcomeEntry {
                        path: normalized,
                        outcome: MergeOutcomeKind::Clean,
                        conflict_path: None,
                    });
                    clean_merges += 1;
                } else {
                    // Remote modified this path → keep remote at original path;
                    // write local blob to a conflict sidecar path.
                    let cp = conflict_path_for(&normalized, conflict_device_name);
                    ensure_generated_push_path(&cp)?;
                    blob_hashes.push(blob_hash.clone());
                    event_changes.push(crate::service::events::EventChange::Blob {
                        path: cp.clone(),
                        blob_hash: blob_hash.clone(),
                        size,
                    });
                    git_changes.push(FileChange::Upsert {
                        path: cp.clone(),
                        file: StoredFile::BlobPointer {
                            hash: blob_hash,
                            size,
                            mime,
                        },
                    });
                    merge_outcomes.push(MergeOutcomeEntry {
                        path: normalized,
                        outcome: MergeOutcomeKind::Conflict,
                        conflict_path: Some(cp),
                    });
                    conflict_merges += 1;
                }
            }
        }
    }

    // If no git changes were produced (e.g., all deletes were no-ops or
    // conflicts), return the current head with the merge outcomes.
    if git_changes.is_empty() {
        if merge_outcomes.is_empty() {
            return Ok(None);
        }
        return Ok(Some(PushResp {
            new_commit: input.current_head.to_string(),
            files_changed: 0,
            merge_outcomes: Some(merge_outcomes),
        }));
    }

    let mut text_count = 0u64;
    let mut blob_count = 0u64;
    let mut delete_count = 0u64;
    for ch in &git_changes {
        match ch {
            FileChange::Upsert {
                file: StoredFile::BlobPointer { .. },
                ..
            } => blob_count += 1,
            FileChange::Upsert { .. } => text_count += 1,
            FileChange::Delete { .. } => delete_count += 1,
        }
    }

    let resp = commit_prepared_push(CommitPushInput {
        state: input.state,
        user: input.user,
        vault_id: input.vault_id,
        parent: Some(input.current_head.to_string()),
        device_name,
        prepared: PreparedPush {
            text_changes: text_count,
            blob_changes: blob_count,
            delete_changes: delete_count,
            git_changes,
            blob_hashes,
            event_changes,
        },
        idempotency_key: input.idempotency_key,
        request_hash: input.request_hash,
        request_metadata: input.request_metadata,
        merge_outcomes: Some(merge_outcomes),
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

fn stored_signature(file: &StoredFile) -> Vec<u8> {
    match file {
        StoredFile::Text { bytes } => {
            let mut out = Vec::with_capacity(bytes.len() + 5);
            out.extend_from_slice(b"text\0");
            out.extend_from_slice(bytes);
            out
        }
        StoredFile::BlobPointer { hash, size, mime } => {
            let mime = mime.as_deref().unwrap_or("");
            let mut out = Vec::with_capacity(hash.len() + mime.len() + 32);
            out.extend_from_slice(b"blob\0");
            out.extend_from_slice(hash.as_bytes());
            out.push(0);
            out.extend_from_slice(size.to_string().as_bytes());
            out.push(0);
            out.extend_from_slice(mime.as_bytes());
            out
        }
    }
}

fn write_text_conflict_sidecar(
    normalized: &str,
    content: String,
    conflict_device_name: &str,
    event_changes: &mut Vec<EventChange>,
    git_changes: &mut Vec<FileChange>,
    merge_outcomes: &mut Vec<MergeOutcomeEntry>,
    inline_budget: &mut SseInlineBudget,
) -> Result<(), ApiError> {
    let cp = conflict_path_for(normalized, conflict_device_name);
    ensure_generated_push_path(&cp)?;
    event_changes.push(text_event_with_budget(&cp, &content, inline_budget));
    git_changes.push(FileChange::Upsert {
        path: cp.clone(),
        file: StoredFile::Text {
            bytes: content.into_bytes(),
        },
    });
    merge_outcomes.push(MergeOutcomeEntry {
        path: normalized.to_string(),
        outcome: MergeOutcomeKind::Conflict,
        conflict_path: Some(cp),
    });
    Ok(())
}

enum MergeTextRead {
    Text(Vec<u8>),
    Missing,
    Unmergeable,
}

async fn read_merge_text(
    git: &Git2VaultStore,
    vault_id: &str,
    path: &str,
    at: &str,
) -> Result<MergeTextRead, ApiError> {
    let file = match git.read_file(vault_id, path, Some(at)).await {
        Ok(file) => file,
        Err(GitStoreError::Git(_)) => return Ok(MergeTextRead::Unmergeable),
        Err(err) => return Err(ApiError::bad_request("bad_commit", err.to_string())),
    };
    match file {
        Some(StoredFile::Text { bytes }) => Ok(MergeTextRead::Text(bytes)),
        Some(StoredFile::BlobPointer { .. }) => Ok(MergeTextRead::Unmergeable),
        None => Ok(MergeTextRead::Missing),
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
