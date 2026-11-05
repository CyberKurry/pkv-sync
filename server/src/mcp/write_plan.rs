//! One write plan: shared shape validation and derived previews (MEL-12).
//!
//! Every MCP write surface (write_files, edit_file, move_file+rewrite)
//! builds a `WritePlan` and validates it through ONE function, so limits
//! (1..=100 operations, per-file and total Text byte budgets, duplicate
//! and conflicting paths) cannot drift between tools. Dry-run previews
//! are DERIVED from the plan (would_write), never hand-built beside it.

use crate::service::sync::PushChange;
use crate::storage::path;
use std::collections::BTreeMap;

pub(crate) const MAX_OPERATIONS: usize = 100;

/// A finalized text write with its geometry known up front.
pub(crate) struct SizedText {
    pub path: String,
    pub final_len: usize,
    pub previous_len: Option<usize>,
}

pub(crate) struct RenamePair {
    pub from: String,
    pub to: String,
}

pub(crate) struct WritePlan {
    pub changes: Vec<PushChange>,
    pub rename: Option<RenamePair>,
    /// Previous sizes for delta reporting (paths not present are new).
    pub previous_sizes: BTreeMap<String, usize>,
}

/// Validate plan shape: canonical paths, Text/Delete mutual exclusion and
/// dedup, 1..=MAX_OPERATIONS physical ops, per-Text <= max_file_size,
/// and the SUM of all Text bytes <= max_file_size (R10-2 discipline).
/// Returns the normalized texts/deletes the caller assembled.
pub(crate) fn validate_plan_shape(
    texts: &[SizedText],
    deletes: &[String],
    max_file_size: u64,
    max_operations: usize,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        texts.len() + deletes.len() <= max_operations,
        "batch_too_large: {} changes exceeds limit of {max_operations}",
        texts.len() + deletes.len()
    );
    anyhow::ensure!(
        !texts.is_empty() || !deletes.is_empty(),
        "empty_batch: provide at least one write or delete"
    );
    let mut total_bytes: u64 = 0;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for text in texts {
        let normalized =
            path::normalize(&text.path).map_err(|e| anyhow::anyhow!("invalid_path: {e}"))?;
        anyhow::ensure!(
            normalized == text.path,
            "invalid_path: path is not normalized: {}",
            text.path
        );
        anyhow::ensure!(
            seen.insert(text.path.clone()),
            "duplicate_path: '{}' appears more than once",
            text.path
        );
        let len_u64 = u64::try_from(text.final_len)
            .map_err(|_| anyhow::anyhow!("file exceeds max_file_size of {max_file_size} bytes"))?;
        anyhow::ensure!(
            len_u64 <= max_file_size,
            "file exceeds max_file_size of {max_file_size} bytes: {}",
            text.path
        );
        total_bytes = total_bytes
            .checked_add(len_u64)
            .ok_or_else(|| anyhow::anyhow!("batch_bytes_exceed_budget: total overflow"))?;
    }
    anyhow::ensure!(
        total_bytes <= max_file_size,
        "batch_bytes_exceed_budget: {} files totalling {total_bytes} bytes exceeds max_file_size of {max_file_size} bytes; split the batch",
        texts.len()
    );
    for delete in deletes {
        let normalized =
            path::normalize(delete).map_err(|e| anyhow::anyhow!("invalid_path: {e}"))?;
        anyhow::ensure!(
            normalized == *delete,
            "invalid_path: path is not normalized: {delete}"
        );
        anyhow::ensure!(
            seen.insert(delete.clone()),
            "duplicate_path: '{}' appears in conflicting batch changes",
            delete
        );
    }
    Ok(())
}

/// The one mapping from a physical change set to dry-run preview lines.
/// A Delete(from) paired with exactly one Text(to) — and no other change
/// touching either — folds into a single `rename` line.
pub(crate) fn preview_from_plan(plan: &WritePlan) -> Vec<WouldWriteLine> {
    let mut lines = Vec::new();
    let rename = plan.rename.as_ref();
    let mut consumed: std::collections::HashSet<&str> = std::collections::HashSet::new();
    if let Some(pair) = rename {
        // Only fold when the physical changes really are the pair.
        let is_pair = plan
            .changes
            .iter()
            .any(|c| matches!(c, PushChange::Delete { path } if path == &pair.from))
            && plan
                .changes
                .iter()
                .any(|c| matches!(c, PushChange::Text { path, .. } if path == &pair.to));
        if is_pair {
            consumed.insert(pair.from.as_str());
            consumed.insert(pair.to.as_str());
            let final_len = plan
                .changes
                .iter()
                .find_map(|c| match c {
                    PushChange::Text { path, content } if path == &pair.to => Some(content.len()),
                    _ => None,
                })
                .unwrap_or(0);
            let previous = plan.previous_sizes.get(&pair.from).copied();
            lines.push(WouldWriteLine {
                path: pair.to.clone(),
                change: "rename",
                old_path: Some(pair.from.clone()),
                previous_size_bytes: previous,
                size_delta: delta(previous, final_len),
                size_bytes: final_len,
            });
        }
    }
    for change in &plan.changes {
        match change {
            PushChange::Text { path, content } => {
                if consumed.contains(path.as_str()) {
                    continue;
                }
                let previous = plan.previous_sizes.get(path).copied();
                lines.push(WouldWriteLine {
                    path: path.clone(),
                    change: "upsert",
                    old_path: None,
                    previous_size_bytes: previous,
                    size_delta: delta(previous, content.len()),
                    size_bytes: content.len(),
                });
            }
            PushChange::Delete { path } => {
                if consumed.contains(path.as_str()) {
                    continue;
                }
                let previous = plan.previous_sizes.get(path).copied();
                lines.push(WouldWriteLine {
                    path: path.clone(),
                    change: "delete",
                    old_path: None,
                    previous_size_bytes: previous,
                    size_delta: delta(previous, 0),
                    size_bytes: 0,
                });
            }
            PushChange::Blob { path, .. } => {
                lines.push(WouldWriteLine {
                    path: path.clone(),
                    change: "upsert",
                    old_path: None,
                    previous_size_bytes: None,
                    size_delta: None,
                    size_bytes: 0,
                });
            }
        }
    }
    lines
}

