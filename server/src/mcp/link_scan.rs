//! Fixed-commit vault scanning with budgets and completeness (MEL-07).
//!
//! `VaultSnapshot` pins ONE commit + filter + size budget for a whole
//! scan, so no consumer can drift onto a moving HEAD mid-operation. The
//! target index is built COMPLETE before any resolution is claimed; an
//! incomplete index yields `scan_complete=false` with fixed reason codes
//! instead of guessed relationships.

use crate::mcp::link_resolver::{self, Resolution, TargetIndex};
use crate::mcp::link_span::{scan_document, DocumentScan, LinkToken, ScanReason};
use crate::service::exclude::SyncPathFilter;
use crate::storage::git::{GitStoreError, GitVaultStore, StoredFile, TreeEntry};
use crate::storage::text_kind::TextClassifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScanLimits {
    pub max_files: usize,
    pub max_source_bytes: u64,
    pub max_links_per_file: usize,
    pub batch_size: usize,
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            max_files: 5000,
            max_source_bytes: 256 * 1024 * 1024,
            max_links_per_file: 10_000,
            batch_size: 64,
        }
    }
}

/// A scan pinned to one commit. Construct via `VaultSnapshot::capture`.
pub(crate) struct VaultSnapshot {
    pub at: String,
    pub filter: SyncPathFilter,
    /// Carried per the snapshot contract (MEL-07); consumers currently
    /// enforce per-batch byte budgets via ScanLimits, not per-file caps.
    #[allow(dead_code)]
    pub max_file_size: u64,
}

/// Completeness of one `visit_snapshot` run.
#[derive(Debug, Clone)]
pub(crate) struct ScanIntegrity {
    pub complete: bool,
    pub reasons: Vec<&'static str>,
}

impl Default for ScanIntegrity {
    fn default() -> Self {
        // A scan starts COMPLETE; failures downgrade it. (Derive-Default
        // gave `complete: false` with no reasons — a silent false negative.)
        Self {
            complete: true,
            reasons: Vec::new(),
        }
    }
}

impl ScanIntegrity {
    fn from_scan_reasons(reasons: &[ScanReason]) -> Self {
        let mut out = ScanIntegrity {
            complete: reasons.is_empty(),
            reasons: reasons
                .iter()
                .map(|r| match r {
                    ScanReason::FileCap => "file_cap",
                    ScanReason::ByteCap => "byte_cap",
                    ScanReason::LinkCap => "link_cap",
                    ScanReason::UnsupportedHost => "unsupported_host",
                })
                .collect(),
        };
        out.reasons.sort_unstable();
        out.reasons.dedup();
        out
    }
}

/// Build the COMPLETE target index from every visible entry (all file
/// kinds participate — resolution must see attachments too). Returns the
/// index plus integrity: a file-cap clip means basename uniqueness can no
/// longer be proven, so callers must treat resolution as partial.
pub(crate) async fn target_index(
    _git: &crate::storage::git::Git2VaultStore,
    snapshot: &VaultSnapshot,
    tree: &[TreeEntry],
    limits: &ScanLimits,
) -> (TargetIndex, ScanIntegrity) {
    let visible: Vec<String> = tree
        .iter()
        .filter(|entry| mcp_visible(&snapshot.filter, &entry.path))
        .map(|entry| entry.path.clone())
        .collect();
    let mut integrity = ScanIntegrity {
        complete: visible.len() <= limits.max_files,
        reasons: if visible.len() > limits.max_files {
            vec!["file_cap"]
        } else {
            Vec::new()
        },
    };
    // The index itself holds every visible path (no read amplification —
    // names only); the cap governs SOURCE scanning, and clipping the
    // index proves nothing about basename uniqueness when exceeded.
    if visible.len() > limits.max_files {
        // Still build from the first max_files entries so partial work is
        // possible, but mark incomplete (resolution claims stay guarded).
        let truncated: Vec<String> = visible.into_iter().take(limits.max_files).collect();
        return (TargetIndex::new(truncated), integrity);
    }
    integrity.complete = true;
    (TargetIndex::new(visible), integrity)
}

pub(crate) fn mcp_visible(filter: &SyncPathFilter, path: &str) -> bool {
    !crate::service::exclude::is_hidden_path(path)
        && crate::service::sync::path_visible_on_read(filter, path)
}

/// Per-file visit outcome the callback reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VisitFlow {
    Continue,
    Stop,
}

/// One source file handed to the visitor.
pub(crate) struct SourceFile<'a> {
    pub path: &'a str,
    pub text: String,
    pub scan: DocumentScan,
    /// sha256 of the stored bytes (hex) — for auto hash guards later.
    pub sha256: String,
}