fn delta(previous: Option<usize>, current: usize) -> Option<i64> {
    previous.map(|p| {
        i64::try_from(current)
            .ok()
            .and_then(|c| i64::try_from(p).ok().map(|p| c - p))
            .unwrap_or_default()
    })
}

/// Mirror of tools::WouldWrite for this module's derived previews; tools
/// converts at its boundary (single mapping, no second producer).
pub(crate) struct WouldWriteLine {
    pub path: String,
    pub change: &'static str,
    pub old_path: Option<String>,
    pub previous_size_bytes: Option<usize>,
    pub size_delta: Option<i64>,
    pub size_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sized(path: &str, len: usize) -> SizedText {
        SizedText {
            path: path.into(),
            final_len: len,
            previous_len: None,
        }
    }

    #[test]
    fn mel_plan_rejects_duplicate_text_paths() {
        let texts = vec![sized("a.md", 10), sized("a.md", 20)];
        assert!(validate_plan_shape(&texts, &[], 100, 100)
            .unwrap_err()
            .to_string()
            .starts_with("duplicate_path:"));
    }

    #[test]
    fn mel_plan_rejects_delete_then_text_same_path() {
        let texts = vec![sized("a.md", 10)];
        let deletes = vec!["a.md".to_string()];
        assert!(validate_plan_shape(&texts, &deletes, 100, 100)
            .unwrap_err()
            .to_string()
            .starts_with("duplicate_path:"));
    }

    #[test]
    fn mel_plan_rejects_operation_overflow() {
        let texts: Vec<_> = (0..99).map(|i| sized(&format!("f{i}.md"), 1)).collect();
        let deletes = vec!["old.md".to_string(), "old2.md".to_string()];
        // 99 texts + 2 deletes = 101 > 100.
        let err = validate_plan_shape(&texts, &deletes, 100, 100).unwrap_err();
        assert!(err.to_string().starts_with("batch_too_large:"));
    }

    #[test]
    fn mel_plan_rejects_single_and_total_over_budget() {
        let big = sized("big.md", 101);
        assert!(validate_plan_shape(&[big], &[], 100, 100)
            .unwrap_err()
            .to_string()
            .contains("file exceeds max_file_size"));
        // Two files, each 60, total 120 > 100.
        let texts = vec![sized("a.md", 60), sized("b.md", 60)];
        let err = validate_plan_shape(&texts, &[], 100, 100).unwrap_err();
        assert!(err.to_string().starts_with("batch_bytes_exceed_budget:"));
    }

    #[test]
    fn mel_plan_rejects_empty_batch() {
        assert!(validate_plan_shape(&[], &[], 100, 100)
            .unwrap_err()
            .to_string()
            .starts_with("empty_batch:"));
    }

    #[test]
    fn mel_plan_preview_folds_rename_pair() {
        let plan = WritePlan {
            changes: vec![
                PushChange::Delete {
                    path: "old.md".into(),
                },
                PushChange::Text {
                    path: "new.md".into(),
                    content: "hello".into(),
                },
            ],
            rename: Some(RenamePair {
                from: "old.md".into(),
                to: "new.md".into(),
            }),
            previous_sizes: BTreeMap::from([("old.md".to_string(), 3)]),
        };
        let lines = preview_from_plan(&plan);
        assert_eq!(lines.len(), 1, "pair folds to one rename line");
        assert_eq!(lines[0].change, "rename");
        assert_eq!(lines[0].old_path.as_deref(), Some("old.md"));
        assert_eq!(lines[0].size_bytes, 5);
        assert_eq!(lines[0].previous_size_bytes, Some(3));
        assert_eq!(lines[0].size_delta, Some(2));
    }

    #[test]
    fn mel_plan_preview_without_rename_metadata_stays_two_lines() {
        let plan = WritePlan {
            changes: vec![
                PushChange::Delete {
                    path: "old.md".into(),
                },
                PushChange::Text {
                    path: "new.md".into(),
                    content: "x".into(),
                },
            ],
            rename: None,
            previous_sizes: BTreeMap::new(),
        };
        let lines = preview_from_plan(&plan);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].change, "delete");
        assert_eq!(lines[1].change, "upsert");
    }
}