/// Scan every visible TEXT file (semantic sources) at the pinned commit,
/// batching reads and enforcing budgets BEFORE content is materialized.
/// The visitor sees each file's text plus its DocumentScan; returning
/// `Stop` halts early (the caller's own limit hit) without marking the
/// SCAN incomplete. The tree is fetched once at the pinned commit.
pub(crate) async fn visit_snapshot<F>(
    git: &crate::storage::git::Git2VaultStore,
    vault_id: &str,
    snapshot: &VaultSnapshot,
    limits: &ScanLimits,
    visit: F,
) -> Result<ScanIntegrity, GitStoreError>
where
    F: FnMut(SourceFile<'_>) -> VisitFlow,
{
    let mut tree = git.list_tree(vault_id, Some(&snapshot.at)).await?;
    tree.sort_by(|a, b| a.path.cmp(&b.path));
    visit_snapshot_tree(git, vault_id, snapshot, tree, limits, visit).await
}

/// The real visitor over an already-fetched tree (avoids a second
/// list_tree in consumers that captured the tree with the snapshot).
pub(crate) async fn visit_snapshot_tree<F>(
    git: &crate::storage::git::Git2VaultStore,
    vault_id: &str,
    snapshot: &VaultSnapshot,
    tree: Vec<TreeEntry>,
    limits: &ScanLimits,
    mut visit: F,
) -> Result<ScanIntegrity, GitStoreError>
where
    F: FnMut(SourceFile<'_>) -> VisitFlow,
{
    let classifier = TextClassifier::default();
    let mut entries: Vec<TreeEntry> = tree
        .into_iter()
        .filter(|e| {
            !e.is_blob_pointer
                && classifier.is_text_path(&e.path)
                && mcp_visible(&snapshot.filter, &e.path)
        })
        .collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));

    let mut integrity = ScanIntegrity::default();
    if entries.len() > limits.max_files {
        integrity.complete = false;
        integrity.reasons.push("file_cap");
        entries.truncate(limits.max_files);
    }

    let mut scanned_bytes: u64 = 0;
    let mut index = 0;
    'outer: while index < entries.len() {
        // Budget the batch from tree sizes BEFORE reading (H-3 discipline):
        // an entry that does not fit the remaining budget never enters the
        // batch, so its content is never materialized.
        let mut batch_paths: Vec<String> = Vec::with_capacity(limits.batch_size);
        while batch_paths.len() < limits.batch_size && index < entries.len() {
            let entry = &entries[index];
            let size = entry.size;
            let used = scanned_bytes.saturating_add(size);
            if used > limits.max_source_bytes {
                integrity.complete = false;
                integrity.reasons.push("byte_cap");
                break 'outer;
            }
            scanned_bytes = used;
            batch_paths.push(entry.path.clone());
            index += 1;
        }
        let paths: Vec<&str> = batch_paths.iter().map(String::as_str).collect();
        let files = git.read_files(vault_id, Some(&snapshot.at), &paths).await?;
        let mut stopped = false;
        for (path, file) in batch_paths.iter().zip(files) {
            let Some(StoredFile::Text { bytes }) = file else {
                continue;
            };
            let Ok(text) = String::from_utf8(bytes) else {
                // Non-UTF-8 text-classified file: report, never skip
                // silently — it counts against completeness.
                integrity.complete = false;
                integrity.reasons.push("unsupported_host");
                continue;
            };
            let scan = scan_document(&text, limits.max_links_per_file);
            let file_integrity = ScanIntegrity::from_scan_reasons(&scan.reasons);
            if !file_integrity.complete {
                integrity.complete = false;
                for reason in file_integrity.reasons {
                    if !integrity.reasons.contains(&reason) {
                        integrity.reasons.push(reason);
                    }
                }
            }
            let sha = sha256_hex(text.as_bytes());
            if visit(SourceFile {
                path,
                text,
                scan,
                sha256: sha,
            }) == VisitFlow::Stop
            {
                stopped = true;
                break;
            }
        }
        if stopped {
            break;
        }
    }
    integrity.reasons.sort_unstable();
    integrity.reasons.dedup();
    Ok(integrity)
}

/// Resolve one token via the shared resolver with guarded completeness:
/// when the index is incomplete, basename uniqueness cannot be proven, so
/// a basename-only hit degrades to Missing rather than Resolved.
pub(crate) fn resolve_guarded(
    index: &TargetIndex,
    index_complete: bool,
    containing: &str,
    token: &LinkToken,
    source: &str,
) -> Resolution {
    let resolution = link_resolver::resolve(index, containing, token, source);
    if !index_complete {
        if let Resolution::Resolved(path) = &resolution {
            // Only DIRECT path matches stay claimable: a basename
            // fallback relies on having seen every file.
            let raw = &source[token.path.clone()];
            let direct = raw.trim().trim_start_matches('/');
            let direct_key = crate::mcp::link_resolver::case_fold_key(direct);
            let direct_key_md = crate::mcp::link_resolver::case_fold_key(&format!("{direct}.md"));
            let claimed = crate::mcp::link_resolver::case_fold_key(path);
            if claimed != direct_key && claimed != direct_key_md {
                return Resolution::Missing;
            }
        }
    }
    resolution
}

/// sha256 hex of stored bytes — shared with tools.rs's expected_hashes.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod resolve_guarded_tests {
    use super::*;
    use crate::mcp::link_resolver::{Resolution, TargetIndex};
    use crate::mcp::link_span::scan_document;

    #[test]
    fn mel_backlinks_guarded_basename_degrades_under_incomplete_index() {
        let source = "[[Note]]";
        let scan = scan_document(source, 10);
        let token = &scan.tokens[0];
        let index = TargetIndex::new(["dir/Note.md".to_string()]);
        // Complete index: basename claim stands.
        assert_eq!(
            resolve_guarded(&index, true, "I.md", token, source),
            Resolution::Resolved("dir/Note.md".into())
        );
        // Incomplete index: the same basename-only claim degrades to
        // Missing — another Note.md elsewhere could have changed the answer.
        assert_eq!(
            resolve_guarded(&index, false, "I.md", token, source),
            Resolution::Missing
        );
    }

    #[test]
    fn mel_backlinks_guarded_direct_path_claims_survive_incompleteness() {
        let source = "[[dir/Note]]";
        let scan = scan_document(source, 10);
        let token = &scan.tokens[0];
        let index = TargetIndex::new(["dir/Note.md".to_string()]);
        // A direct path match does not depend on basename uniqueness.
        assert_eq!(
            resolve_guarded(&index, false, "I.md", token, source),
            Resolution::Resolved("dir/Note.md".into())
        );
    }
}
