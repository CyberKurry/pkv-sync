use crate::api::error::ApiError;
use crate::auth::AuthenticatedUser;
use crate::db::repos::{BlobRefRepo, NewActivity, SyncActivityRepo, Vault, VaultRepo};
use crate::service::exclude::SyncPathFilter;
use crate::service::sync::{self, PushChange, PushReq, RequestMetadata};
use crate::service::vault::ensure_user_vault;
use crate::service::AppState;
use crate::storage::blob::BlobStore;
use crate::storage::git::{Git2VaultStore, GitStoreError, GitVaultStore, StoredFile};
use crate::storage::path;
use crate::storage::text_kind::TextClassifier;
use anyhow::{anyhow, bail, Result};
use base64::Engine;
use rmcp::model::{object, Tool, ToolAnnotations};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

const SEARCH_MAX_TREE_FILES: usize = 5000;
const MCP_READ_BATCH_SIZE: usize = 64;
#[cfg(test)]
const SEARCH_MAX_TOTAL_BYTES: usize = 64 * 1024;
#[cfg(not(test))]
const SEARCH_MAX_TOTAL_BYTES: usize = 256 * 1024 * 1024;
const LINK_GRAPH_MAX_NODES: usize = 5000;
#[cfg(test)]
const LINK_GRAPH_MAX_TOTAL_BYTES: usize = 64 * 1024;
#[cfg(not(test))]
const LINK_GRAPH_MAX_TOTAL_BYTES: usize = 256 * 1024 * 1024;
const CHANGES_SINCE_MAX_ENTRIES: usize = 5000;
const STALE_PAGES_MAX: usize = 500;
#[cfg(test)]
const STALE_PAGES_MAX_TOTAL_BYTES: usize = 64 * 1024;
#[cfg(not(test))]
const STALE_PAGES_MAX_TOTAL_BYTES: usize = 256 * 1024 * 1024;
#[cfg(test)]
const MAX_LINKS_PER_FILE: usize = 50;
#[cfg(not(test))]
const MAX_LINKS_PER_FILE: usize = 10_000;
#[cfg(test)]
const MAX_BROKEN_LINKS: usize = 60;
#[cfg(not(test))]
const MAX_BROKEN_LINKS: usize = 20_000;
#[cfg(test)]
const MCP_MAX_BINARY_RESPONSE_BYTES: u64 = 64;
#[cfg(not(test))]
const MCP_MAX_BINARY_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_SEARCH_LIMIT: usize = 100;
const SEARCH_MAX_LIMIT: usize = 500;
const LIST_VAULTS_HEAD_CONCURRENCY: usize = 16;
const WRITE_FILES_MAX_FILES: usize = 100;
const _: () = assert!(LIST_VAULTS_HEAD_CONCURRENCY > 0);
const _: () = assert!(LIST_VAULTS_HEAD_CONCURRENCY <= 32);

#[derive(Debug, Serialize)]
pub struct ListVaultsOutput {
    pub vaults: Vec<VaultSummary>,
}

#[derive(Debug, Serialize)]
pub struct VaultSummary {
    pub id: String,
    pub name: String,
    pub head_commit: Option<String>,
    pub file_count: i64,
    pub size_bytes: i64,
}

#[derive(Debug, Deserialize)]
pub struct ListFilesInput {
    pub vault_id: String,
    pub at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListFilesOutput {
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReadFileInput {
    pub vault_id: String,
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct ReadFileAtCommitInput {
    pub vault_id: String,
    pub path: String,
    pub commit: String,
}

#[derive(Debug, Serialize)]
pub struct ReadFileOutput {
    pub path: String,
    pub is_binary: bool,
    pub content: String,
    pub mime: Option<String>,
    pub encoding: Option<String>,
    /// SHA-256 of the exact stored bytes, hex encoded. This is the value the
    /// write tools' `expected_hashes` asks for, so a caller can base an edit
    /// on what it actually read instead of re-encoding the payload.
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchInput {
    pub vault_id: String,
    pub query: String,
    pub at: Option<String>,
    pub limit: Option<usize>,
    /// Restrict the scan to one subtree; matched against the tree before
    /// any file is read (no read amplification).
    #[serde(default)]
    pub path_prefix: Option<String>,
    /// Top-level frontmatter constraints: `true` requires the key to exist
    /// (scalar or list), a string requires a trimmed whole-string scalar
    /// match. Files dropped by this filter still count against the content
    /// budget.
    #[serde(default)]
    pub frontmatter: Option<std::collections::HashMap<String, Value>>,
    /// Opaque pagination cursor from a previous call's `next_after`.
    /// Pass it back verbatim; it encodes the (path, line) where the last
    /// page stopped, so no match is skipped or duplicated.
    #[serde(default)]
    pub after: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchOutput {
    pub matches: Vec<SearchMatch>,
    /// Opaque cursor to pass as `after` for the next page, or null when the
    /// walk finished. Best-effort snapshot: pin `at` for a stable walk.
    pub next_after: Option<String>,
}

/// Exact resume position for `search` pagination. `line` is the 1-based
/// line number of the last match returned for `path`; the next page resumes
/// at `line + 1`. Serialised as JSON (paths may contain any byte, so a
/// separator-joined format would be ambiguous) and treated as opaque by
/// clients.
#[derive(Debug, Serialize, Deserialize)]
struct SearchCursor {
    path: String,
    line: usize,
}

impl SearchCursor {
    fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn decode(raw: &str) -> Result<Self> {
        serde_json::from_str(raw).map_err(|_| {
            anyhow!("invalid_cursor: the `after` value was not produced by a previous search call")
        })
    }
}

#[derive(Debug, Serialize)]
pub struct SearchMatch {
    pub path: String,
    pub line: String,
    pub line_number: usize,
}

#[derive(Debug, Deserialize)]
pub struct StalePagesInput {
    pub vault_id: String,
    #[serde(default)]
    pub path_prefix: Option<String>,
    #[serde(default)]
    pub at: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

/// One wiki page whose declared sources changed after the commit the page
/// was last reconciled against. This is the mechanical half of lint: it
/// reports "a source moved under this page", never "this claim is wrong" —
/// judging the claim stays the agent's job.
#[derive(Debug, Serialize)]
pub struct StalePage {
    pub page: String,
    /// Sources whose content changed since `last_reconciled`.
    pub changed_sources: Vec<String>,
    /// Sources deleted since `last_reconciled`.
    pub removed_sources: Vec<String>,
    /// Sources moved since `last_reconciled` (the page still cites the old
    /// path, so it needs rewriting even though the content survived).
    pub renamed: Vec<RenamedSource>,
    /// Declared sources this server cannot check: excluded/hidden paths, or
    /// paths that never existed at `at`. Reported rather than dropped so a
    /// typo in `sources:` cannot hide behind a clean-looking result.
    pub unresolved_sources: Vec<String>,
    /// The `source_commit` the page declared, i.e. what the staleness was
    /// measured against.
    pub last_reconciled: Option<String>,
    /// False when the underlying git listing was clipped at its entry cap, so
    /// the change lists above may be incomplete. Such a page is always
    /// reported (even with nothing listed): a page that cannot be verified
    /// must not look clean.
    pub verified: bool,
}

#[derive(Debug, Serialize)]
pub struct RenamedSource {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Serialize)]
pub struct StalePagesOutput {
    pub stale: Vec<StalePage>,
    /// Pages that declare `sources:` but no `source_commit`. They need one
    /// reconciliation pass to anchor them; guessing an anchor from the page's
    /// own mtime would report a number the server cannot stand behind.
    pub missing_source_commit: Vec<String>,
    /// Pages whose declared `source_commit` is not an ancestor of `at`
    /// (later commit, rollback, or garbage collected): nothing can be
    /// measured, so they are counted rather than guessed at.
    pub skipped: usize,
    pub truncated: bool,
}

#[derive(Debug, Deserialize)]
pub struct LinkGraphInput {
    pub vault_id: String,
    pub at: Option<String>,
    pub path_prefix: Option<String>,
    pub limit: Option<usize>,
    /// Restrict the graph to files whose top-level frontmatter `kind:`
    /// equals this value. Targets still resolve against every visible text
    /// file, so a link that leaves the selected layer is not reported as
    /// broken unless the target is genuinely absent.
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LinkGraphOutput {
    pub nodes: Vec<LinkNode>,
    pub orphans: Vec<String>,
    pub broken: Vec<BrokenLink>,
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct LinkNode {
    pub path: String,
    pub outlinks: Vec<String>,
    pub inlinks: usize,
}

#[derive(Debug, Serialize)]
pub struct BrokenLink {
    pub from: String,
    pub raw_link: String,
    pub reason: BrokenReason,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrokenReason {
    Missing,
    Ambiguous,
}

#[derive(Debug, Deserialize)]
pub struct ChangesSinceInput {
    pub vault_id: String,
    pub since_commit: String,
    pub path_prefix: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ChangesSinceOutput {
    pub from_commit: String,
    pub to_commit: String,
    pub changes: Vec<crate::storage::git::ChangedEntry>,
    pub truncated: bool,
}

#[derive(Debug, Deserialize)]
pub struct WriteFileInput {
    pub vault_id: String,
    pub path: String,
    pub content: String,
    pub parent_commit: String,
    /// Validate everything and report what would change without committing.
    /// Consumes no write quota and records no activity. Passing this does not
    /// guarantee the real write will succeed: checks that live below the
    /// commit path (file-kind and exclude-glob enforcement) are not run.
    #[serde(default)]
    pub dry_run: Option<bool>,
    /// Optional guard per path: the SHA-256 the caller last read for it (from
    /// `read_file`'s `sha256`). A mismatch aborts the whole call with
    /// `content_diverged` and commits nothing.
    #[serde(default)]
    pub expected_hashes: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteFileInput {
    pub vault_id: String,
    pub path: String,
    pub parent_commit: String,
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(default)]
    pub expected_hashes: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct FileWrite {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct WriteFilesInput {
    pub vault_id: String,
    pub parent_commit: String,
    #[serde(default)]
    pub writes: Vec<FileWrite>,
    #[serde(default)]
    pub deletes: Vec<String>,
    #[serde(default)]
    pub dry_run: Option<bool>,
    #[serde(default)]
    pub expected_hashes: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
pub struct MoveFileInput {
    pub vault_id: String,
    pub parent_commit: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub dry_run: Option<bool>,
    /// Guard for the source path (the `from` side) only.
    #[serde(default)]
    pub expected_hashes: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum WriteToolOutput {
    Success {
        commit: String,
    },
    Conflict {
        conflict: bool,
        current_head: Option<String>,
    },
    /// A `dry_run` answer: exactly what the call would have written, with
    /// nothing committed.
    DryRun {
        dry_run: bool,
        would_write: Vec<WouldWrite>,
        parent_commit: String,
    },
}

#[derive(Debug, Serialize)]
pub struct WouldWrite {
    pub path: String,
    /// `upsert`, `delete` or `rename`.
    pub change: &'static str,
    /// Set for renames (the source path).
    pub old_path: Option<String>,
    pub size_bytes: usize,
}

/// SHA-256 of stored bytes, hex encoded — the currency of `expected_hashes`.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Refuse the whole batch when any guarded path no longer matches what the
/// caller read. This is the optimistic half of "AI writes safely": the
/// parent_commit CAS protects the vault as a whole, and this protects the
/// individual file the agent is editing from a concurrent human edit that
/// already landed in the same commit window.
async fn verify_expected_hashes(
    state: &AppState,
    vault_id: &str,
    at: Option<&str>,
    expected: Option<&std::collections::HashMap<String, String>>,
) -> Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let git = state.git_store();
    // Deterministic order so a multi-path mismatch always reports the same
    // path (and the tests can pin it).
    let mut paths: Vec<&String> = expected.keys().collect();
    paths.sort();
    for path in paths {
        let want = &expected[path];
        let actual = match git
            .read_file(vault_id, path, at)
            .await
            .map_err(git_error_to_anyhow)?
        {
            Some(StoredFile::Text { bytes }) => sha256_hex(&bytes),
            Some(StoredFile::BlobPointer { .. }) => {
                // Binary payloads are stored out of band; their bytes are
                // what a caller cannot have hashed from a `read_file` text
                // body, so a guard on one is refused outright rather than
                // silently accepted.
                bail!(
                    "content_diverged: path={path} expected={want} actual=binary-pointer (binary files cannot be guarded by content hash)"
                );
            }
            None => "null".to_string(),
        };
        if actual != *want {
            bail!("content_diverged: path={path} expected={want} actual={actual}");
        }
    }
    Ok(())
}

struct WriteToolRequest {
    vault_id: String,
    parent_commit: String,
    activity_action: &'static str,
    /// The MCP tool name, recorded in the activity details so the admin page
    /// can attribute a write to the agent that made it.
    activity_tool: &'static str,
    activity_path: String,
    activity_size_bytes: usize,
    change: PushChange,
    dry_run: bool,
    expected_hashes: Option<std::collections::HashMap<String, String>>,
}

struct WriteBatchRequest {
    vault_id: String,
    parent_commit: String,
    activity_action: &'static str,
    activity_tool: &'static str,
    activity_path: String,
    activity_size_bytes: usize,
    changes: Vec<PushChange>,
    dry_run: bool,
    expected_hashes: Option<std::collections::HashMap<String, String>>,
    /// Populated by the caller for a dry run; `changes` already describes
    /// what would happen, but a rename reports two changes (delete + add)
    /// and callers want it as one line.
    would_write: Vec<WouldWrite>,
}

pub async fn list_vaults(state: &AppState, user_id: &str) -> Result<ListVaultsOutput> {
    let git = state.git_store();
    let vaults = state.vaults.list_for_user(user_id).await?;
    let mut iter = vaults.into_iter().enumerate();
    let mut tasks = tokio::task::JoinSet::new();
    let mut summaries = Vec::new();

    for _ in 0..LIST_VAULTS_HEAD_CONCURRENCY {
        spawn_next_vault_head_task(&mut iter, &mut tasks, &git);
    }

    while let Some(result) = tasks.join_next().await {
        summaries.push(result??);
        spawn_next_vault_head_task(&mut iter, &mut tasks, &git);
    }

    summaries.sort_by_key(|(index, _)| *index);
    Ok(ListVaultsOutput {
        vaults: summaries.into_iter().map(|(_, summary)| summary).collect(),
    })
}

fn spawn_next_vault_head_task<I>(
    iter: &mut I,
    tasks: &mut tokio::task::JoinSet<Result<(usize, VaultSummary)>>,
    git: &Git2VaultStore,
) where
    I: Iterator<Item = (usize, Vault)>,
{
    if let Some((index, vault)) = iter.next() {
        let git = git.clone();
        tasks.spawn(async move {
            let head_commit = git.head(&vault.id).await?;
            Ok((
                index,
                VaultSummary {
                    id: vault.id,
                    name: vault.name,
                    head_commit,
                    file_count: vault.file_count,
                    size_bytes: vault.size_bytes,
                },
            ))
        });
    }
}

pub async fn list_files(
    state: &AppState,
    user_id: &str,
    input: ListFilesInput,
) -> Result<ListFilesOutput> {
    ensure_owned_vault(state, user_id, &input.vault_id).await?;
    let filter = sync::vault_path_filter(state, &input.vault_id)
        .await
        .map_err(api_error_to_anyhow)?;
    let git = state.git_store();
    let mut paths = git
        .list_tree(&input.vault_id, input.at.as_deref())
        .await
        .map_err(|err| {
            public_git_read_error(
                err,
                input.at.as_deref().unwrap_or("vault head"),
                input.at.is_some(),
            )
        })?
        .into_iter()
        .map(|entry| entry.path)
        .filter(|path| sync::path_visible_on_read(&filter, path))
        .collect::<Vec<_>>();
    paths.sort();
    Ok(ListFilesOutput { paths })
}

pub async fn read_file(
    state: &AppState,
    user_id: &str,
    input: ReadFileInput,
) -> Result<ReadFileOutput> {
    ensure_owned_vault(state, user_id, &input.vault_id).await?;
    let path = sync::ensure_path_visible_for_sync_api(state, &input.vault_id, &input.path)
        .await
        .map_err(api_error_to_anyhow)?;
    read_file_inner(state, user_id, input.vault_id, path, None).await
}

pub async fn read_file_at_commit(
    state: &AppState,
    user_id: &str,
    input: ReadFileAtCommitInput,
) -> Result<ReadFileOutput> {
    ensure_owned_vault(state, user_id, &input.vault_id).await?;
    let path = sync::ensure_path_visible_for_sync_api(state, &input.vault_id, &input.path)
        .await
        .map_err(api_error_to_anyhow)?;
    read_file_inner(state, user_id, input.vault_id, path, Some(input.commit)).await
}

pub async fn search(state: &AppState, user_id: &str, input: SearchInput) -> Result<SearchOutput> {
    ensure_owned_vault(state, user_id, &input.vault_id).await?;
    if input.query.is_empty() {
        // Deliberately not an "enumerate pages with this frontmatter" mode:
        // listing is list_files' job. An empty query returns no matches even
        // with filters attached.
        return Ok(SearchOutput {
            matches: Vec::new(),
            next_after: None,
        });
    }

    let git = state.git_store();
    let filter = sync::vault_path_filter(state, &input.vault_id)
        .await
        .map_err(api_error_to_anyhow)?;
    let prefix = match input.path_prefix.as_deref() {
        Some("") => String::new(),
        Some(prefix) => normalize_mcp_path(prefix.to_string())?,
        None => String::new(),
    };
    let cursor = match input.after.as_deref() {
        Some(raw) => Some(SearchCursor::decode(raw)?),
        None => None,
    };
    let tree = git
        .list_tree(&input.vault_id, input.at.as_deref())
        .await
        .map_err(|err| {
            public_git_read_error(
                err,
                input.at.as_deref().unwrap_or("vault head"),
                input.at.is_some(),
            )
        })?;
    // Sorted tree => the after_path cursor's <= comparison is a code
    // invariant instead of a git storage detail (list_files/link_graph
    // already sort; search joins them).
    let mut tree = tree;
    tree.sort_by(|left, right| left.path.cmp(&right.path));
    // The tree-size gate counts what this call will actually scan: prefix
    // and cursor first, so a big vault with a small wiki layer stays
    // searchable (the Synthesis-Horizon case the filter exists for).
    let visible_count = tree
        .iter()
        .filter(|entry| sync::path_visible_on_read(&filter, &entry.path))
        .filter(|entry| path_under_prefix(&entry.path, &prefix))
        .filter(|entry| {
            cursor
                .as_ref()
                .map(|cursor| entry.path.as_str() >= cursor.path.as_str())
                .unwrap_or(true)
        })
        .take(SEARCH_MAX_TREE_FILES + 1)
        .count();
    if visible_count > SEARCH_MAX_TREE_FILES {
        bail!("too many files to search: {}", visible_count);
    }

    let classifier = TextClassifier::default();
    let needle = input.query.to_ascii_lowercase();
    let limit = input
        .limit
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .min(SEARCH_MAX_LIMIT);
    let mut matches = Vec::new();
    let mut searched_bytes = 0usize;

    // Read candidate files in batches of MCP_READ_BATCH_SIZE through a single
    // repo open each, instead of opening the bare repo once per file. The
    // budget decision is made from tree-entry sizes BEFORE read_files
    // materializes blob contents, so an entry that does not fit in the
    // remaining budget never enters the chunk (H-3).
    let mut exhausted = true;
    let mut last_consumed_path: Option<(String, usize)> = None;
    let mut iter = tree
        .into_iter()
        .filter(|entry| sync::path_visible_on_read(&filter, &entry.path))
        .filter(|entry| path_under_prefix(&entry.path, &prefix))
        .filter(|entry| {
            cursor
                .as_ref()
                .map(|cursor| entry.path.as_str() >= cursor.path.as_str())
                .unwrap_or(true)
        });
    loop {
        let mut candidates: Vec<(String, u64)> = Vec::new();
        for entry in iter.by_ref() {
            if matches.len() >= limit {
                exhausted = false;
                break;
            }
            if entry.is_blob_pointer || !classifier.is_text_path(&entry.path) {
                continue;
            }
            candidates.push((
                entry.path,
                usize::try_from(entry.size).unwrap_or(usize::MAX) as u64,
            ));
            if candidates.len() >= MCP_READ_BATCH_SIZE {
                break;
            }
        }
        let (chunk, budget_exhausted) = collect_budgeted_chunk(
            &mut candidates.into_iter(),
            MCP_READ_BATCH_SIZE,
            (SEARCH_MAX_TOTAL_BYTES.saturating_sub(searched_bytes)) as u64,
        );
        if budget_exhausted {
            bail!("search content budget exceeded");
        }
        if chunk.is_empty() {
            break;
        }
        let paths: Vec<&str> = chunk.iter().map(|(path, _)| path.as_str()).collect();
        let files = git
            .read_files(&input.vault_id, input.at.as_deref(), &paths)
            .await
            .map_err(|err| {
                public_git_read_error(
                    err,
                    input.at.as_deref().unwrap_or("vault head"),
                    input.at.is_some(),
                )
            })?;
        for ((path, size), file) in chunk.into_iter().zip(files) {
            // Defense in depth: the chunk collector above already gates on
            // tree entry sizes, so this post-read accumulation should never
            // fire in practice. It stays because tree sizes could lie (a
            // corrupted or racing repo), and a lying tree must not turn the
            // budget into a silent success.
            searched_bytes = searched_bytes.saturating_add(size as usize);
            if searched_bytes > SEARCH_MAX_TOTAL_BYTES {
                bail!("search content budget exceeded");
            }
            let Some(StoredFile::Text { bytes }) = file else {
                continue;
            };
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            // Frontmatter constraints are checked after the read on purpose:
            // a file dropped here has still been materialised, so it must
            // keep counting against the content budget (otherwise the
            // filter becomes a way to enumerate a large vault for free).
            if let Some(wanted) = &input.frontmatter {
                // Validate the filter once, before scanning (R4-3): an
                // unsupported value used to silently drop every file, so
                // `{"draft": false}` read as "no page matches" instead of
                // "that comparison does not exist". Rejecting it up front
                // also stops the scan from spending the content budget on a
                // filter that can never match.
                for (key, value) in wanted {
                    if !matches!(value, Value::Bool(true) | Value::String(_)) {
                        bail!(
                            "invalid_path: frontmatter filter for '{key}' must be true or a string, got {value}"
                        );
                    }
                }
                let parsed = super::frontmatter::Frontmatter::parse(&text);
                let keep = wanted.iter().all(|(key, value)| match parsed.as_ref() {
                    None => false,
                    Some(fm) => match value {
                        Value::Bool(true) => fm.has(key),
                        Value::String(expected) => fm.scalar(key) == Some(expected.as_str()),
                        _ => false,
                    },
                });
                if !keep {
                    continue;
                }
            }
            // Resume inside the cursor's own file: skip the lines already
            // returned by the previous page. `limit` stays a hard cap, and
            // the (path, line) cursor means nothing is stranded by stopping
            // mid-file — the next page picks up exactly here.
            let first_line = cursor
                .as_ref()
                .filter(|cursor| cursor.path == path)
                .map(|cursor| cursor.line)
                .unwrap_or(0);
            let mut limit_hit = false;
            for (idx, line) in text.lines().enumerate() {
                let line_number = idx + 1;
                if line_number <= first_line {
                    continue;
                }
                if contains_ascii_case_insensitive(line, &needle) {
                    matches.push(SearchMatch {
                        path: path.clone(),
                        line: line.to_string(),
                        line_number,
                    });
                    if matches.len() >= limit {
                        last_consumed_path = Some((path.clone(), line_number));
                        limit_hit = true;
                        break;
                    }
                    last_consumed_path = Some((path.clone(), line_number));
                }
            }
            if limit_hit {
                // Files after this one in the tree are still unscanned.
                exhausted = false;
                break;
            }
        }
        if !exhausted {
            break;
        }
    }

    Ok(SearchOutput {
        matches,
        next_after: (!exhausted)
            .then(|| last_consumed_path.map(|(path, line)| SearchCursor { path, line }.encode()))
            .flatten(),
    })
}

/// Report which wiki pages have sources that changed after the page's
/// declared `source_commit`.
///
/// Semantics are deliberately narrow (see the LLM-wiki design spec): the
/// comparison is a git *tree diff* between the page's anchor and `at`, not
/// history reasoning about what "should" have happened, and the output is
/// facts for the agent to act on — contradictions and wrong claims are not
/// this tool's business. Visibility uses the stricter graph/history policy
/// (`mcp_path_visible`), so allowlisted hidden paths stay non-addressable
/// here just as they are for `link_graph` and `changes_since`.
pub async fn stale_pages(
    state: &AppState,
    user_id: &str,
    input: StalePagesInput,
) -> Result<StalePagesOutput> {
    ensure_owned_vault(state, user_id, &input.vault_id).await?;
    let filter = sync::vault_path_filter(state, &input.vault_id)
        .await
        .map_err(api_error_to_anyhow)?;
    let git = state.git_store();
    let classifier = TextClassifier::default();
    let prefix = match input.path_prefix.as_deref() {
        Some("") => String::new(),
        Some(prefix) => normalize_mcp_path(prefix.to_string())?,
        None => String::new(),
    };

    let at = match input.at.clone() {
        Some(at) => Some(at),
        None => git
            .head(&input.vault_id)
            .await
            .map_err(git_error_to_anyhow)?,
    };
    let Some(at) = at else {
        // A vault with no commits has nothing that can be stale.
        return Ok(StalePagesOutput {
            stale: Vec::new(),
            missing_source_commit: Vec::new(),
            skipped: 0,
            truncated: false,
        });
    };

    let mut tree = git.list_tree(&input.vault_id, Some(&at)).await?;
    tree.sort_by(|left, right| left.path.cmp(&right.path));
    // Every visible path at `at` answers "does this source still exist?",
    // independently of the text/prefix filters that select *pages*.
    let present: std::collections::HashSet<String> = tree
        .iter()
        .filter(|entry| mcp_path_visible(&filter, &entry.path))
        .map(|entry| entry.path.clone())
        .collect();

    let limit = input.limit.unwrap_or(STALE_PAGES_MAX).min(STALE_PAGES_MAX);
    let mut truncated = false;
    let mut scanned_bytes = 0usize;
    let mut pages: Vec<(String, super::frontmatter::Frontmatter, Vec<String>)> = Vec::new();
    let mut iter = tree.into_iter().filter(|entry| {
        !entry.is_blob_pointer
            && classifier.is_text_path(&entry.path)
            && path_under_prefix(&entry.path, &prefix)
            && mcp_path_visible(&filter, &entry.path)
    });
    loop {
        let mut candidates: Vec<(String, u64)> = Vec::new();
        for entry in iter.by_ref() {
            candidates.push((
                entry.path,
                usize::try_from(entry.size).unwrap_or(usize::MAX) as u64,
            ));
            if candidates.len() >= MCP_READ_BATCH_SIZE {
                break;
            }
        }
        let (chunk, budget_exhausted) = collect_budgeted_chunk(
            &mut candidates.into_iter(),
            MCP_READ_BATCH_SIZE,
            (STALE_PAGES_MAX_TOTAL_BYTES.saturating_sub(scanned_bytes)) as u64,
        );
        if budget_exhausted {
            // Reading budget: report what was collected as truncated rather
            // than failing the whole call (link_graph's contract, not
            // search's error).
            truncated = true;
            break;
        }
        if chunk.is_empty() {
            break;
        }
        let paths: Vec<&str> = chunk.iter().map(|(path, _)| path.as_str()).collect();
        let files = git
            .read_files(&input.vault_id, Some(&at), &paths)
            .await
            .map_err(|err| public_git_read_error(err, &at, true))?;
        for ((path, size), file) in chunk.into_iter().zip(files) {
            scanned_bytes = scanned_bytes.saturating_add(size as usize);
            if scanned_bytes > STALE_PAGES_MAX_TOTAL_BYTES {
                truncated = true;
                break;
            }
            let Some(StoredFile::Text { bytes }) = file else {
                continue;
            };
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            // Only pages that declare a scalar list of in-vault sources
            // participate; anything the mini-reader cannot parse simply has
            // no declared sources (never an error).
            let Some(frontmatter) = super::frontmatter::Frontmatter::parse(&text) else {
                continue;
            };
            let sources = match frontmatter.list("sources") {
                Some(sources) if !sources.is_empty() => sources.to_vec(),
                _ => continue,
            };
            // The cap is enforced on *pages*, not on files read: only a page
            // that this call actually drops counts as truncation.
            if pages.len() >= limit {
                truncated = true;
                break;
            }
            pages.push((path, frontmatter, sources));
        }
        if truncated {
            break;
        }
    }

    let mut stale = Vec::new();
    let mut missing_source_commit = Vec::new();
    let mut skipped = 0usize;
    // Pages usually share an anchor (one ingest run reconciles a batch), and
    // both git reads below depend only on the anchor. Caching them per anchor
    // turns N pages into one walk + one tree diff; measured on the 1000-commit
    // / 200-page gate this is the difference between 3.6s and well under 1s.
    // Bounded so a vault with one anchor per page cannot grow this without
    // limit (the cap is far above the distinct-anchor count real vaults show,
    // and beyond it the work is simply redone, not skipped).
    const ANCHOR_CACHE_MAX: usize = 16;
    let mut anchor_cache: std::collections::HashMap<
        String,
        Option<(Vec<crate::storage::git::ChangedEntry>, bool)>,
    > = std::collections::HashMap::new();
    for (page, frontmatter, sources) in pages {
        let Some(source_commit) = frontmatter.scalar("source_commit").map(str::to_string) else {
            missing_source_commit.push(page);
            continue;
        };
        let cached = anchor_cache.get(&source_commit).cloned();
        let resolved = match cached {
            Some(resolved) => resolved,
            None => {
                // `is_ancestor` is already at-relative: it answers "is this
                // commit an ancestor of the commit we are measuring at",
                // which is exactly the question. An unknown, later,
                // rolled-back or collected anchor has no answer, so the page
                // is counted rather than guessed at.
                let reachable = matches!(
                    git.is_ancestor(&input.vault_id, &source_commit, &at).await,
                    Ok(true)
                );
                let resolved = if reachable {
                    let (entries, delta_truncated) = git
                        .list_changes_between(&input.vault_id, &source_commit, &at)
                        .await?;
                    Some((entries, delta_truncated))
                } else {
                    None
                };
                if anchor_cache.len() < ANCHOR_CACHE_MAX {
                    anchor_cache.insert(source_commit.clone(), resolved.clone());
                }
                resolved
            }
        };
        let Some((entries, delta_truncated)) = resolved else {
            skipped += 1;
            continue;
        };
        truncated |= delta_truncated;

        let mut changed_sources = Vec::new();
        let mut removed_sources = Vec::new();
        let mut renamed = Vec::new();
        let mut unresolved_sources = Vec::new();
        for source in &sources {
            if !mcp_path_visible(&filter, source) {
                unresolved_sources.push(source.clone());
                continue;
            }
            let mut touched = false;
            for entry in &entries {
                if entry.path == *source {
                    match entry.status {
                        crate::storage::git::ChangeStatus::Deleted => {
                            removed_sources.push(source.clone());
                        }
                        _ => changed_sources.push(source.clone()),
                    }
                    touched = true;
                    break;
                }
                if entry.old_path.as_deref() == Some(source.as_str()) {
                    // The page still cites the old path: that is a rewrite
                    // even though the content survived the move.
                    removed_sources.push(source.clone());
                    renamed.push(RenamedSource {
                        from: source.clone(),
                        to: entry.path.clone(),
                    });
                    touched = true;
                    break;
                }
            }
            if !touched && !present.contains(source.as_str()) {
                unresolved_sources.push(source.clone());
            }
        }

        // A page is reported when it needs attention (a changed, removed,
        // renamed or unresolvable source) *or* when its listing was clipped
        // and we therefore cannot call it clean. The second half matters:
        // with a clipped diff, a page whose source changed outside the clip
        // would otherwise be indistinguishable from a fresh one (M-3).
        let verified = !delta_truncated;
        if verified
            && changed_sources.is_empty()
            && removed_sources.is_empty()
            && unresolved_sources.is_empty()
        {
            continue;
        }
        stale.push(StalePage {
            page,
            changed_sources,
            removed_sources,
            renamed,
            unresolved_sources,
            last_reconciled: Some(source_commit),
            verified,
        });
    }

    Ok(StalePagesOutput {
        stale,
        missing_source_commit,
        skipped,
        truncated,
    })
}

pub async fn link_graph(
    state: &AppState,
    user_id: &str,
    input: LinkGraphInput,
) -> Result<LinkGraphOutput> {
    ensure_owned_vault(state, user_id, &input.vault_id).await?;
    let filter = sync::vault_path_filter(state, &input.vault_id)
        .await
        .map_err(api_error_to_anyhow)?;
    let git = state.git_store();
    let classifier = TextClassifier::default();
    let prefix = match input.path_prefix {
        Some(prefix) if prefix.is_empty() => String::new(),
        Some(prefix) => normalize_mcp_path(prefix)?,
        None => String::new(),
    };
    let mut tree = git.list_tree(&input.vault_id, input.at.as_deref()).await?;
    tree.sort_by(|left, right| left.path.cmp(&right.path));

    let limit = input
        .limit
        .unwrap_or(LINK_GRAPH_MAX_NODES)
        .min(LINK_GRAPH_MAX_NODES);
    let mut graph_files = Vec::new();
    let mut scanned_bytes = 0usize;
    let mut truncated = false;
    // With a `kind` filter the reported nodes are one layer, but link
    // targets must still resolve against the whole visible text tree —
    // otherwise a wiki page citing a source page would be reported as a
    // broken link (a false statement the agent may then "fix").
    let resolution_paths: Vec<String> = match &input.kind {
        None => Vec::new(),
        Some(_) => tree
            .iter()
            .filter(|entry| {
                !entry.is_blob_pointer
                    && classifier.is_text_path(&entry.path)
                    && path_under_prefix(&entry.path, &prefix)
                    && !crate::service::exclude::is_hidden_path(&entry.path)
                    && sync::path_visible_on_read(&filter, &entry.path)
            })
            .map(|entry| entry.path.clone())
            .collect(),
    };
    let mut iter = tree.into_iter().filter(|entry| {
        !entry.is_blob_pointer
            && classifier.is_text_path(&entry.path)
            && path_under_prefix(&entry.path, &prefix)
            && !crate::service::exclude::is_hidden_path(&entry.path)
            && sync::path_visible_on_read(&filter, &entry.path)
    });
    loop {
        let mut candidates: Vec<(String, u64)> = Vec::new();
        for entry in iter.by_ref() {
            if graph_files.len() >= limit {
                truncated = true;
                break;
            }
            candidates.push((entry.path, entry.size));
            if candidates.len() >= MCP_READ_BATCH_SIZE {
                break;
            }
        }
        let (chunk, budget_exhausted) = collect_budgeted_chunk(
            &mut candidates.into_iter(),
            MCP_READ_BATCH_SIZE,
            (LINK_GRAPH_MAX_TOTAL_BYTES.saturating_sub(scanned_bytes)) as u64,
        );
        if budget_exhausted {
            // The next entry does not fit in the remaining byte budget: stop
            // scanning entirely rather than erroring — link_graph's contract
            // is a truncated result, not a failed request (H-3).
            truncated = true;
            break;
        }
        if chunk.is_empty() {
            break;
        }
        let paths: Vec<&str> = chunk.iter().map(|(path, _)| path.as_str()).collect();
        let files = git
            .read_files(&input.vault_id, input.at.as_deref(), &paths)
            .await?;
        for ((path, _size), file) in chunk.into_iter().zip(files) {
            if graph_files.len() >= limit {
                truncated = true;
                break;
            }
            let Some(text) = (match file {
                Some(StoredFile::Text { bytes }) => String::from_utf8(bytes).ok(),
                _ => None,
            }) else {
                continue;
            };
            // Defense in depth: the chunk collector above already gates on
            // tree entry sizes, so this post-read accumulation should never
            // fire in practice. It stays because tree sizes could lie (a
            // corrupted or racing repo), and a lying tree must not turn the
            // budget into a silent success.
            scanned_bytes = scanned_bytes.saturating_add(text.len());
            if scanned_bytes > LINK_GRAPH_MAX_TOTAL_BYTES {
                truncated = true;
                break;
            }
            if let Some(kind) = &input.kind {
                let matches = super::frontmatter::Frontmatter::parse(&text)
                    .and_then(|fm| fm.scalar("kind").map(str::to_string))
                    .as_deref()
                    == Some(kind.as_str());
                if !matches {
                    continue;
                }
            }
            graph_files.push((path.clone(), text));
        }
        if truncated {
            break;
        }
    }

    let index_paths: Vec<String> = if resolution_paths.is_empty() {
        graph_files.iter().map(|(path, _)| path.clone()).collect()
    } else {
        resolution_paths
    };
    let paths_by_path = index_paths.iter().cloned().collect::<HashSet<_>>();
    let mut by_basename: HashMap<String, Vec<String>> = HashMap::new();
    for path in &index_paths {
        if let Some(stem) = Path::new(path).file_stem().and_then(|s| s.to_str()) {
            by_basename
                .entry(stem.to_string())
                .or_default()
                .push(path.clone());
        }
    }

    let mut nodes = Vec::new();
    let mut inlinks: HashMap<String, usize> = HashMap::new();
    let mut broken = Vec::new();

    for (path, text) in graph_files {
        let mut outlinks = Vec::new();
        for link in extract_link_targets(&text) {
            // L-6: per-file link extraction and global broken-link
            // accumulation are capped; hitting either marks the graph
            // truncated instead of amplifying a link-spam file into
            // unbounded structures.
            if outlinks.len() >= MAX_LINKS_PER_FILE {
                truncated = true;
                break;
            }
            if broken.len() >= MAX_BROKEN_LINKS {
                truncated = true;
                break;
            }
            match resolve_target(
                &link.raw_link,
                link.kind,
                &path,
                &paths_by_path,
                &by_basename,
            ) {
                Ok(target) => {
                    *inlinks.entry(target.clone()).or_default() += 1;
                    outlinks.push(target);
                }
                Err(reason) => broken.push(BrokenLink {
                    from: path.clone(),
                    raw_link: link.raw_link,
                    reason,
                }),
            }
        }
        nodes.push(LinkNode {
            path,
            outlinks,
            inlinks: 0,
        });
        if truncated && broken.len() >= MAX_BROKEN_LINKS {
            break;
        }
    }
    for node in &mut nodes {
        node.inlinks = inlinks.get(&node.path).copied().unwrap_or(0);
    }
    let orphans = nodes
        .iter()
        .filter(|node| node.inlinks == 0)
        .map(|node| node.path.clone())
        .collect();

    Ok(LinkGraphOutput {
        nodes,
        orphans,
        broken,
        truncated,
    })
}

pub async fn changes_since(
    state: &AppState,
    user_id: &str,
    input: ChangesSinceInput,
) -> Result<ChangesSinceOutput> {
    ensure_owned_vault(state, user_id, &input.vault_id).await?;
    let filter = sync::vault_path_filter(state, &input.vault_id)
        .await
        .map_err(api_error_to_anyhow)?;
    let prefix = match input.path_prefix {
        Some(prefix) if prefix.is_empty() => String::new(),
        Some(prefix) => normalize_mcp_path(prefix)?,
        None => String::new(),
    };
    let git = state.git_store();
    let head = git
        .head(&input.vault_id)
        .await?
        .ok_or_else(|| anyhow!("invalid_commit: vault has no commits"))?;
    match git
        .is_ancestor(&input.vault_id, &input.since_commit, &head)
        .await
    {
        Ok(true) => {}
        Ok(false) => bail!("unrelated_commit: since_commit is not an ancestor of head"),
        Err(_) => bail!("invalid_commit: unknown since_commit"),
    }

    let limit = input
        .limit
        .unwrap_or(CHANGES_SINCE_MAX_ENTRIES)
        .min(CHANGES_SINCE_MAX_ENTRIES);
    let (delta_entries, delta_truncated) = git
        .list_changes_between(&input.vault_id, &input.since_commit, &head)
        .await?;
    let visible = delta_entries.into_iter().filter(|change| {
        path_under_prefix(&change.path, &prefix)
            && mcp_path_visible(&filter, &change.path)
            && change
                .old_path
                .as_ref()
                .is_none_or(|old_path| mcp_path_visible(&filter, old_path))
    });

    let mut changes = Vec::new();
    // The git-side clip and this call's own limit are both truncation: a
    // client that reconnects with the returned commit must know the list was
    // cut, or it will treat the gap as "nothing happened".
    let mut truncated = delta_truncated;
    for change in visible {
        if changes.len() >= limit {
            truncated = true;
            break;
        }
        changes.push(change);
    }

    Ok(ChangesSinceOutput {
        from_commit: input.since_commit,
        to_commit: head,
        changes,
        truncated,
    })
}

// MCP read surfaces such as list_files/search use path_visible_on_read(), which
// may expose vault-allowlisted hidden paths. Mutating and graph/history tools use
// this stricter policy so hidden/excluded paths remain non-addressable to agents.
fn mcp_path_visible(filter: &SyncPathFilter, path: &str) -> bool {
    !crate::service::exclude::is_hidden_path(path) && sync::path_visible_on_read(filter, path)
}

fn contains_ascii_case_insensitive(haystack: &str, lowercase_needle: &str) -> bool {
    let haystack = haystack.as_bytes();
    let needle = lowercase_needle.as_bytes();
    let n = needle.len();
    if n == 0 {
        return true;
    }
    if haystack.len() < n {
        return false;
    }
    let first = needle[0];
    let upper = first.to_ascii_uppercase();
    let rest = &needle[1..];
    let scan_limit = haystack.len() - n;
    let mut from = 0;
    loop {
        let region = &haystack[from..=scan_limit];
        let found = if upper != first {
            memchr::memchr2(first, upper, region)
        } else {
            memchr::memchr(first, region)
        };
        match found {
            None => return false,
            Some(rel) => {
                let start = from + rel;
                if rest
                    .iter()
                    .zip(&haystack[start + 1..start + n])
                    .all(|(right, left)| left.to_ascii_lowercase() == *right)
                {
                    return true;
                }
                from = start + 1;
            }
        }
    }
}

/// Extract link targets from markdown text: Obsidian wikilinks `[[T]]`,
/// `[[T#h]]`, `[[T|alias]]`, embeds `![[T]]`, and relative markdown links
/// `[text](path.md)`. External `http(s)://` and absolute links are ignored.
#[cfg(test)]
fn extract_links(text: &str) -> Vec<String> {
    extract_link_targets(text)
        .into_iter()
        .map(|link| link.raw_link)
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkKind {
    Wikilink,
    Markdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExtractedLink {
    raw_link: String,
    kind: LinkKind,
}

fn extract_link_targets(text: &str) -> Vec<ExtractedLink> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            if let Some(end) = text[i + 2..].find("]]") {
                let inner = &text[i + 2..i + 2 + end];
                let target = inner.split(['#', '|']).next().unwrap_or("").trim();
                if !target.is_empty() {
                    out.push(ExtractedLink {
                        raw_link: target.to_string(),
                        kind: LinkKind::Wikilink,
                    });
                }
                i = i + 2 + end + 2;
                continue;
            }
        }
        if bytes[i] == b']' && i + 1 < bytes.len() && bytes[i + 1] == b'(' {
            if let Some(end) = text[i + 2..].find(')') {
                let dest = text[i + 2..i + 2 + end].trim();
                if markdown_link_dest_is_relative(dest) {
                    out.push(ExtractedLink {
                        raw_link: dest.to_string(),
                        kind: LinkKind::Markdown,
                    });
                }
                i = i + 2 + end + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn markdown_link_dest_is_relative(dest: &str) -> bool {
    if dest.is_empty() || dest.starts_with('/') || dest.starts_with('\\') {
        return false;
    }
    if let Some(colon) = dest.find(':') {
        let before_colon = &dest[..colon];
        if !before_colon.is_empty() && before_colon.bytes().all(|byte| byte.is_ascii_alphabetic()) {
            return false;
        }
    }
    true
}

fn resolve_target(
    raw: &str,
    kind: LinkKind,
    from_path: &str,
    paths_by_path: &HashSet<String>,
    by_basename: &HashMap<String, Vec<String>>,
) -> std::result::Result<String, BrokenReason> {
    if kind == LinkKind::Markdown {
        match resolve_markdown_relative_target(raw, from_path, paths_by_path) {
            Ok(Some(target)) => return Ok(target),
            Ok(None) => {}
            Err(()) => return Err(BrokenReason::Missing),
        }
    }

    let candidate = if raw.ends_with(".md") {
        raw.to_string()
    } else {
        format!("{raw}.md")
    };
    if paths_by_path.contains(raw) {
        return Ok(raw.to_string());
    }
    if paths_by_path.contains(&candidate) {
        return Ok(candidate);
    }
    let base = Path::new(raw)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(raw);
    match by_basename.get(base).map(Vec::as_slice) {
        Some([only]) => Ok(only.clone()),
        Some([_, ..]) => Err(BrokenReason::Ambiguous),
        _ => Err(BrokenReason::Missing),
    }
}

fn resolve_markdown_relative_target(
    raw: &str,
    from_path: &str,
    paths_by_path: &HashSet<String>,
) -> std::result::Result<Option<String>, ()> {
    let normalized = normalize_markdown_relative_path(raw, from_path)?;
    let candidate = if normalized.ends_with(".md") {
        normalized.clone()
    } else {
        format!("{normalized}.md")
    };
    if paths_by_path.contains(&normalized) {
        return Ok(Some(normalized));
    }
    if paths_by_path.contains(&candidate) {
        return Ok(Some(candidate));
    }
    Ok(None)
}

fn normalize_markdown_relative_path(raw: &str, from_path: &str) -> std::result::Result<String, ()> {
    let mut parts = from_path
        .rsplit_once('/')
        .map(|(dir, _)| {
            dir.split('/')
                .filter(|part| !part.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let raw = raw.replace('\\', "/");
    for part in raw.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            parts.pop().ok_or(())?;
        } else {
            parts.push(part.to_string());
        }
    }
    let joined = parts.join("/");
    path::normalize(&joined).map_err(|_| ())
}

pub async fn write_file(
    state: &AppState,
    user: &AuthenticatedUser,
    input: WriteFileInput,
) -> Result<WriteToolOutput> {
    let path = normalize_mcp_path(input.path)?;
    let size_bytes = input.content.len();
    let max_file_size = state.runtime_cfg.max_file_size().await;
    if size_bytes as u64 > max_file_size {
        bail!("file exceeds max_file_size of {max_file_size} bytes");
    }
    apply_write_tool(
        state,
        user,
        WriteToolRequest {
            vault_id: input.vault_id,
            parent_commit: input.parent_commit,
            activity_action: "mcp_write",
            activity_tool: "write_file",
            activity_path: path.clone(),
            activity_size_bytes: size_bytes,
            change: PushChange::Text {
                path,
                content: input.content,
            },
            dry_run: input.dry_run.unwrap_or(false),
            expected_hashes: input.expected_hashes,
        },
    )
    .await
}

pub async fn delete_file(
    state: &AppState,
    user: &AuthenticatedUser,
    input: DeleteFileInput,
) -> Result<WriteToolOutput> {
    let path = normalize_mcp_path(input.path)?;
    apply_write_tool(
        state,
        user,
        WriteToolRequest {
            vault_id: input.vault_id,
            parent_commit: input.parent_commit,
            activity_action: "mcp_delete",
            activity_tool: "delete_file",
            activity_path: path.clone(),
            activity_size_bytes: 0,
            change: PushChange::Delete { path },
            dry_run: input.dry_run.unwrap_or(false),
            expected_hashes: input.expected_hashes,
        },
    )
    .await
}

pub async fn write_files(
    state: &AppState,
    user: &AuthenticatedUser,
    input: WriteFilesInput,
) -> Result<WriteToolOutput> {
    let total = input.writes.len() + input.deletes.len();
    if total == 0 {
        bail!("empty_batch: provide at least one write or delete");
    }
    if total > WRITE_FILES_MAX_FILES {
        bail!("batch_too_large: {total} changes exceeds limit of {WRITE_FILES_MAX_FILES}");
    }

    let max_file_size = state.runtime_cfg.max_file_size().await;
    let mut changes = Vec::with_capacity(total);
    let mut total_bytes = 0usize;
    let mut write_paths = HashSet::new();

    for write in input.writes {
        let path = normalize_mcp_path(write.path)?;
        if !write_paths.insert(path.clone()) {
            bail!("duplicate_path: '{path}' appears more than once");
        }
        let size = write.content.len();
        if size as u64 > max_file_size {
            // Same whitelisted prefix as write_file's single-file rejection
            // so the tools boundary can publish it: the path moves after the
            // prefix instead of in front of it (M-5 — the old "file '{path}'
            // exceeds …" phrasing started with the path and got masked as
            // `internal server error`).
            bail!("file exceeds max_file_size of {max_file_size} bytes: {path}");
        }
        total_bytes = total_bytes.saturating_add(size);
        changes.push(PushChange::Text {
            path,
            content: write.content,
        });
    }

    let mut delete_paths = HashSet::new();
    for delete in input.deletes {
        let path = normalize_mcp_path(delete)?;
        if write_paths.contains(&path) || !delete_paths.insert(path.clone()) {
            bail!("duplicate_path: '{path}' appears in conflicting batch changes");
        }
        changes.push(PushChange::Delete { path });
    }

    let would_write = would_write_from_changes(&changes);
    apply_write_batch(
        state,
        user,
        WriteBatchRequest {
            vault_id: input.vault_id,
            parent_commit: input.parent_commit,
            activity_action: "mcp_write",
            activity_tool: "write_files",
            activity_path: format!("{total} files"),
            activity_size_bytes: total_bytes,
            changes,
            dry_run: input.dry_run.unwrap_or(false),
            expected_hashes: input.expected_hashes,
            would_write,
        },
    )
    .await
}

pub async fn move_file(
    state: &AppState,
    user: &AuthenticatedUser,
    input: MoveFileInput,
) -> Result<WriteToolOutput> {
    let from = normalize_mcp_path(input.from)?;
    let to = normalize_mcp_path(input.to)?;
    if from == to {
        bail!("invalid_path: from and to are identical");
    }

    if let Some(conflict) =
        ensure_write_ready_without_rate_limit(state, user, &input.vault_id, &input.parent_commit)
            .await?
    {
        return Ok(conflict);
    }

    let filter = sync::vault_path_filter(state, &input.vault_id)
        .await
        .map_err(api_error_to_anyhow)?;
    let git = state.git_store();
    let at = (!input.parent_commit.is_empty()).then_some(input.parent_commit.as_str());
    if !mcp_path_visible(&filter, &from) {
        bail!("not_found: '{from}' does not exist");
    }
    if !mcp_path_visible(&filter, &to) {
        bail!("invalid_path: target path is not writable through MCP");
    }

    let source = match git.read_file(&input.vault_id, &from, at).await? {
        Some(file) => file,
        None => bail!("not_found: '{from}' does not exist"),
    };

    // A dry run answers before any of the write bookkeeping: no quota, no
    // activity, no CAS. Its validations are the ones above (parent currency,
    // visibility, source presence, target absence), which is exactly what the
    // real call checks first.
    if input.dry_run.unwrap_or(false) {
        let content = match &source {
            StoredFile::Text { bytes } => String::from_utf8(bytes.clone())
                .map_err(|_| anyhow!("unsupported_binary_move: '{from}' is not UTF-8 text"))?,
            StoredFile::BlobPointer { .. } => {
                bail!("unsupported_binary_move: '{from}' is binary; v1 supports text only");
            }
        };
        if git.read_file(&input.vault_id, &to, at).await?.is_some() {
            bail!("target_exists: '{to}' already exists");
        }
        verify_expected_hashes(state, &input.vault_id, at, input.expected_hashes.as_ref()).await?;
        return Ok(dry_run_output(
            &input.parent_commit,
            vec![WouldWrite {
                path: to,
                change: "rename",
                old_path: Some(from),
                size_bytes: content.len(),
            }],
        ));
    }

    // Unchanged ordering for the real path: the attempt is recorded before
    // the content-kind check, so a rejected binary move still costs one
    // attempt (pinned by `move_file_binary_source_consumes_write_rate_limit`),
    // while path-shaped refusals above cost nothing.
    record_mcp_write_rate_limit(state, user, &input.vault_id)?;
    let content = match source {
        StoredFile::Text { bytes } => String::from_utf8(bytes)
            .map_err(|_| anyhow!("unsupported_binary_move: '{from}' is not UTF-8 text"))?,
        StoredFile::BlobPointer { .. } => {
            bail!("unsupported_binary_move: '{from}' is binary; v1 supports text only");
        }
    };

    if git.read_file(&input.vault_id, &to, at).await?.is_some() {
        bail!("target_exists: '{to}' already exists");
    }
    verify_expected_hashes(state, &input.vault_id, at, input.expected_hashes.as_ref()).await?;
    let activity_size_bytes = content.len();

    apply_preflighted_write_batch(
        state,
        user,
        WriteBatchRequest {
            vault_id: input.vault_id,
            parent_commit: input.parent_commit,
            activity_action: "mcp_write",
            activity_tool: "move_file",
            activity_path: format!("{from} -> {to}"),
            activity_size_bytes,
            changes: vec![
                PushChange::Delete { path: from },
                PushChange::Text { path: to, content },
            ],
            dry_run: false,
            expected_hashes: None,
            would_write: Vec::new(),
        },
    )
    .await
}

fn normalize_mcp_path(raw_path: String) -> Result<String> {
    path::normalize(&raw_path).map_err(|err| anyhow!("invalid_path: {err}"))
}

/// L-8: a `path_prefix` filter must match the exact path or a child of it —
/// a bare `starts_with` also admitted sibling directories like `notes-old/`
/// under the prefix `notes`.
fn path_under_prefix(path: &str, prefix: &str) -> bool {
    prefix.is_empty() || path == prefix || path.starts_with(&format!("{prefix}/"))
}

static TOOL_DEFINITIONS: LazyLock<Vec<Tool>> = LazyLock::new(|| {
    vec![
        tool(
            "list_vaults",
            "List vaults available to the authenticated user.",
            object(serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            })),
        ),
        tool(
            "list_files",
            "List files in a vault at the current head or a specific commit.",
            object(serde_json::json!({
                "type": "object",
                "required": ["vault_id"],
                "properties": {
                    "vault_id": { "type": "string" },
                    "at": { "type": ["string", "null"] }
                },
                "additionalProperties": false
            })),
        ),
        tool(
            "read_file",
            "Read a file from a vault at the current head.",
            object(serde_json::json!({
                "type": "object",
                "required": ["vault_id", "path"],
                "properties": {
                    "vault_id": { "type": "string" },
                    "path": { "type": "string" }
                },
                "additionalProperties": false
            })),
        ),
        tool(
            "read_file_at_commit",
            "Read a file from a vault at a specific commit.",
            object(serde_json::json!({
                "type": "object",
                "required": ["vault_id", "path", "commit"],
                "properties": {
                    "vault_id": { "type": "string" },
                    "path": { "type": "string" },
                    "commit": { "type": "string" }
                },
                "additionalProperties": false
            })),
        ),
        tool(
            "search",
            "Search text files in a vault with case-insensitive substring matching, optionally scoped by path prefix, top-level frontmatter keys, and a pagination cursor.",
            object(serde_json::json!({
                "type": "object",
                "required": ["vault_id", "query"],
                "properties": {
                    "vault_id": { "type": "string" },
                    "query": { "type": "string" },
                    "at": { "type": ["string", "null"] },
                    "limit": { "type": ["integer", "null"], "minimum": 1, "maximum": SEARCH_MAX_LIMIT },
                    "path_prefix": { "type": ["string", "null"], "description": "Restrict the scan to one subtree; applied before any file is read and before the tree-size gate counts." },
                    "frontmatter": { "type": ["object", "null"], "description": "Top-level frontmatter constraints. A string value must equal the trimmed whole scalar; true requires the key to exist (scalars and lists both count). Any other value (false, numbers, arrays, objects) is rejected as invalid_path before scanning. Files dropped by this filter still count against the content budget. An empty query returns no matches even with filters." },
                    "after": { "type": ["string", "null"], "description": "Opaque pagination cursor: pass the previous response's `next_after` back verbatim. It encodes the (path, line) resume point, so pages neither skip nor duplicate matches, and `limit` stays a hard cap. Best-effort snapshot; pin 'at' for a stable walk." }
                },
                "additionalProperties": false
            })),
        ),
        tool(
            "link_graph",
            "Return the wikilink/markdown link graph for a vault: per-file outlinks, computed inlinks, orphaned pages, and broken links. Hidden paths are never reported.",
            object(serde_json::json!({
                "type": "object",
                "required": ["vault_id"],
                "properties": {
                    "vault_id": { "type": "string" },
                    "at": { "type": ["string", "null"] },
                    "path_prefix": { "type": ["string", "null"] },
                    "limit": { "type": ["integer", "null"], "minimum": 1, "maximum": LINK_GRAPH_MAX_NODES },
                    "kind": { "type": ["string", "null"], "description": "Only files whose top-level frontmatter `kind:` equals this value become nodes. Link targets still resolve against every visible text file, so links leaving the selected layer are not reported as broken unless the target is truly absent." }
                },
                "additionalProperties": false
            })),
        ),
        tool(
            "stale_pages",
            "List wiki pages whose declared `sources:` changed after the page's own `source_commit`. Reports changed/removed/renamed sources as git tree facts for the agent to reconcile; it does not judge whether a claim is wrong. Pages declaring sources without a source_commit come back in missing_source_commit so the first reconciliation pass can anchor them. A page whose listing was clipped is reported with verified=false even when nothing is listed, so an incomplete scan cannot read as clean. Hidden and excluded paths are never addressed.",
            object(serde_json::json!({
                "type": "object",
                "required": ["vault_id"],
                "properties": {
                    "vault_id": { "type": "string" },
                    "path_prefix": { "type": ["string", "null"], "description": "Restrict the page scan to one subtree." },
                    "at": { "type": ["string", "null"], "description": "Commit to measure staleness at; defaults to the vault head." },
                    "limit": { "type": ["integer", "null"], "minimum": 1, "maximum": STALE_PAGES_MAX }
                },
                "additionalProperties": false
            })),
        ),
        tool(
            "changes_since",
            "List files in a vault that were added, modified, deleted, or renamed since a given commit. Hidden paths are excluded.",
            object(serde_json::json!({
                "type": "object",
                "required": ["vault_id", "since_commit"],
                "properties": {
                    "vault_id": { "type": "string" },
                    "since_commit": { "type": "string" },
                    "path_prefix": { "type": ["string", "null"] },
                    "limit": { "type": ["integer", "null"], "minimum": 1, "maximum": CHANGES_SINCE_MAX_ENTRIES }
                },
                "additionalProperties": false
            })),
        ),
        write_tool(
            "write_file",
            "Create or update a text file in a vault using optimistic concurrency.",
            object(serde_json::json!({
                "type": "object",
                "required": ["vault_id", "path", "content", "parent_commit"],
                "properties": {
                    "vault_id": { "type": "string" },
                    "path": { "type": "string" },
                    "content": { "type": "string" },
                    "parent_commit": { "type": "string" },
                    "dry_run": { "type": ["boolean", "null"], "description": "Validate and report what would change (would_write) without committing. Consumes no write quota and records no activity. Not a guarantee the real write succeeds: file-kind and exclude-glob enforcement live below the commit path." },
                    "expected_hashes": { "type": ["object", "null"], "additionalProperties": { "type": "string" }, "description": "Optional per-path guard: the sha256 a previous read_file returned for that path. Any mismatch aborts the whole call with content_diverged and commits nothing." }
                },
                "additionalProperties": false
            })),
            false,
        ),
        write_tool(
            "delete_file",
            "Delete a file from a vault using optimistic concurrency.",
            object(serde_json::json!({
                "type": "object",
                "required": ["vault_id", "path", "parent_commit"],
                "properties": {
                    "vault_id": { "type": "string" },
                    "path": { "type": "string" },
                    "parent_commit": { "type": "string" },
                    "dry_run": { "type": ["boolean", "null"], "description": "Validate and report what would change (would_write) without committing. Consumes no write quota and records no activity. Not a guarantee the real write succeeds: file-kind and exclude-glob enforcement live below the commit path." },
                    "expected_hashes": { "type": ["object", "null"], "additionalProperties": { "type": "string" }, "description": "Optional per-path guard: the sha256 a previous read_file returned for that path. Any mismatch aborts the whole call with content_diverged and commits nothing." }
                },
                "additionalProperties": false
            })),
            true,
        ),
        write_tool(
            "write_files",
            "Atomically create/update and/or delete multiple text files in a vault in a single commit, using optimistic concurrency on parent_commit.",
            object(serde_json::json!({
                "type": "object",
                "required": ["vault_id", "parent_commit"],
                "properties": {
                    "vault_id": { "type": "string" },
                    "parent_commit": { "type": "string" },
                    "writes": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["path", "content"],
                            "properties": {
                                "path": { "type": "string" },
                                "content": { "type": "string" }
                            },
                            "additionalProperties": false
                        }
                    },
                    "deletes": { "type": "array", "items": { "type": "string" } },
                    "dry_run": { "type": ["boolean", "null"], "description": "Validate and report what would change (would_write) without committing. Consumes no write quota and records no activity. Not a guarantee the real write succeeds: file-kind and exclude-glob enforcement live below the commit path." },
                    "expected_hashes": { "type": ["object", "null"], "additionalProperties": { "type": "string" }, "description": "Optional per-path guard: the sha256 a previous read_file returned for that path. Any mismatch aborts the whole call with content_diverged and commits nothing." }
                },
                "additionalProperties": false
            })),
            true,
        ),
        write_tool(
            "move_file",
            "Move or rename a text file within a vault in a single commit, preserving git history. Fails if the target exists or the source is binary.",
            object(serde_json::json!({
                "type": "object",
                "required": ["vault_id", "parent_commit", "from", "to"],
                "properties": {
                    "vault_id": { "type": "string" },
                    "parent_commit": { "type": "string" },
                    "from": { "type": "string" },
                    "to": { "type": "string" },
                    "dry_run": { "type": ["boolean", "null"], "description": "Validate and report the rename (would_write) without committing. Consumes no write quota and records no activity." },
                    "expected_hashes": { "type": ["object", "null"], "additionalProperties": { "type": "string" }, "description": "Optional guard for the source path: the sha256 a previous read_file returned for `from`. A mismatch aborts with content_diverged." }
                },
                "additionalProperties": false
            })),
            true,
        ),
    ]
});

pub fn tool_definitions() -> Vec<Tool> {
    TOOL_DEFINITIONS.clone()
}

async fn apply_write_tool(
    state: &AppState,
    user: &AuthenticatedUser,
    input: WriteToolRequest,
) -> Result<WriteToolOutput> {
    // Single-change tools delegate to the batch path entirely, dry run
    // included (R4-1/R4-2): this wrapper used to build its own dry-run
    // answer with a hardcoded `change: "upsert"`, which reported a delete as
    // an upsert, and duplicated the batch's guard sequence. Deriving the
    // report from the same `PushChange` mapping the batch uses makes that
    // class of divergence structurally impossible. The hash guard still runs
    // in the batch path, after the CAS preflight: an out-of-date
    // `parent_commit` must keep winning as a Conflict (the client has to
    // re-read anyway), and only then does a stale file reject as
    // `content_diverged`.
    let changes = vec![input.change];
    apply_write_batch(
        state,
        user,
        WriteBatchRequest {
            vault_id: input.vault_id,
            parent_commit: input.parent_commit,
            activity_action: input.activity_action,
            activity_tool: input.activity_tool,
            activity_path: input.activity_path,
            activity_size_bytes: input.activity_size_bytes,
            would_write: would_write_from_changes(&changes),
            changes,
            dry_run: input.dry_run,
            expected_hashes: input.expected_hashes,
        },
    )
    .await
}

async fn apply_write_batch(
    state: &AppState,
    user: &AuthenticatedUser,
    input: WriteBatchRequest,
) -> Result<WriteToolOutput> {
    if input.dry_run {
        if let Some(conflict) = ensure_write_ready_without_rate_limit(
            state,
            user,
            &input.vault_id,
            &input.parent_commit,
        )
        .await?
        {
            return Ok(conflict);
        }
        let at = (!input.parent_commit.is_empty()).then_some(input.parent_commit.as_str());
        verify_expected_hashes(state, &input.vault_id, at, input.expected_hashes.as_ref()).await?;
        return Ok(dry_run_output(&input.parent_commit, input.would_write));
    }
    if let Some(conflict) =
        apply_write_preflight(state, user, &input.vault_id, input.parent_commit.clone()).await?
    {
        return Ok(conflict);
    }
    // Checked after the preflight (so an out-of-date parent still wins as a
    // Conflict) and before the push, so a diverged file aborts the batch
    // without consuming the CAS attempt.
    let at = (!input.parent_commit.is_empty()).then_some(input.parent_commit.as_str());
    verify_expected_hashes(state, &input.vault_id, at, input.expected_hashes.as_ref()).await?;
    apply_preflighted_write_batch(state, user, input).await
}

/// The one place that turns a `PushChange` into the dry-run report line.
/// Single-change and batch tools both go through it, so a delete can never be
/// reported as an upsert by a second, divergent mapping (R4-1/R4-2).
fn would_write_from_changes(changes: &[PushChange]) -> Vec<WouldWrite> {
    changes
        .iter()
        .map(|change| match change {
            PushChange::Text { path, content } => WouldWrite {
                path: path.clone(),
                change: "upsert",
                old_path: None,
                size_bytes: content.len(),
            },
            PushChange::Delete { path } => WouldWrite {
                path: path.clone(),
                change: "delete",
                old_path: None,
                size_bytes: 0,
            },
            // The MCP batch builder only produces text and delete changes;
            // blob writes arrive through the sync API, not this tool.
            PushChange::Blob { path, .. } => WouldWrite {
                path: path.clone(),
                change: "upsert",
                old_path: None,
                size_bytes: 0,
            },
        })
        .collect()
}

fn dry_run_output(parent_commit: &str, would_write: Vec<WouldWrite>) -> WriteToolOutput {
    WriteToolOutput::DryRun {
        dry_run: true,
        would_write,
        parent_commit: parent_commit.to_string(),
    }
}

async fn apply_write_preflight(
    state: &AppState,
    user: &AuthenticatedUser,
    vault_id: &str,
    parent_commit: String,
) -> Result<Option<WriteToolOutput>> {
    record_mcp_write_rate_limit(state, user, vault_id)?;
    ensure_write_ready_without_rate_limit(state, user, vault_id, &parent_commit).await
}

fn record_mcp_write_rate_limit(
    state: &AppState,
    user: &AuthenticatedUser,
    vault_id: &str,
) -> Result<()> {
    state
        .mcp_write_limiter
        .try_record(&user.token_id, vault_id)
        .map_err(|retry_after| {
            anyhow!(
                "rate_limited: mcp write rate limit (60/min) exceeded for this token+vault; retry in {}s",
                retry_after.as_secs().max(1)
            )
        })?;

    Ok(())
}

async fn ensure_write_ready_without_rate_limit(
    state: &AppState,
    user: &AuthenticatedUser,
    vault_id: &str,
    parent_commit: &str,
) -> Result<Option<WriteToolOutput>> {
    ensure_owned_vault(state, &user.user_id, vault_id).await?;
    let parent = (!parent_commit.is_empty()).then_some(parent_commit);
    let current_head = state
        .git_store()
        .head(vault_id)
        .await
        .map_err(git_error_to_anyhow)?;
    if current_head.as_deref() != parent {
        return Ok(Some(WriteToolOutput::Conflict {
            conflict: true,
            current_head,
        }));
    }

    Ok(None)
}

async fn apply_preflighted_write_batch(
    state: &AppState,
    user: &AuthenticatedUser,
    input: WriteBatchRequest,
) -> Result<WriteToolOutput> {
    let parent = (!input.parent_commit.is_empty()).then_some(input.parent_commit.as_str());
    match sync::push_with_cas(
        state,
        user,
        &input.vault_id,
        parent,
        RequestMetadata::default(),
        PushReq {
            changes: input.changes,
            device_name: Some("MCP".into()),
        },
    )
    .await
    .map_err(api_error_to_anyhow)?
    {
        Ok(resp) => {
            // L-9: the commit has already landed — an audit-trail write
            // failure must not turn a successful write into an error the
            // client might "compensate" for with a delete or rewrite.
            if let Err(err) = record_mcp_write_activity(
                state,
                user,
                &input.vault_id,
                input.activity_action,
                input.activity_tool,
                &input.activity_path,
                input.activity_size_bytes,
                &resp.new_commit,
            )
            .await
            {
                tracing::error!(error = %err, "mcp write activity recording failed");
            }
            Ok(WriteToolOutput::Success {
                commit: resp.new_commit,
            })
        }
        Err(conflict) => Ok(WriteToolOutput::Conflict {
            conflict: true,
            current_head: conflict.current_head,
        }),
    }
}

fn git_error_to_anyhow(error: GitStoreError) -> anyhow::Error {
    anyhow!("git: {error}")
}

/// Classify git-store failures on MCP READ surfaces into the whitelisted
/// public prefixes, so a client-correctable mistake (bad commit id, path
/// that names a directory, empty vault) is not masked as
/// `internal server error` (ultrareview M-5). Everything else keeps the
/// internal `git: …` shape and stays masked.
///
/// `spec` is the caller-facing identifier the user got wrong — the commit
/// for `at`/`commit` reads, or the path for head reads.
fn public_git_read_error(error: GitStoreError, spec: &str, reading_commit: bool) -> anyhow::Error {
    match &error {
        GitStoreError::Git(git_err) => {
            let message = git_err.message();
            // A path that names a tree entry: git2 reports
            // "the requested type does not match the type in the ODB"
            // (class Invalid, code NotFound).
            if message.contains("does not match the type") {
                return anyhow!("not_found: {spec}");
            }
            // An explicit commit spec that cannot be resolved — unparseable
            // or unknown — is a correctable client error. A missing file at a
            // valid commit returns Ok(None) instead of reaching here.
            if reading_commit
                || message.contains("OID")
                || git_err.code() == git2::ErrorCode::NotFound
            {
                return anyhow!("invalid_commit: {spec}");
            }
            anyhow!("git: {error}")
        }
        GitStoreError::NotFound => {
            if reading_commit {
                anyhow!("invalid_commit: {spec}")
            } else {
                anyhow!("not_found: {spec}")
            }
        }
        other => anyhow!("git: {other}"),
    }
}

#[allow(clippy::too_many_arguments)]
async fn record_mcp_write_activity(
    state: &AppState,
    user: &AuthenticatedUser,
    vault_id: &str,
    action: &'static str,
    tool: &'static str,
    path: &str,
    size_bytes: usize,
    commit: &str,
) -> Result<()> {
    #[derive(Serialize)]
    struct McpWriteDetails<'a> {
        /// Which MCP tool performed the write. The admin activity page shows
        /// it next to the device name, so a human can tell "an agent deleted
        /// this" from "the plugin synced a local delete".
        tool: &'a str,
        path: &'a str,
        commit: &'a str,
        size_bytes: usize,
    }
    let details = serde_json::to_string(&McpWriteDetails {
        tool,
        path,
        commit,
        size_bytes,
    })?;
    state
        .activities
        .insert(NewActivity {
            user_id: &user.user_id,
            vault_id: Some(vault_id),
            token_id: Some(&user.token_id),
            action,
            commit_hash: Some(commit),
            client_ip: None,
            user_agent: None,
            details: Some(&details),
        })
        .await?;
    Ok(())
}

async fn read_file_inner(
    state: &AppState,
    user_id: &str,
    vault_id: String,
    path: String,
    at: Option<String>,
) -> Result<ReadFileOutput> {
    ensure_owned_vault(state, user_id, &vault_id).await?;
    let git = state.git_store();
    let file = git
        .read_file(&vault_id, &path, at.as_deref())
        .await
        .map_err(|err| {
            // M-5: a directory target or an unknown commit spec is a
            // correctable client error, not an internal server error.
            public_git_read_error(err, at.as_deref().unwrap_or(&path), at.is_some())
        })?
        .ok_or_else(|| anyhow!("not_found: {path}"))?;
    render_file(state, &vault_id, path, file).await
}

async fn render_file(
    state: &AppState,
    vault_id: &str,
    path: String,
    file: StoredFile,
) -> Result<ReadFileOutput> {
    match file {
        StoredFile::Text { bytes } => render_bytes(path, bytes, None),
        StoredFile::BlobPointer { hash, mime, .. } => {
            if !state
                .blob_refs
                .is_referenced_by_vault(vault_id, &hash)
                .await?
            {
                bail!("blob not referenced by vault");
            }
            let blob = state.blob_store();
            let size = blob
                .size_bytes(&hash)
                .await?
                .ok_or_else(|| anyhow!("blob not found: {hash}"))?;
            if size > MCP_MAX_BINARY_RESPONSE_BYTES {
                bail!("file exceeds MCP response limit");
            }
            let bytes = blob
                .get(&hash)
                .await?
                .ok_or_else(|| anyhow!("blob not found: {hash}"))?;
            render_bytes(path, bytes.to_vec(), mime)
        }
    }
}

fn render_bytes(path: String, bytes: Vec<u8>, mime: Option<String>) -> Result<ReadFileOutput> {
    let is_text =
        TextClassifier::default_ref().is_text_path(&path) || mime_is_text(mime.as_deref());
    if is_text {
        match String::from_utf8(bytes) {
            Ok(content) => {
                return Ok(ReadFileOutput {
                    path,
                    is_binary: false,
                    sha256: sha256_hex(content.as_bytes()),
                    content,
                    mime,
                    encoding: Some("utf-8".into()),
                });
            }
            Err(err) => {
                let bytes = err.into_bytes();
                return binary_output(path, bytes, mime);
            }
        }
    }
    binary_output(path, bytes, mime)
}

fn binary_output(path: String, bytes: Vec<u8>, mime: Option<String>) -> Result<ReadFileOutput> {
    // L-7: every base64 path enforces the binary response cap — the
    // non-UTF8-text fallback used to skip the size check the BlobPointer
    // branch performed, letting a large binary-looking text file expand
    // into an unbounded response.
    let len = bytes.len() as u64;
    if len > MCP_MAX_BINARY_RESPONSE_BYTES {
        bail!(
            "binary response of {len} bytes exceeds the {MCP_MAX_BINARY_RESPONSE_BYTES}-byte limit"
        );
    }
    let sha256 = sha256_hex(&bytes);
    Ok(ReadFileOutput {
        path,
        is_binary: true,
        content: base64::engine::general_purpose::STANDARD.encode(bytes),
        mime,
        encoding: Some("base64".into()),
        sha256,
    })
}

fn mime_is_text(mime: Option<&str>) -> bool {
    matches!(
        mime,
        Some(mime)
            if mime.starts_with("text/")
                || matches!(mime, "application/json" | "application/xml" | "application/yaml")
    )
}

fn tool(
    name: &'static str,
    description: &'static str,
    input_schema: rmcp::model::JsonObject,
) -> Tool {
    Tool::new(name, description, input_schema).annotate(
        ToolAnnotations::new()
            .read_only(true)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
    )
}

fn write_tool(
    name: &'static str,
    description: &'static str,
    input_schema: rmcp::model::JsonObject,
    destructive: bool,
) -> Tool {
    Tool::new(name, description, input_schema).annotate(
        ToolAnnotations::new()
            .read_only(false)
            .destructive(destructive)
            .idempotent(false)
            .open_world(false),
    )
}

async fn ensure_owned_vault(state: &AppState, user_id: &str, vault_id: &str) -> Result<()> {
    ensure_user_vault(state, user_id, vault_id)
        .await
        .map(|_| ())
        .map_err(|e| anyhow!(e.message))
}

fn api_error_to_anyhow(error: ApiError) -> anyhow::Error {
    anyhow!("{}: {}", error.code, error.message)
}

/// Collects up to `batch` (path,size) entries whose cumulative size fits in
/// `remaining_budget`. Returns the chunk and whether collection stopped
/// because the budget was exhausted by the NEXT entry (either alone or
/// cumulatively).
///
/// The budget is decided from tree-entry sizes so oversized entries never
/// reach `read_files` and get materialized into memory (H-3: 64 entries x
/// 100 MiB blobs used to accumulate ~6.4 GiB before the budget check fired).
fn collect_budgeted_chunk(
    iter: &mut impl Iterator<Item = (String, u64)>,
    batch: usize,
    remaining_budget: u64,
) -> (Vec<(String, u64)>, bool) {
    let mut chunk: Vec<(String, u64)> = Vec::new();
    let mut used: u64 = 0;
    for entry in iter.by_ref() {
        let fits_alone = entry.1 <= remaining_budget;
        let fits_cumulatively = used.saturating_add(entry.1) <= remaining_budget;
        if !fits_alone || !fits_cumulatively {
            return (chunk, true);
        }
        used += entry.1;
        chunk.push(entry);
        if chunk.len() >= batch {
            break;
        }
    }
    (chunk, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::password;
    use crate::db::pool;
    use crate::db::repos::{NewToken, NewUser, RuntimeConfigRepo, TokenRepo, UserRepo};
    use crate::service::{vault, vault_settings};
    use crate::storage::blob::{BlobStore, LocalFsBlobStore};
    use crate::storage::git::read_probe;
    use crate::storage::git::{FileChange, GitVaultStore};
    use bytes::Bytes;

    async fn state_user_vault() -> (AppState, String, String, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let p = pool::connect_memory().await.unwrap();
        sqlx::migrate!("./migrations").run(&p).await.unwrap();
        let state = AppState::new(p, tmp.path().to_path_buf(), "t".into(), true)
            .await
            .unwrap();
        let user = state
            .users
            .create(NewUser {
                username: "u".into(),
                password_hash: password::hash("passw0rd!!").unwrap(),
                is_admin: false,
            })
            .await
            .unwrap();
        let vault = vault::create_vault(&state, &user.id, "main").await.unwrap();
        (state, user.id, vault.id, tmp)
    }

    async fn seed_two_files(
        state: &AppState,
        vault_id: &str,
        first_path: &str,
        second_path: &str,
    ) -> String {
        let git = Git2VaultStore::new(state.default_vault_root());
        git.commit_changes(
            vault_id,
            None,
            &[
                FileChange::Upsert {
                    path: first_path.into(),
                    file: StoredFile::Text {
                        bytes: b"visible needle".to_vec(),
                    },
                },
                FileChange::Upsert {
                    path: second_path.into(),
                    file: StoredFile::Text {
                        bytes: b"secret needle".to_vec(),
                    },
                },
            ],
            "seed files",
        )
        .await
        .unwrap()
    }

    /// The write path keys activity and the rate limiter on a real token row,
    /// so tests must authenticate like the transports do.
    async fn authenticated_user(state: &AppState, user_id: &str) -> AuthenticatedUser {
        let raw = crate::auth::token::generate();
        let token_row = state
            .tokens
            .create(NewToken {
                user_id,
                token_hash: &crate::auth::token::hash(&raw),
                device_id: "tools-test-device",
                device_name: "tools-test",
            })
            .await
            .unwrap();
        let user = state.users.find_by_id(user_id).await.unwrap().unwrap();
        AuthenticatedUser {
            user_id: user.id,
            username: user.username,
            is_admin: user.is_admin,
            token_id: token_row.id,
            device_id: token_row.device_id,
        }
    }

    async fn seed_text_files(state: &AppState, vault_id: &str, files: &[(&str, &str)]) -> String {
        let git = Git2VaultStore::new(state.default_vault_root());
        let changes = files
            .iter()
            .map(|(path, content)| FileChange::Upsert {
                path: (*path).into(),
                file: StoredFile::Text {
                    bytes: content.as_bytes().to_vec(),
                },
            })
            .collect::<Vec<_>>();
        git.commit_changes(vault_id, None, &changes, "seed text files")
            .await
            .unwrap()
    }

    async fn seed_raw_files(state: &AppState, vault_id: &str, files: &[(&str, Vec<u8>)]) -> String {
        let git = Git2VaultStore::new(state.default_vault_root());
        let changes = files
            .iter()
            .map(|(path, bytes)| FileChange::Upsert {
                path: (*path).into(),
                file: StoredFile::Text {
                    bytes: bytes.clone(),
                },
            })
            .collect::<Vec<_>>();
        git.commit_changes(vault_id, None, &changes, "seed raw files")
            .await
            .unwrap()
    }

    async fn set_vault_allowlist(state: &AppState, vault_id: &str, globs: Vec<String>) {
        vault_settings::save(
            state,
            vault_id,
            &vault_settings::VaultSettings {
                extra_sync_globs: globs,
            },
        )
        .await
        .unwrap();
    }

    async fn set_user_excludes(state: &AppState, excludes: Vec<String>) {
        state
            .runtime_cfg_repo
            .set_extra_exclude_globs(excludes, None)
            .await
            .unwrap();
        state
            .runtime_cfg
            .replace(state.runtime_cfg_repo.load().await.unwrap())
            .await;
    }

    #[tokio::test]
    async fn list_files_hides_filtered_path() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_two_files(&state, &vault_id, "note.md", "secret.md").await;
        set_user_excludes(&state, vec!["secret.md".into()]).await;

        let out = list_files(&state, &user_id, ListFilesInput { vault_id, at: None })
            .await
            .unwrap();

        assert!(out.paths.contains(&"note.md".to_string()));
        assert!(
            !out.paths.contains(&"secret.md".to_string()),
            "filtered path must be hidden"
        );
    }

    #[tokio::test]
    async fn list_files_fails_closed_when_vault_allowlist_cannot_compile() {
        // A vault allowlist that exceeds the compile limits used to be
        // silently replaced by an empty one, hiding every allowlisted
        // hidden file; reads must fail closed instead (ultrareview M-7).
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_two_files(&state, &vault_id, "note.md", ".obsidian/app.json").await;
        let too_many = (0..600)
            .map(|i| format!(".hidden{i}/**"))
            .collect::<Vec<_>>();
        set_vault_allowlist(&state, &vault_id, too_many).await;

        let err = list_files(&state, &user_id, ListFilesInput { vault_id, at: None })
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("extra_sync_globs"),
            "must surface the vault allowlist compile failure, got: {err}"
        );
    }

    #[tokio::test]
    async fn read_file_hides_filtered_path() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_two_files(&state, &vault_id, "note.md", "secret.md").await;
        set_user_excludes(&state, vec!["secret.md".into()]).await;

        let err = read_file(
            &state,
            &user_id,
            ReadFileInput {
                vault_id,
                path: "secret.md".into(),
            },
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string().contains("not found"),
            "must be not_found, got: {err}"
        );
    }

    #[tokio::test]
    async fn read_file_at_commit_hides_filtered_path() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let commit = seed_two_files(&state, &vault_id, "note.md", "secret.md").await;
        set_user_excludes(&state, vec!["secret.md".into()]).await;

        let err = read_file_at_commit(
            &state,
            &user_id,
            ReadFileAtCommitInput {
                vault_id,
                path: "secret.md".into(),
                commit,
            },
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string().contains("not found"),
            "must be not_found, got: {err}"
        );
    }

    #[tokio::test]
    async fn search_hides_filtered_path() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_two_files(&state, &vault_id, "note.md", "secret.md").await;
        set_user_excludes(&state, vec!["secret.md".into()]).await;

        let out = search(
            &state,
            &user_id,
            SearchInput {
                vault_id,
                query: "needle".into(),
                at: None,
                limit: None,
                path_prefix: None,
                frontmatter: None,
                after: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(out.matches.len(), 1);
        assert_eq!(out.matches[0].path, "note.md");
    }

    #[tokio::test]
    async fn search_file_cap_counts_only_visible_paths() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let mut changes = Vec::with_capacity(SEARCH_MAX_TREE_FILES + 1);
        for idx in 0..SEARCH_MAX_TREE_FILES {
            changes.push(FileChange::Upsert {
                path: format!("hidden/{idx}.md"),
                file: StoredFile::Text {
                    bytes: b"hidden needle".to_vec(),
                },
            });
        }
        changes.push(FileChange::Upsert {
            path: "note.md".into(),
            file: StoredFile::Text {
                bytes: b"visible needle".to_vec(),
            },
        });
        git.commit_changes(&vault_id, None, &changes, "seed many hidden files")
            .await
            .unwrap();
        set_user_excludes(&state, vec!["hidden/**".into()]).await;

        let out = search(
            &state,
            &user_id,
            SearchInput {
                vault_id,
                query: "needle".into(),
                at: None,
                limit: None,
                path_prefix: None,
                frontmatter: None,
                after: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(out.matches.len(), 1);
        assert_eq!(out.matches[0].path, "note.md");
    }

    #[tokio::test]
    async fn search_frontmatter_filter_uses_howto_shape() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        // The wiki page uses the howto's indented block list; the others have
        // a scalar kind, no frontmatter at all, or a nested-only key.
        seed_text_files(
            &state,
            &vault_id,
            &[
                (
                    "wiki/a.md",
                    "---\nkind: wiki\nsources:\n  - s1.md\n---\nneedle here",
                ),
                ("src/s1.md", "---\nkind: source\n---\nneedle here"),
                ("plain.md", "needle here"),
            ],
        )
        .await;
        let mut wanted = std::collections::HashMap::new();
        wanted.insert("kind".to_string(), serde_json::json!("wiki"));
        let out = search(
            &state,
            &user_id,
            SearchInput {
                vault_id: vault_id.clone(),
                query: "needle".into(),
                at: None,
                limit: None,
                path_prefix: None,
                frontmatter: Some(wanted),
                after: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(out.matches.len(), 1, "matches: {:?}", out.matches);
        assert_eq!(out.matches[0].path, "wiki/a.md");
        assert_eq!(out.next_after, None);

        // Existence form: `true` must accept the list-valued key too.
        let mut exists = std::collections::HashMap::new();
        exists.insert("sources".to_string(), serde_json::json!(true));
        let out = search(
            &state,
            &user_id,
            SearchInput {
                vault_id,
                query: "needle".into(),
                at: None,
                limit: None,
                path_prefix: None,
                frontmatter: Some(exists),
                after: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(out.matches.len(), 1);
        assert_eq!(out.matches[0].path, "wiki/a.md");
    }

    #[tokio::test]
    async fn search_pagination_tiles_the_match_set_exactly() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_text_files(
            &state,
            &vault_id,
            &[
                ("a.md", "needle 1\nneedle 2\nneedle 3\nneedle 4\nneedle 5\n"),
                ("b.md", "needle 6\n"),
            ],
        )
        .await;

        let first = search(
            &state,
            &user_id,
            SearchInput {
                vault_id: vault_id.clone(),
                query: "needle".into(),
                at: None,
                limit: Some(3),
                path_prefix: None,
                frontmatter: None,
                after: None,
            },
        )
        .await
        .unwrap();
        // `limit` stays a hard cap (no token-budget surprise)...
        assert_eq!(first.matches.len(), 3, "matches: {:?}", first.matches);
        assert!(first.next_after.is_some());

        let second = search(
            &state,
            &user_id,
            SearchInput {
                vault_id: vault_id.clone(),
                query: "needle".into(),
                at: None,
                limit: Some(3),
                path_prefix: None,
                frontmatter: None,
                after: first.next_after.clone(),
            },
        )
        .await
        .unwrap();
        let third = search(
            &state,
            &user_id,
            SearchInput {
                vault_id: vault_id.clone(),
                query: "needle".into(),
                at: None,
                limit: Some(3),
                path_prefix: None,
                frontmatter: None,
                after: second.next_after.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(third.next_after, None, "walk must terminate");

        // ...and the pages still tile the result exactly: every match line
        // once, in order, with nothing stranded at a page boundary.
        let mut paged: Vec<(String, usize)> = first
            .matches
            .iter()
            .chain(second.matches.iter())
            .chain(third.matches.iter())
            .map(|m| (m.path.clone(), m.line_number))
            .collect();
        paged.sort();
        assert_eq!(
            paged,
            vec![
                ("a.md".to_string(), 1),
                ("a.md".to_string(), 2),
                ("a.md".to_string(), 3),
                ("a.md".to_string(), 4),
                ("a.md".to_string(), 5),
                ("b.md".to_string(), 1),
            ],
            "pages must tile the match set exactly"
        );
    }

    #[tokio::test]
    async fn search_rejects_a_cursor_it_did_not_issue() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_text_files(&state, &vault_id, &[("a.md", "needle")]).await;
        let err = search(
            &state,
            &user_id,
            SearchInput {
                vault_id,
                query: "needle".into(),
                at: None,
                limit: None,
                path_prefix: None,
                frontmatter: None,
                after: Some("not-a-cursor".into()),
            },
        )
        .await
        .unwrap_err();
        assert!(err.to_string().starts_with("invalid_cursor:"), "got: {err}");
    }

    #[tokio::test]
    async fn search_path_prefix_scopes_the_tree_cap() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let mut changes = Vec::with_capacity(SEARCH_MAX_TREE_FILES + 1);
        for idx in 0..SEARCH_MAX_TREE_FILES {
            changes.push(FileChange::Upsert {
                path: format!("bulk/{idx}.md"),
                file: StoredFile::Text {
                    bytes: b"bulk filler".to_vec(),
                },
            });
        }
        changes.push(FileChange::Upsert {
            path: "wiki/one.md".into(),
            file: StoredFile::Text {
                bytes: b"needle".to_vec(),
            },
        });
        git.commit_changes(&vault_id, None, &changes, "seed bulk vault")
            .await
            .unwrap();

        // Unscoped, the same vault still trips the cap.
        let err = search(
            &state,
            &user_id,
            SearchInput {
                vault_id: vault_id.clone(),
                query: "needle".into(),
                at: None,
                limit: None,
                path_prefix: None,
                frontmatter: None,
                after: None,
            },
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("too many files to search"),
            "expected the tree cap, got: {err}"
        );

        // Scoped to the wiki layer, the cap counts what the call scans.
        let out = search(
            &state,
            &user_id,
            SearchInput {
                vault_id: vault_id.clone(),
                query: "needle".into(),
                at: None,
                limit: None,
                path_prefix: Some("wiki".into()),
                frontmatter: None,
                after: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(out.matches.len(), 1);
        assert_eq!(out.matches[0].path, "wiki/one.md");

        // A sibling directory is not under the prefix.
        let out = search(
            &state,
            &user_id,
            SearchInput {
                vault_id: vault_id.clone(),
                query: "needle".into(),
                at: None,
                limit: None,
                path_prefix: Some("wiki2".into()),
                frontmatter: None,
                after: None,
            },
        )
        .await
        .unwrap();
        assert!(out.matches.is_empty());
    }

    /// R4-3: an unsupported filter value must be refused, not silently
    /// matched against nothing. `{"draft": false}` used to return an empty
    /// result set, which reads as "no page is a draft" — a wrong answer with
    /// no error attached.
    #[tokio::test]
    async fn search_rejects_unsupported_frontmatter_filter_values() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_text_files(
            &state,
            &vault_id,
            &[("a.md", "---\nkind: wiki\n---\nneedle")],
        )
        .await;
        for bad in [
            serde_json::json!(false),
            serde_json::json!(1),
            serde_json::json!(null),
        ] {
            let mut wanted = std::collections::HashMap::new();
            wanted.insert("kind".to_string(), bad.clone());
            let err = search(
                &state,
                &user_id,
                SearchInput {
                    vault_id: vault_id.clone(),
                    query: "needle".into(),
                    at: None,
                    limit: None,
                    path_prefix: None,
                    frontmatter: Some(wanted),
                    after: None,
                },
            )
            .await
            .unwrap_err();
            assert!(
                err.to_string().contains("must be true or a string"),
                "value {bad} must be rejected up front, got: {err}"
            );
        }
    }

    #[tokio::test]
    async fn search_empty_query_with_frontmatter_returns_empty() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_text_files(&state, &vault_id, &[("a.md", "---\nkind: wiki\n---\nx")]).await;
        let mut wanted = std::collections::HashMap::new();
        wanted.insert("kind".to_string(), serde_json::json!("wiki"));
        let out = search(
            &state,
            &user_id,
            SearchInput {
                vault_id,
                query: String::new(),
                at: None,
                limit: None,
                path_prefix: None,
                frontmatter: Some(wanted),
                after: None,
            },
        )
        .await
        .unwrap();
        assert!(out.matches.is_empty());
        assert_eq!(out.next_after, None);
    }

    #[test]
    fn parse_links_extracts_wikilinks_and_md() {
        let raw = "see [[Target]] and [[Other#h|alias]] and ![[Embed]] and [x](sub/p.md) and [ext](https://a.com)";
        let links = extract_links(raw);
        assert_eq!(
            links,
            vec![
                "Target".to_string(),
                "Other".to_string(),
                "Embed".to_string(),
                "sub/p.md".to_string(),
            ]
        );
    }

    /// Build a page that declares `sources`/`source_commit` in the howto's
    /// indented shape.
    fn wiki_page(sources: &[&str], source_commit: &str) -> String {
        let mut page = String::from("---\nkind: wiki\nsources:\n");
        for source in sources {
            page.push_str(&format!("  - {source}\n"));
        }
        page.push_str(&format!("source_commit: {source_commit}\n---\n\n# Page\n"));
        page
    }

    #[tokio::test]
    async fn write_dry_run_reports_plan_and_commits_nothing() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let head = seed_text_files(&state, &vault_id, &[("a.md", "one")]).await;
        let auth = authenticated_user(&state, &user_id).await;

        let out = write_file(
            &state,
            &auth,
            WriteFileInput {
                vault_id: vault_id.clone(),
                path: "a.md".into(),
                content: "two".into(),
                parent_commit: head.clone(),
                dry_run: Some(true),
                expected_hashes: None,
            },
        )
        .await
        .unwrap();

        match out {
            WriteToolOutput::DryRun {
                dry_run,
                would_write,
                parent_commit,
            } => {
                assert!(dry_run);
                assert_eq!(parent_commit, head);
                assert_eq!(would_write.len(), 1);
                assert_eq!(would_write[0].path, "a.md");
                assert_eq!(would_write[0].change, "upsert");
                assert_eq!(would_write[0].size_bytes, 3);
            }
            other => panic!("expected a dry run, got: {other:?}"),
        }

        // Nothing landed: the head is unchanged and the content is intact.
        let git = Git2VaultStore::new(state.default_vault_root());
        assert_eq!(
            git.head(&vault_id).await.unwrap().as_deref(),
            Some(head.as_str())
        );
        let current = read_file(
            &state,
            &user_id,
            ReadFileInput {
                vault_id,
                path: "a.md".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(current.content, "one");
    }

    #[tokio::test]
    async fn write_dry_run_does_not_consume_the_rate_limit() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let head = seed_text_files(&state, &vault_id, &[("a.md", "one")]).await;
        let auth = authenticated_user(&state, &user_id).await;

        // Burn the whole window on dry runs: if any of them took a token the
        // real write below would be refused.
        for _ in 0..80 {
            write_file(
                &state,
                &auth,
                WriteFileInput {
                    vault_id: vault_id.clone(),
                    path: "a.md".into(),
                    content: "two".into(),
                    parent_commit: head.clone(),
                    dry_run: Some(true),
                    expected_hashes: None,
                },
            )
            .await
            .unwrap();
        }
        let real = write_file(
            &state,
            &auth,
            WriteFileInput {
                vault_id,
                path: "a.md".into(),
                content: "two".into(),
                parent_commit: head,
                dry_run: None,
                expected_hashes: None,
            },
        )
        .await
        .unwrap();
        assert!(
            matches!(real, WriteToolOutput::Success { .. }),
            "a real write after 80 dry runs must still be allowed (limit is 60/min): {real:?}"
        );
    }

    #[tokio::test]
    async fn write_rejects_a_diverged_file_without_committing() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let head = seed_text_files(&state, &vault_id, &[("a.md", "one")]).await;
        let auth = authenticated_user(&state, &user_id).await;

        let mut wrong = std::collections::HashMap::new();
        wrong.insert("a.md".to_string(), sha256_hex(b"something else"));
        let err = write_file(
            &state,
            &auth,
            WriteFileInput {
                vault_id: vault_id.clone(),
                path: "a.md".into(),
                content: "two".into(),
                parent_commit: head.clone(),
                dry_run: None,
                expected_hashes: Some(wrong),
            },
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().starts_with("content_diverged: path=a.md"),
            "got: {err}"
        );
        let git = Git2VaultStore::new(state.default_vault_root());
        assert_eq!(
            git.head(&vault_id).await.unwrap().as_deref(),
            Some(head.as_str()),
            "a refused write must not move the head"
        );

        // The right hash (what read_file hands out) goes through.
        let current = read_file(
            &state,
            &user_id,
            ReadFileInput {
                vault_id: vault_id.clone(),
                path: "a.md".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(current.sha256, sha256_hex(b"one"));
        let mut right = std::collections::HashMap::new();
        right.insert("a.md".to_string(), current.sha256);
        let ok = write_file(
            &state,
            &auth,
            WriteFileInput {
                vault_id,
                path: "a.md".into(),
                content: "two".into(),
                parent_commit: head,
                dry_run: None,
                expected_hashes: Some(right),
            },
        )
        .await
        .unwrap();
        assert!(matches!(ok, WriteToolOutput::Success { .. }), "got: {ok:?}");
    }

    #[tokio::test]
    async fn move_file_dry_run_reports_a_rename_and_takes_no_quota() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let head = seed_text_files(&state, &vault_id, &[("a.md", "one")]).await;
        let auth = authenticated_user(&state, &user_id).await;

        for _ in 0..80 {
            let out = move_file(
                &state,
                &auth,
                MoveFileInput {
                    vault_id: vault_id.clone(),
                    parent_commit: head.clone(),
                    from: "a.md".into(),
                    to: "b.md".into(),
                    dry_run: Some(true),
                    expected_hashes: None,
                },
            )
            .await
            .unwrap();
            match out {
                WriteToolOutput::DryRun { would_write, .. } => {
                    assert_eq!(would_write[0].change, "rename");
                    assert_eq!(would_write[0].path, "b.md");
                    assert_eq!(would_write[0].old_path.as_deref(), Some("a.md"));
                }
                other => panic!("expected a dry run, got: {other:?}"),
            }
        }
        // move_file used to record the limiter before its own body, so this
        // real move would have been refused had the dry runs consumed quota.
        let real = move_file(
            &state,
            &auth,
            MoveFileInput {
                vault_id,
                parent_commit: head,
                from: "a.md".into(),
                to: "b.md".into(),
                dry_run: None,
                expected_hashes: None,
            },
        )
        .await
        .unwrap();
        assert!(
            matches!(real, WriteToolOutput::Success { .. }),
            "the real move must still be allowed: {real:?}"
        );
    }

    /// The admin page can only attribute a write if the tool name actually
    /// reached the activity row. The page-level test inserts rows by hand, so
    /// this pins the write path itself, for all four write tools.
    #[tokio::test]
    async fn write_tools_record_the_tool_name_in_activity_details() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let head = seed_text_files(&state, &vault_id, &[("a.md", "one")]).await;
        let auth = authenticated_user(&state, &user_id).await;

        write_file(
            &state,
            &auth,
            WriteFileInput {
                vault_id: vault_id.clone(),
                path: "a.md".into(),
                content: "two".into(),
                parent_commit: head,
                dry_run: None,
                expected_hashes: None,
            },
        )
        .await
        .unwrap();
        let head = Git2VaultStore::new(state.default_vault_root())
            .head(&vault_id)
            .await
            .unwrap()
            .unwrap();
        write_files(
            &state,
            &auth,
            WriteFilesInput {
                vault_id: vault_id.clone(),
                parent_commit: head,
                writes: vec![FileWrite {
                    path: "b.md".into(),
                    content: "three".into(),
                }],
                deletes: vec!["a.md".into()],
                dry_run: None,
                expected_hashes: None,
            },
        )
        .await
        .unwrap();
        let head = Git2VaultStore::new(state.default_vault_root())
            .head(&vault_id)
            .await
            .unwrap()
            .unwrap();
        move_file(
            &state,
            &auth,
            MoveFileInput {
                vault_id: vault_id.clone(),
                parent_commit: head,
                from: "b.md".into(),
                to: "c.md".into(),
                dry_run: None,
                expected_hashes: None,
            },
        )
        .await
        .unwrap();
        let head = Git2VaultStore::new(state.default_vault_root())
            .head(&vault_id)
            .await
            .unwrap()
            .unwrap();
        delete_file(
            &state,
            &auth,
            DeleteFileInput {
                vault_id: vault_id.clone(),
                path: "c.md".into(),
                parent_commit: head,
                dry_run: None,
                expected_hashes: None,
            },
        )
        .await
        .unwrap();

        // Only the MCP rows: the sync layer also records a `push` row per
        // commit, which is not what this test is about.
        let rows: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT action, details FROM sync_activity
             WHERE vault_id = ? AND action IN ('mcp_write', 'mcp_delete')
             ORDER BY timestamp, rowid",
        )
        .bind(&vault_id)
        .fetch_all(&state.pool)
        .await
        .unwrap();
        let details: Vec<String> = rows
            .iter()
            .filter_map(|(_, details)| details.clone())
            .collect();
        assert_eq!(details.len(), 4, "one row per write: {rows:?}");
        for (details, tool) in
            details
                .iter()
                .zip(["write_file", "write_files", "move_file", "delete_file"])
        {
            assert!(
                details.contains(&format!("\"tool\":\"{tool}\"")),
                "row for {tool} must name it: {details}"
            );
        }
    }

    /// R4-1: a dry-run delete must report `change: "delete"`. The bug was
    /// that the single-change wrapper hardcoded `"upsert"` in its dry-run
    /// answer, so an agent asking "what would this do" about a delete was
    /// told "it would write the file" — the plan lied about the very thing
    /// the feature exists to preview.
    #[tokio::test]
    async fn delete_file_dry_run_reports_a_delete() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let head = seed_text_files(&state, &vault_id, &[("doomed.md", "bye")]).await;
        let auth = authenticated_user(&state, &user_id).await;

        let out = delete_file(
            &state,
            &auth,
            DeleteFileInput {
                vault_id: vault_id.clone(),
                path: "doomed.md".into(),
                parent_commit: head.clone(),
                dry_run: Some(true),
                expected_hashes: None,
            },
        )
        .await
        .unwrap();
        match out {
            WriteToolOutput::DryRun { would_write, .. } => {
                assert_eq!(would_write.len(), 1, "one change: {would_write:?}");
                assert_eq!(
                    would_write[0].change, "delete",
                    "a dry-run delete must not be reported as an upsert (R4-1)"
                );
                assert_eq!(would_write[0].path, "doomed.md");
            }
            other => panic!("expected a dry run, got: {other:?}"),
        }

        // And nothing landed.
        let git = Git2VaultStore::new(state.default_vault_root());
        assert_eq!(
            git.head(&vault_id).await.unwrap().as_deref(),
            Some(head.as_str())
        );
        assert!(git
            .read_file(&vault_id, "doomed.md", None)
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn write_files_accepts_the_legacy_deletes_shape() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let head = seed_text_files(&state, &vault_id, &[("a.md", "one"), ("b.md", "two")]).await;
        let auth = authenticated_user(&state, &user_id).await;

        // The unchanged `Vec<String>` deletes shape must keep working: the
        // guard is an additive top-level map, not a reshape of the batch.
        let out = write_files(
            &state,
            &auth,
            WriteFilesInput {
                vault_id: vault_id.clone(),
                parent_commit: head,
                writes: vec![FileWrite {
                    path: "c.md".into(),
                    content: "three".into(),
                }],
                deletes: vec!["a.md".into()],
                dry_run: None,
                expected_hashes: None,
            },
        )
        .await
        .unwrap();
        assert!(
            matches!(out, WriteToolOutput::Success { .. }),
            "got: {out:?}"
        );
        let git = Git2VaultStore::new(state.default_vault_root());
        assert!(git
            .read_file(&vault_id, "a.md", None)
            .await
            .unwrap()
            .is_none());
        assert!(git
            .read_file(&vault_id, "c.md", None)
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn stale_pages_reports_a_source_changed_after_the_anchor() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let base = git
            .commit_changes(
                &vault_id,
                None,
                &[
                    FileChange::Upsert {
                        path: "sources/s1.md".into(),
                        file: StoredFile::Text {
                            bytes: b"first".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "wiki/a.md".into(),
                        file: StoredFile::Text {
                            bytes: wiki_page(&["sources/s1.md"], "PLACEHOLDER").into_bytes(),
                        },
                    },
                ],
                "base",
            )
            .await
            .unwrap();
        // Rewrite the page with the real anchor, then touch the source.
        git.commit_changes(
            &vault_id,
            Some(&base),
            &[FileChange::Upsert {
                path: "wiki/a.md".into(),
                file: StoredFile::Text {
                    bytes: wiki_page(&["sources/s1.md"], &base).into_bytes(),
                },
            }],
            "anchor the page",
        )
        .await
        .unwrap();
        git.commit_changes(
            &vault_id,
            None,
            &[FileChange::Upsert {
                path: "sources/s1.md".into(),
                file: StoredFile::Text {
                    bytes: b"second".to_vec(),
                },
            }],
            "source moved on",
        )
        .await
        .unwrap();

        let out = stale_pages(
            &state,
            &user_id,
            StalePagesInput {
                vault_id: vault_id.clone(),
                path_prefix: None,
                at: None,
                limit: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(out.stale.len(), 1, "stale: {:?}", out.stale);
        assert_eq!(out.stale[0].page, "wiki/a.md");
        assert_eq!(out.stale[0].changed_sources, vec!["sources/s1.md"]);
        assert_eq!(out.stale[0].last_reconciled.as_deref(), Some(base.as_str()));
        assert!(out.stale[0].verified, "a small diff must be fully verified");
        assert!(out.missing_source_commit.is_empty());
        assert!(!out.truncated);
    }

    #[tokio::test]
    async fn stale_pages_reports_removed_and_renamed_sources() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let sources_commit = git
            .commit_changes(
                &vault_id,
                None,
                &[
                    FileChange::Upsert {
                        path: "sources/gone.md".into(),
                        file: StoredFile::Text {
                            bytes: b"gone".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "sources/moved.md".into(),
                        file: StoredFile::Text {
                            bytes: b"moved".to_vec(),
                        },
                    },
                ],
                "sources",
            )
            .await
            .unwrap();
        let base = git
            .commit_changes(
                &vault_id,
                Some(&sources_commit),
                &[FileChange::Upsert {
                    path: "wiki/a.md".into(),
                    file: StoredFile::Text {
                        bytes: wiki_page(&["sources/gone.md", "sources/moved.md"], &sources_commit)
                            .into_bytes(),
                    },
                }],
                "page anchored at the sources",
            )
            .await
            .unwrap();
        git.commit_changes(
            &vault_id,
            Some(&base),
            &[
                FileChange::Delete {
                    path: "sources/gone.md".into(),
                },
                // A move in git terms: the old path disappears and the same
                // content appears elsewhere, which rename detection pairs up.
                FileChange::Delete {
                    path: "sources/moved.md".into(),
                },
                FileChange::Upsert {
                    path: "sources/renamed.md".into(),
                    file: StoredFile::Text {
                        bytes: b"moved".to_vec(),
                    },
                },
            ],
            "delete one, move another",
        )
        .await
        .unwrap();

        let out = stale_pages(
            &state,
            &user_id,
            StalePagesInput {
                vault_id,
                path_prefix: None,
                at: None,
                limit: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(out.stale.len(), 1, "stale: {:?}", out.stale);
        let page = &out.stale[0];
        // The deletion and the move are both "this citation is dead": the
        // page still points at the old paths either way.
        assert_eq!(
            page.removed_sources,
            vec!["sources/gone.md", "sources/moved.md"]
        );
        assert_eq!(page.renamed.len(), 1, "page: {page:?}");
        assert_eq!(page.renamed[0].from, "sources/moved.md");
        assert_eq!(page.renamed[0].to, "sources/renamed.md");
    }

    #[tokio::test]
    async fn stale_pages_requires_an_anchor_and_skips_unusable_ones() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let base = git
            .commit_changes(
                &vault_id,
                None,
                &[
                    FileChange::Upsert {
                        path: "sources/s1.md".into(),
                        file: StoredFile::Text {
                            bytes: b"one".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "wiki/unanchored.md".into(),
                        file: StoredFile::Text {
                            bytes: b"---\nkind: wiki\nsources:\n  - sources/s1.md\n---\nbody"
                                .to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "wiki/bogus.md".into(),
                        file: StoredFile::Text {
                            bytes: wiki_page(&["sources/s1.md"], &"0".repeat(40)).into_bytes(),
                        },
                    },
                ],
                "base",
            )
            .await
            .unwrap();
        git.commit_changes(
            &vault_id,
            Some(&base),
            &[
                // Anchored at a commit that is *later* than the `at` we will
                // ask about, i.e. not an ancestor of it.
                FileChange::Upsert {
                    path: "wiki/late.md".into(),
                    file: StoredFile::Text {
                        bytes: wiki_page(&["sources/s1.md"], &base).into_bytes(),
                    },
                },
            ],
            "later",
        )
        .await
        .unwrap();

        let out = stale_pages(
            &state,
            &user_id,
            StalePagesInput {
                vault_id: vault_id.clone(),
                path_prefix: None,
                at: Some(base.clone()),
                limit: None,
            },
        )
        .await
        .unwrap();

        // Unanchored pages are reported, not guessed at.
        assert_eq!(out.missing_source_commit, vec!["wiki/unanchored.md"]);
        // The bogus anchor (unknown commit) and the not-yet-ancestor anchor
        // are counted, never silently treated as fresh.
        assert_eq!(out.skipped, 1, "bogus anchor must be skipped: {out:?}");
    }

    #[tokio::test]
    async fn stale_pages_reports_unresolvable_sources_instead_of_staying_quiet() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let sources_commit = git
            .commit_changes(
                &vault_id,
                None,
                &[FileChange::Upsert {
                    path: "sources/s1.md".into(),
                    file: StoredFile::Text {
                        bytes: b"one".to_vec(),
                    },
                }],
                "sources",
            )
            .await
            .unwrap();
        let base = git
            .commit_changes(
                &vault_id,
                Some(&sources_commit),
                &[FileChange::Upsert {
                    path: "wiki/a.md".into(),
                    file: StoredFile::Text {
                        // The second source never existed: a typo must
                        // surface, not hide behind a clean result.
                        bytes: wiki_page(&["sources/s1.md", "sources/typo.md"], &sources_commit)
                            .into_bytes(),
                    },
                }],
                "page anchored at the sources",
            )
            .await
            .unwrap();
        set_user_excludes(&state, vec!["sources/secret.md".into()]).await;

        let out = stale_pages(
            &state,
            &user_id,
            StalePagesInput {
                vault_id,
                path_prefix: None,
                at: Some(base),
                limit: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(out.stale.len(), 1, "stale: {:?}", out.stale);
        assert_eq!(
            out.stale[0].unresolved_sources,
            vec!["sources/typo.md"],
            "a nonexistent source must be reported"
        );
        assert!(out.stale[0].changed_sources.is_empty());
    }

    #[tokio::test]
    async fn stale_pages_truncates_at_the_page_limit() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let sources_commit = git
            .commit_changes(
                &vault_id,
                None,
                &[FileChange::Upsert {
                    path: "sources/s1.md".into(),
                    file: StoredFile::Text {
                        bytes: b"one".to_vec(),
                    },
                }],
                "sources",
            )
            .await
            .unwrap();
        let mut changes = Vec::new();
        for idx in 0..2 {
            changes.push(FileChange::Upsert {
                path: format!("wiki/p{idx}.md"),
                file: StoredFile::Text {
                    bytes: wiki_page(&["sources/s1.md"], &sources_commit).into_bytes(),
                },
            });
        }
        git.commit_changes(&vault_id, Some(&sources_commit), &changes, "pages")
            .await
            .unwrap();

        let out = stale_pages(
            &state,
            &user_id,
            StalePagesInput {
                vault_id,
                path_prefix: None,
                at: None,
                limit: Some(1),
            },
        )
        .await
        .unwrap();
        assert!(out.truncated, "a page-limit cut must be reported: {out:?}");
    }

    /// Performance gate for the spec's residual risk #2: 1000 commits with
    /// 200 anchored pages must stay well inside a tool-call budget. Ignored
    /// by default because it builds a thousand commits; run explicitly with
    /// `cargo test -p pkv-sync-server --lib -- --ignored stale_pages_perf`.
    ///
    /// Note: under `cfg(test)` the diff cap (LIST_CHANGES_MAX = 8) clips each
    /// page's listing early, so this is a *lower bound* for production, where
    /// a listing can carry up to 10k entries.
    #[tokio::test]
    #[ignore]
    async fn stale_pages_perf_1000_commits_200_pages() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());

        let mut seed = Vec::new();
        for idx in 0..200 {
            seed.push(FileChange::Upsert {
                path: format!("sources/s{idx}.md"),
                file: StoredFile::Text {
                    bytes: format!("source {idx}").into_bytes(),
                },
            });
        }
        let sources_commit = git
            .commit_changes(&vault_id, None, &seed, "sources")
            .await
            .unwrap();
        let mut pages = Vec::new();
        for idx in 0..200 {
            pages.push(FileChange::Upsert {
                path: format!("wiki/p{idx}.md"),
                file: StoredFile::Text {
                    bytes: wiki_page(&[&format!("sources/s{idx}.md")], &sources_commit)
                        .into_bytes(),
                },
            });
        }
        let mut head = git
            .commit_changes(&vault_id, Some(&sources_commit), &pages, "pages")
            .await
            .unwrap();
        // 800 more commits, each touching one source.
        for idx in 0..800 {
            let source = idx % 200;
            head = git
                .commit_changes(
                    &vault_id,
                    Some(&head),
                    &[FileChange::Upsert {
                        path: format!("sources/s{source}.md"),
                        file: StoredFile::Text {
                            bytes: format!("source {source} rev {idx}").into_bytes(),
                        },
                    }],
                    "touch",
                )
                .await
                .unwrap();
        }

        let started = std::time::Instant::now();
        let out = stale_pages(
            &state,
            &user_id,
            StalePagesInput {
                vault_id,
                path_prefix: None,
                at: None,
                limit: Some(STALE_PAGES_MAX),
            },
        )
        .await
        .unwrap();
        let elapsed = started.elapsed();
        println!(
            "stale_pages: {} pages, {} stale, truncated={}, elapsed={:?}",
            out.stale.len() + out.missing_source_commit.len(),
            out.stale.len(),
            out.truncated,
            elapsed
        );
        assert!(
            elapsed < std::time::Duration::from_secs(2),
            "stale_pages took {elapsed:?} for 1000 commits / 200 pages"
        );
    }

    /// A clipped listing must not make a page look fresh. Under `cfg(test)`
    /// LIST_CHANGES_MAX is 8, so one commit touching ten sources clips the
    /// listing; the pages whose source fell outside the clip have nothing to
    /// show, and must still come back marked unverified.
    #[tokio::test]
    async fn stale_pages_never_reports_a_clipped_page_as_clean() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let mut seed = Vec::new();
        for idx in 0..10 {
            seed.push(FileChange::Upsert {
                path: format!("sources/s{idx}.md"),
                file: StoredFile::Text {
                    bytes: format!("source {idx}").into_bytes(),
                },
            });
        }
        let sources_commit = git
            .commit_changes(&vault_id, None, &seed, "sources")
            .await
            .unwrap();
        let mut pages = Vec::new();
        for idx in 0..10 {
            pages.push(FileChange::Upsert {
                path: format!("wiki/p{idx}.md"),
                file: StoredFile::Text {
                    bytes: wiki_page(&[&format!("sources/s{idx}.md")], &sources_commit)
                        .into_bytes(),
                },
            });
        }
        git.commit_changes(&vault_id, Some(&sources_commit), &pages, "pages")
            .await
            .unwrap();
        // One commit touching every source: the resulting listing is clipped.
        let mut touch = Vec::new();
        for idx in 0..10 {
            touch.push(FileChange::Upsert {
                path: format!("sources/s{idx}.md"),
                file: StoredFile::Text {
                    bytes: format!("source {idx} rewritten").into_bytes(),
                },
            });
        }
        git.commit_changes(&vault_id, None, &touch, "touch everything")
            .await
            .unwrap();

        let out = stale_pages(
            &state,
            &user_id,
            StalePagesInput {
                vault_id,
                path_prefix: None,
                at: None,
                limit: None,
            },
        )
        .await
        .unwrap();

        assert!(out.truncated, "the listing was clipped: {out:?}");
        assert_eq!(
            out.stale.len(),
            10,
            "every page must be accounted for, including the ones the clip hid: {out:?}"
        );
        assert!(
            out.stale.iter().all(|page| !page.verified),
            "pages measured against a clipped listing are not verified"
        );
        assert!(
            out.stale.iter().any(|page| page.changed_sources.is_empty()),
            "at least one page was reported purely because it could not be verified"
        );
    }

    #[tokio::test]
    async fn stale_pages_ignores_pages_without_the_convention() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let base = git
            .commit_changes(
                &vault_id,
                None,
                &[
                    FileChange::Upsert {
                        path: "plain.md".into(),
                        file: StoredFile::Text {
                            bytes: b"no frontmatter".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "nested.md".into(),
                        file: StoredFile::Text {
                            bytes: b"---\nkind: wiki\nmeta:\n  sources:\n    - x.md\n---\nbody"
                                .to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "empty.md".into(),
                        file: StoredFile::Text {
                            bytes: b"---\nkind: wiki\nsources:\n---\nbody".to_vec(),
                        },
                    },
                ],
                "base",
            )
            .await
            .unwrap();
        let out = stale_pages(
            &state,
            &user_id,
            StalePagesInput {
                vault_id,
                path_prefix: None,
                at: Some(base),
                limit: None,
            },
        )
        .await
        .unwrap();
        assert!(out.stale.is_empty(), "stale: {:?}", out.stale);
        assert!(out.missing_source_commit.is_empty());
        assert_eq!(out.skipped, 0);
    }

    #[tokio::test]
    async fn stale_pages_is_not_stale_when_the_source_did_not_move() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let base = git
            .commit_changes(
                &vault_id,
                None,
                &[
                    FileChange::Upsert {
                        path: "sources/s1.md".into(),
                        file: StoredFile::Text {
                            bytes: b"stable".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "wiki/a.md".into(),
                        file: StoredFile::Text {
                            bytes: wiki_page(&["sources/s1.md"], "SELF").into_bytes(),
                        },
                    },
                ],
                "base",
            )
            .await
            .unwrap();
        let head = git
            .commit_changes(
                &vault_id,
                Some(&base),
                &[FileChange::Upsert {
                    path: "wiki/a.md".into(),
                    file: StoredFile::Text {
                        bytes: wiki_page(&["sources/s1.md"], &base).into_bytes(),
                    },
                }],
                "anchor",
            )
            .await
            .unwrap();
        let _ = head;

        let out = stale_pages(
            &state,
            &user_id,
            StalePagesInput {
                vault_id,
                path_prefix: None,
                at: None,
                limit: None,
            },
        )
        .await
        .unwrap();
        assert!(out.stale.is_empty(), "stale: {:?}", out.stale);
        assert!(out.missing_source_commit.is_empty());
    }

    #[tokio::test]
    async fn link_graph_kind_filter_selects_one_layer_without_false_broken_links() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_text_files(
            &state,
            &vault_id,
            &[
                // wiki layer: one page links inside the layer, one to a
                // source page, one to a genuinely missing page.
                (
                    "w/a.md",
                    "---\nkind: wiki\n---\n[[w/b]] [[s/raw]] [[w/ghost]]",
                ),
                ("w/b.md", "---\nkind: wiki\n---\nalone"),
                ("s/raw.md", "---\nkind: source\n---\nraw material"),
                ("n/plain.md", "no frontmatter at all"),
            ],
        )
        .await;

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id,
                at: None,
                path_prefix: None,
                limit: None,
                kind: Some("wiki".into()),
            },
        )
        .await
        .unwrap();

        let paths: Vec<&str> = out.nodes.iter().map(|node| node.path.as_str()).collect();
        assert_eq!(
            paths,
            vec!["w/a.md", "w/b.md"],
            "only the wiki layer is a node"
        );

        // The link to the source page resolves against the full tree, so it
        // must NOT be reported as broken...
        assert!(
            !out.broken.iter().any(|b| b.raw_link == "s/raw"),
            "a cross-layer link must not be called broken: {:?}",
            out.broken
        );
        // ...while a link to a page that truly does not exist still is.
        assert!(
            out.broken
                .iter()
                .any(|b| b.raw_link == "w/ghost" && b.reason == BrokenReason::Missing),
            "genuinely missing target must stay broken: {:?}",
            out.broken
        );
        // Orphans are computed inside the selected layer: a is unlinked,
        // b is linked from a, and the source/plain files are not nodes at
        // all.
        assert!(out.orphans.contains(&"w/a.md".to_string()));
        assert!(!out.orphans.contains(&"w/b.md".to_string()));
        assert!(!out.orphans.contains(&"s/raw.md".to_string()));
    }

    #[tokio::test]
    async fn link_graph_reports_links_orphans_broken_and_hides_filtered_paths() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_text_files(
            &state,
            &vault_id,
            &[
                ("index.md", "[[a]] [[ghost]] [[secret]]"),
                ("a.md", "linked"),
                ("orphan.md", "alone"),
                ("secret.md", "[[a]]"),
            ],
        )
        .await;
        set_user_excludes(&state, vec!["secret.md".into()]).await;

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id,
                at: None,
                path_prefix: None,
                limit: None,
                kind: None,
            },
        )
        .await
        .unwrap();

        let node_paths = out
            .nodes
            .iter()
            .map(|node| node.path.as_str())
            .collect::<Vec<_>>();
        assert!(node_paths.contains(&"index.md"));
        assert!(node_paths.contains(&"a.md"));
        assert!(node_paths.contains(&"orphan.md"));
        assert!(!node_paths.contains(&"secret.md"));

        let index = out
            .nodes
            .iter()
            .find(|node| node.path == "index.md")
            .unwrap();
        assert!(index.outlinks.contains(&"a.md".to_string()));
        assert!(!index.outlinks.contains(&"secret.md".to_string()));

        let a = out.nodes.iter().find(|node| node.path == "a.md").unwrap();
        assert_eq!(a.inlinks, 1);
        assert!(out.orphans.contains(&"orphan.md".to_string()));
        assert!(out.broken.iter().any(|broken| {
            broken.from == "index.md"
                && broken.raw_link == "ghost"
                && broken.reason == BrokenReason::Missing
        }));
        assert!(!out.truncated);
    }

    #[tokio::test]
    async fn link_graph_reports_ambiguous_basename_links() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_text_files(
            &state,
            &vault_id,
            &[
                ("index.md", "[[dup]]"),
                ("left/dup.md", "left"),
                ("right/dup.md", "right"),
            ],
        )
        .await;

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id,
                at: None,
                path_prefix: None,
                limit: None,
                kind: None,
            },
        )
        .await
        .unwrap();

        assert!(out.broken.iter().any(|broken| {
            broken.from == "index.md"
                && broken.raw_link == "dup"
                && broken.reason == BrokenReason::Ambiguous
        }));
    }

    #[tokio::test]
    async fn link_graph_normalizes_path_prefix() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_text_files(
            &state,
            &vault_id,
            &[
                ("docs/a.md", "[[b]]"),
                ("docs/b.md", "linked"),
                ("other.md", "outside prefix"),
            ],
        )
        .await;

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id,
                at: None,
                path_prefix: Some("./docs".into()),
                limit: None,
                kind: None,
            },
        )
        .await
        .unwrap();

        let node_paths = out
            .nodes
            .iter()
            .map(|node| node.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(node_paths, vec!["docs/a.md", "docs/b.md"]);
    }

    #[tokio::test]
    async fn link_graph_never_reports_hidden_paths_even_when_allowlisted() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_text_files(
            &state,
            &vault_id,
            &[
                ("index.md", "[[graph]]"),
                (".obsidian/graph.md", "[[index]]"),
            ],
        )
        .await;
        set_vault_allowlist(&state, &vault_id, vec![".obsidian/**".into()]).await;

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id,
                at: None,
                path_prefix: None,
                limit: None,
                kind: None,
            },
        )
        .await
        .unwrap();

        assert!(out.nodes.iter().any(|node| node.path == "index.md"));
        assert!(!out
            .nodes
            .iter()
            .any(|node| node.path == ".obsidian/graph.md"));
        assert!(out.broken.iter().any(|broken| {
            broken.from == "index.md"
                && broken.raw_link == "graph"
                && broken.reason == BrokenReason::Missing
        }));
    }

    #[tokio::test]
    async fn link_graph_ignores_non_text_paths() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_text_files(
            &state,
            &vault_id,
            &[("index.md", "[[data]]"), ("data.bin", "[[index]]")],
        )
        .await;

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id,
                at: None,
                path_prefix: None,
                limit: None,
                kind: None,
            },
        )
        .await
        .unwrap();

        assert!(out.nodes.iter().any(|node| node.path == "index.md"));
        assert!(!out.nodes.iter().any(|node| node.path == "data.bin"));
        assert!(out.broken.iter().any(|broken| {
            broken.from == "index.md"
                && broken.raw_link == "data"
                && broken.reason == BrokenReason::Missing
        }));
    }

    #[tokio::test]
    async fn link_graph_ignores_non_utf8_text_targets() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_raw_files(
            &state,
            &vault_id,
            &[
                ("index.md", b"[[bad]]".to_vec()),
                ("bad.md", vec![0xff, 0xfe, 0xfd]),
            ],
        )
        .await;

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id,
                at: None,
                path_prefix: None,
                limit: None,
                kind: None,
            },
        )
        .await
        .unwrap();

        assert!(out.nodes.iter().any(|node| node.path == "index.md"));
        assert!(!out.nodes.iter().any(|node| node.path == "bad.md"));
        assert!(out.broken.iter().any(|broken| {
            broken.from == "index.md"
                && broken.raw_link == "bad"
                && broken.reason == BrokenReason::Missing
        }));
    }

    #[tokio::test]
    async fn link_graph_resolves_markdown_links_relative_to_source_directory() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_text_files(
            &state,
            &vault_id,
            &[
                ("docs/index.md", "[child](child/page.md)"),
                ("docs/child/page.md", "expected"),
                ("child/page.md", "wrong root target"),
            ],
        )
        .await;

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id,
                at: None,
                path_prefix: None,
                limit: None,
                kind: None,
            },
        )
        .await
        .unwrap();

        let index = out
            .nodes
            .iter()
            .find(|node| node.path == "docs/index.md")
            .unwrap();
        assert!(index.outlinks.contains(&"docs/child/page.md".to_string()));
        assert!(!index.outlinks.contains(&"child/page.md".to_string()));
    }

    #[tokio::test]
    async fn link_graph_resolves_parent_markdown_links_relative_to_source_directory() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_text_files(
            &state,
            &vault_id,
            &[
                ("docs/sub/index.md", "[parent](../target.md)"),
                ("docs/target.md", "expected"),
                ("other/target.md", "ambiguous fallback target"),
            ],
        )
        .await;

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id,
                at: None,
                path_prefix: None,
                limit: None,
                kind: None,
            },
        )
        .await
        .unwrap();

        let index = out
            .nodes
            .iter()
            .find(|node| node.path == "docs/sub/index.md")
            .unwrap();
        assert!(index.outlinks.contains(&"docs/target.md".to_string()));
        assert!(!out.broken.iter().any(|broken| {
            broken.from == "docs/sub/index.md" && broken.raw_link == "../target.md"
        }));
    }

    #[tokio::test]
    async fn link_graph_does_not_fallback_parent_markdown_links_that_escape_vault_root() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_text_files(
            &state,
            &vault_id,
            &[
                ("docs/sub/index.md", "[escape](../../../target.md)"),
                ("target.md", "must not be resolved by basename fallback"),
            ],
        )
        .await;

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id,
                at: None,
                path_prefix: None,
                limit: None,
                kind: None,
            },
        )
        .await
        .unwrap();

        let index = out
            .nodes
            .iter()
            .find(|node| node.path == "docs/sub/index.md")
            .unwrap();
        assert!(!index.outlinks.contains(&"target.md".to_string()));
        assert!(out.broken.iter().any(|broken| {
            broken.from == "docs/sub/index.md"
                && broken.raw_link == "../../../target.md"
                && broken.reason == BrokenReason::Missing
        }));
    }

    #[tokio::test]
    async fn link_graph_caps_outlinks_per_file_and_sets_truncated() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let spam = "[[target]]".repeat(MAX_LINKS_PER_FILE + 5);
        seed_text_files(
            &state,
            &vault_id,
            &[("spam.md", &spam), ("target.md", "exists")],
        )
        .await;

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id,
                at: None,
                path_prefix: None,
                limit: None,
                kind: None,
            },
        )
        .await
        .unwrap();

        let spam_node = out
            .nodes
            .iter()
            .find(|node| node.path == "spam.md")
            .unwrap();
        assert_eq!(
            spam_node.outlinks.len(),
            MAX_LINKS_PER_FILE,
            "outlinks must stop at the per-file cap"
        );
        assert!(out.truncated, "hitting the cap must set truncated");
    }

    #[tokio::test]
    async fn link_graph_caps_broken_links_and_sets_truncated() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let spam = (0..MAX_BROKEN_LINKS + 5)
            .map(|idx| format!("[[ghost{idx}]]"))
            .collect::<String>();
        seed_text_files(&state, &vault_id, &[("spam.md", &spam)]).await;

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id,
                at: None,
                path_prefix: None,
                limit: None,
                kind: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(
            out.broken.len(),
            MAX_BROKEN_LINKS,
            "broken must stop at the cap"
        );
        assert!(out.truncated);
    }

    #[tokio::test]
    async fn link_graph_under_caps_stays_untruncated() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let body = "[[target]]".repeat(MAX_LINKS_PER_FILE - 1);
        seed_text_files(
            &state,
            &vault_id,
            &[("links.md", &body), ("target.md", "exists")],
        )
        .await;

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id,
                at: None,
                path_prefix: None,
                limit: None,
                kind: None,
            },
        )
        .await
        .unwrap();
        assert!(!out.truncated);
    }

    #[test]
    fn path_under_prefix_rejects_sibling_directories() {
        assert!(path_under_prefix("notes", "notes"));
        assert!(path_under_prefix("notes/a.md", "notes"));
        assert!(!path_under_prefix("notes-old/a.md", "notes"));
        assert!(!path_under_prefix("notes2/x.md", "notes"));
        assert!(path_under_prefix("anything.md", ""));
    }

    #[tokio::test]
    async fn read_file_non_utf8_text_caps_base64_response() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        // A .md file whose bytes are invalid UTF-8 takes the base64 fallback;
        // past the (test) cap it must error instead of expanding.
        let big: Vec<u8> = vec![0xFF; (MCP_MAX_BINARY_RESPONSE_BYTES + 1) as usize];
        seed_raw_files(&state, &vault_id, &[("big.md", big)]).await;

        let err = read_file(
            &state,
            &user_id,
            ReadFileInput {
                vault_id,
                path: "big.md".into(),
            },
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("exceeds"),
            "expected size-limit error, got: {err}"
        );
    }

    #[tokio::test]
    async fn link_graph_limit_counts_utf8_graph_nodes() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        seed_raw_files(
            &state,
            &vault_id,
            &[
                ("a.md", vec![0xff, 0xfe, 0xfd]),
                ("b.md", b"valid".to_vec()),
                ("c.md", b"also valid".to_vec()),
            ],
        )
        .await;

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id,
                at: None,
                path_prefix: None,
                limit: Some(1),
                kind: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(out.nodes.len(), 1);
        assert_eq!(out.nodes[0].path, "b.md");
        assert!(out.truncated);
    }

    #[tokio::test]
    async fn changes_since_reports_changes_and_hides_filtered_paths() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let base = git
            .commit_changes(
                &vault_id,
                None,
                &[
                    FileChange::Upsert {
                        path: "notes/a.md".into(),
                        file: StoredFile::Text {
                            bytes: b"old".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "notes/delete.md".into(),
                        file: StoredFile::Text {
                            bytes: b"delete".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "old-name.md".into(),
                        file: StoredFile::Text {
                            bytes: b"same".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "secret.md".into(),
                        file: StoredFile::Text {
                            bytes: b"secret".to_vec(),
                        },
                    },
                ],
                "base",
            )
            .await
            .unwrap();
        let head = git
            .commit_changes(
                &vault_id,
                Some(&base),
                &[
                    FileChange::Upsert {
                        path: "notes/a.md".into(),
                        file: StoredFile::Text {
                            bytes: b"new".to_vec(),
                        },
                    },
                    FileChange::Delete {
                        path: "notes/delete.md".into(),
                    },
                    FileChange::Upsert {
                        path: "notes/c.md".into(),
                        file: StoredFile::Text {
                            bytes: b"added".to_vec(),
                        },
                    },
                    FileChange::Delete {
                        path: "old-name.md".into(),
                    },
                    FileChange::Upsert {
                        path: "notes/new-name.md".into(),
                        file: StoredFile::Text {
                            bytes: b"same".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "secret.md".into(),
                        file: StoredFile::Text {
                            bytes: b"new secret".to_vec(),
                        },
                    },
                ],
                "head",
            )
            .await
            .unwrap();
        set_user_excludes(&state, vec!["secret.md".into()]).await;

        let out = changes_since(
            &state,
            &user_id,
            ChangesSinceInput {
                vault_id,
                since_commit: base.clone(),
                path_prefix: None,
                limit: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(out.from_commit, base);
        assert_eq!(out.to_commit, head);
        assert!(out.changes.contains(&crate::storage::git::ChangedEntry {
            path: "notes/a.md".into(),
            status: crate::storage::git::ChangeStatus::Modified,
            old_path: None,
        }));
        assert!(out.changes.contains(&crate::storage::git::ChangedEntry {
            path: "notes/delete.md".into(),
            status: crate::storage::git::ChangeStatus::Deleted,
            old_path: None,
        }));
        assert!(out.changes.contains(&crate::storage::git::ChangedEntry {
            path: "notes/c.md".into(),
            status: crate::storage::git::ChangeStatus::Added,
            old_path: None,
        }));
        assert!(out.changes.contains(&crate::storage::git::ChangedEntry {
            path: "notes/new-name.md".into(),
            status: crate::storage::git::ChangeStatus::Renamed,
            old_path: Some("old-name.md".into()),
        }));
        assert!(!out.changes.iter().any(|change| change.path == "secret.md"));
        assert!(!out.truncated);
    }

    #[tokio::test]
    async fn changes_since_hides_renames_from_filtered_old_paths() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let base = git
            .commit_changes(
                &vault_id,
                None,
                &[FileChange::Upsert {
                    path: "secret.md".into(),
                    file: StoredFile::Text {
                        bytes: b"same".to_vec(),
                    },
                }],
                "base",
            )
            .await
            .unwrap();
        git.commit_changes(
            &vault_id,
            Some(&base),
            &[
                FileChange::Delete {
                    path: "secret.md".into(),
                },
                FileChange::Upsert {
                    path: "public.md".into(),
                    file: StoredFile::Text {
                        bytes: b"same".to_vec(),
                    },
                },
            ],
            "rename secret",
        )
        .await
        .unwrap();
        set_user_excludes(&state, vec!["secret.md".into()]).await;

        let out = changes_since(
            &state,
            &user_id,
            ChangesSinceInput {
                vault_id,
                since_commit: base,
                path_prefix: None,
                limit: None,
            },
        )
        .await
        .unwrap();

        assert!(
            out.changes.is_empty(),
            "renames from filtered old paths must be hidden: {:?}",
            out.changes
        );
    }

    #[tokio::test]
    async fn changes_since_applies_path_prefix_and_limit() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let base = git
            .commit_changes(
                &vault_id,
                None,
                &[FileChange::Upsert {
                    path: "seed.md".into(),
                    file: StoredFile::Text {
                        bytes: b"seed".to_vec(),
                    },
                }],
                "base",
            )
            .await
            .unwrap();
        let head = git
            .commit_changes(
                &vault_id,
                Some(&base),
                &[
                    FileChange::Upsert {
                        path: "notes/a.md".into(),
                        file: StoredFile::Text {
                            bytes: b"a".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "notes/b.md".into(),
                        file: StoredFile::Text {
                            bytes: b"b".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "other/c.md".into(),
                        file: StoredFile::Text {
                            bytes: b"c".to_vec(),
                        },
                    },
                ],
                "head",
            )
            .await
            .unwrap();

        let out = changes_since(
            &state,
            &user_id,
            ChangesSinceInput {
                vault_id,
                since_commit: base.clone(),
                path_prefix: Some("notes/".into()),
                limit: Some(1),
            },
        )
        .await
        .unwrap();

        assert_eq!(out.from_commit, base);
        assert_eq!(out.to_commit, head);
        assert_eq!(out.changes.len(), 1);
        assert!(out.changes[0].path.starts_with("notes/"));
        assert!(out.truncated);
    }

    #[tokio::test]
    async fn changes_since_rejects_unrelated_commit() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let base = git
            .commit_changes(
                &vault_id,
                None,
                &[FileChange::Upsert {
                    path: "base.md".into(),
                    file: StoredFile::Text {
                        bytes: b"base".to_vec(),
                    },
                }],
                "base",
            )
            .await
            .unwrap();
        let head = git
            .commit_changes(
                &vault_id,
                Some(&base),
                &[FileChange::Upsert {
                    path: "head.md".into(),
                    file: StoredFile::Text {
                        bytes: b"head".to_vec(),
                    },
                }],
                "head",
            )
            .await
            .unwrap();
        git.set_main_ref(&vault_id, &base, "rewind for sibling")
            .await
            .unwrap();
        let unrelated = git
            .commit_changes(
                &vault_id,
                Some(&base),
                &[FileChange::Upsert {
                    path: "sibling.md".into(),
                    file: StoredFile::Text {
                        bytes: b"sibling".to_vec(),
                    },
                }],
                "sibling",
            )
            .await
            .unwrap();
        git.set_main_ref(&vault_id, &head, "restore head")
            .await
            .unwrap();

        let err = changes_since(
            &state,
            &user_id,
            ChangesSinceInput {
                vault_id,
                since_commit: unrelated,
                path_prefix: None,
                limit: None,
            },
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("unrelated_commit"));
    }

    #[test]
    fn parse_links_ignores_external_and_absolute_markdown_links() {
        let raw = "[a](HTTPS://example.com) [b](file:///tmp/x.md) [c](C:/vault/x.md) [d](\\\\server\\share\\x.md) [e](../relative.md)";

        assert_eq!(extract_links(raw), vec!["../relative.md".to_string()]);
    }

    #[tokio::test]
    async fn search_caps_requested_limit() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let content = (0..600)
            .map(|i| format!("needle line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        git.commit_changes(
            &vault_id,
            None,
            &[FileChange::Upsert {
                path: "notes.md".into(),
                file: StoredFile::Text {
                    bytes: content.into_bytes(),
                },
            }],
            "seed",
        )
        .await
        .unwrap();

        let result = search(
            &state,
            &user_id,
            SearchInput {
                vault_id,
                query: "needle".into(),
                at: None,
                limit: Some(10_000),
                path_prefix: None,
                frontmatter: None,
                after: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(result.matches.len(), SEARCH_MAX_LIMIT);
    }

    #[tokio::test]
    async fn search_rejects_total_text_budget_overrun() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        git.commit_changes(
            &vault_id,
            None,
            &[FileChange::Upsert {
                path: "large.md".into(),
                file: StoredFile::Text {
                    bytes: vec![b'a'; SEARCH_MAX_TOTAL_BYTES + 1],
                },
            }],
            "seed",
        )
        .await
        .unwrap();

        let err = search(
            &state,
            &user_id,
            SearchInput {
                vault_id,
                query: "needle".into(),
                at: None,
                limit: None,
                path_prefix: None,
                frontmatter: None,
                after: None,
            },
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("search content budget exceeded"));
    }

    // H-3 user-visible contract across the budget boundary: with the cfg(test)
    // budget of 64 KiB, two exact-fit 32 KiB files plus a small third file
    // containing the query overrun the budget at the third file, and the
    // request must fail with the budget message. This pins ONLY that
    // externally visible error, which is byte-identical before and after the
    // H-3 fix (post-read accumulation vs pre-read gating on tree entry
    // sizes) — it says nothing about what was materialized. The memory bound
    // itself is pinned by `search_oversized_entry_never_reaches_a_read_batch`
    // below (T-2/T-9).
    #[tokio::test]
    async fn search_budget_overrun_at_next_file_errors() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        git.commit_changes(
            &vault_id,
            None,
            &[
                FileChange::Upsert {
                    path: "a.md".into(),
                    file: StoredFile::Text {
                        bytes: vec![b'a'; 32 * 1024],
                    },
                },
                FileChange::Upsert {
                    path: "b.md".into(),
                    file: StoredFile::Text {
                        bytes: vec![b'b'; 32 * 1024],
                    },
                },
                FileChange::Upsert {
                    path: "c.md".into(),
                    file: StoredFile::Text {
                        bytes: b"needle at the end".to_vec(),
                    },
                },
            ],
            "seed",
        )
        .await
        .unwrap();

        let err = search(
            &state,
            &user_id,
            SearchInput {
                vault_id,
                query: "needle".into(),
                at: None,
                limit: None,
                path_prefix: None,
                frontmatter: None,
                after: None,
            },
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string().contains("search content budget exceeded"),
            "expected budget error, got: {err}"
        );
    }

    /// T-2: H-3's invariant is a memory bound, not an error message. An entry
    /// larger than the whole content budget must never be handed to
    /// `read_files`, because `read_files` materializes every path it is given
    /// (that is how 64 x 100 MiB reached ~6.4 GiB before the budget fired).
    /// The user-visible error cannot pin this: post-read accumulation emits
    /// the same "search content budget exceeded". The read batches themselves
    /// are observed through the test-only probe in `storage::git::read_probe`
    /// (see its docs for why a recording `GitVaultStore` decorator cannot be
    /// injected here).
    #[tokio::test]
    async fn search_oversized_entry_never_reaches_a_read_batch() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        git.commit_changes(
            &vault_id,
            None,
            &[
                FileChange::Upsert {
                    path: "small.md".into(),
                    file: StoredFile::Text {
                        bytes: b"needle".to_vec(),
                    },
                },
                FileChange::Upsert {
                    path: "zz-oversized.md".into(),
                    file: StoredFile::Text {
                        bytes: vec![b'x'; SEARCH_MAX_TOTAL_BYTES + 1],
                    },
                },
            ],
            "seed",
        )
        .await
        .unwrap();

        let err = search(
            &state,
            &user_id,
            SearchInput {
                vault_id: vault_id.clone(),
                query: "needle".into(),
                at: None,
                limit: None,
                path_prefix: None,
                frontmatter: None,
                after: None,
            },
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("search content budget exceeded"),
            "expected budget error, got: {err}"
        );

        let oversized_batches = read_probe::batches_containing("zz-oversized.md");
        assert!(
            oversized_batches.is_empty(),
            "an entry larger than the whole budget must never enter a read batch (H-3); \
             observed read_files batches containing it: {oversized_batches:?}"
        );

        // Positive control: the probe must actually observe read batches,
        // otherwise the assertion above would also hold if nothing were
        // recorded at all.
        let control_vault = vault::create_vault(&state, &user_id, "control")
            .await
            .unwrap();
        git.commit_changes(
            &control_vault.id,
            None,
            &[FileChange::Upsert {
                path: "control.md".into(),
                file: StoredFile::Text {
                    bytes: b"needle".to_vec(),
                },
            }],
            "seed",
        )
        .await
        .unwrap();
        let control = search(
            &state,
            &user_id,
            SearchInput {
                vault_id: control_vault.id,
                query: "needle".into(),
                at: None,
                limit: None,
                path_prefix: None,
                frontmatter: None,
                after: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(control.matches.len(), 1);
        assert!(
            !read_probe::batches_containing("control.md").is_empty(),
            "control: the read probe must record read_files batches"
        );
    }

    /// R3-5: `link_graph` shares H-3's guard with `search`, and T-2 only
    /// pinned the `search` call site — reverting `link_graph`'s call to
    /// "hand the candidate list to `read_files` first" left every test green,
    /// because `link_graph`'s end state is `truncated = true` rather than an
    /// error. Same probe, same positive control, second call site.
    #[tokio::test]
    async fn link_graph_oversized_entry_never_reaches_a_read_batch() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        git.commit_changes(
            &vault_id,
            None,
            &[
                FileChange::Upsert {
                    path: "index.md".into(),
                    file: StoredFile::Text {
                        bytes: b"[[a]]".to_vec(),
                    },
                },
                FileChange::Upsert {
                    path: "zz-oversized.md".into(),
                    file: StoredFile::Text {
                        bytes: vec![b'x'; LINK_GRAPH_MAX_TOTAL_BYTES + 1],
                    },
                },
            ],
            "seed",
        )
        .await
        .unwrap();

        let out = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id: vault_id.clone(),
                at: None,
                path_prefix: None,
                limit: None,
                kind: None,
            },
        )
        .await
        .unwrap();
        assert!(
            out.truncated,
            "an entry larger than the whole budget must stop the scan"
        );

        let oversized_batches = read_probe::batches_containing("zz-oversized.md");
        assert!(
            oversized_batches.is_empty(),
            "an entry larger than the whole budget must never enter a read batch (H-3); \
             observed read_files batches containing it: {oversized_batches:?}"
        );

        // Positive control: the probe must actually observe this call site's
        // read batches, otherwise the assertion above would also hold if
        // nothing were recorded at all.
        let control_vault = vault::create_vault(&state, &user_id, "control")
            .await
            .unwrap();
        git.commit_changes(
            &control_vault.id,
            None,
            &[FileChange::Upsert {
                path: "control.md".into(),
                file: StoredFile::Text {
                    bytes: b"[[control]]".to_vec(),
                },
            }],
            "seed",
        )
        .await
        .unwrap();
        let control = link_graph(
            &state,
            &user_id,
            LinkGraphInput {
                vault_id: control_vault.id,
                at: None,
                path_prefix: None,
                limit: None,
                kind: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(control.nodes.len(), 1);
        assert!(
            !read_probe::batches_containing("control.md").is_empty(),
            "control: the read probe must record link_graph's read_files batches"
        );
    }

    #[test]
    fn collect_budgeted_chunk_rejects_entry_larger_than_whole_budget() {
        let mut iter = vec![("a.md".to_string(), (64 * 1024 + 1) as u64)].into_iter();
        let (chunk, budget_exhausted) =
            collect_budgeted_chunk(&mut iter, MCP_READ_BATCH_SIZE, (64 * 1024) as u64);
        assert!(chunk.is_empty(), "oversized entry must not be included");
        assert!(budget_exhausted);
    }

    #[test]
    fn collect_budgeted_chunk_stops_when_cumulative_total_would_exceed() {
        let mut iter = vec![
            ("a.md".to_string(), (24 * 1024) as u64),
            ("b.md".to_string(), (24 * 1024) as u64),
            ("c.md".to_string(), (24 * 1024) as u64),
        ]
        .into_iter();
        let (chunk, budget_exhausted) =
            collect_budgeted_chunk(&mut iter, MCP_READ_BATCH_SIZE, (64 * 1024) as u64);
        let paths: Vec<&str> = chunk.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(paths, vec!["a.md", "b.md"]);
        assert!(budget_exhausted);
    }

    #[test]
    fn collect_budgeted_chunk_exhausted_iterator_is_not_budget_exhaustion() {
        let mut iter = vec![
            ("a.md".to_string(), (24 * 1024) as u64),
            ("b.md".to_string(), (24 * 1024) as u64),
        ]
        .into_iter();
        let (chunk, budget_exhausted) =
            collect_budgeted_chunk(&mut iter, MCP_READ_BATCH_SIZE, (64 * 1024) as u64);
        assert_eq!(chunk.len(), 2);
        assert!(!budget_exhausted);
    }

    #[test]
    fn collect_budgeted_chunk_accepts_exact_fit_without_flagging_exhaustion() {
        let mut iter = vec![
            ("a.md".to_string(), (32 * 1024) as u64),
            ("b.md".to_string(), (32 * 1024) as u64),
        ]
        .into_iter();
        let (chunk, budget_exhausted) =
            collect_budgeted_chunk(&mut iter, MCP_READ_BATCH_SIZE, (64 * 1024) as u64);
        assert_eq!(chunk.len(), 2);
        assert!(
            !budget_exhausted,
            "filling the budget exactly is not exhaustion"
        );
    }

    #[tokio::test]
    async fn read_file_rejects_blob_over_response_budget_before_loading() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let git = Git2VaultStore::new(state.default_vault_root());
        let blob = LocalFsBlobStore::new(state.default_blob_root());
        let bytes = Bytes::from(vec![b'x'; MCP_MAX_BINARY_RESPONSE_BYTES as usize + 1]);
        let hash = LocalFsBlobStore::sha256(&bytes);
        blob.put_verified(&hash, bytes.clone()).await.unwrap();
        state
            .blob_refs
            .add_refs(&vault_id, "seed", std::slice::from_ref(&hash))
            .await
            .unwrap();
        git.commit_changes(
            &vault_id,
            None,
            &[FileChange::Upsert {
                path: "large.bin".into(),
                file: StoredFile::BlobPointer {
                    hash: hash.clone(),
                    size: bytes.len() as u64,
                    mime: Some("application/octet-stream".into()),
                },
            }],
            "seed",
        )
        .await
        .unwrap();

        let err = read_file(
            &state,
            &user_id,
            ReadFileInput {
                vault_id,
                path: "large.bin".into(),
            },
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("file exceeds MCP response limit"));
    }

    #[tokio::test]
    async fn list_vaults_returns_ordered_summaries_with_heads() {
        let (state, user_id, first_vault_id, _tmp) = state_user_vault().await;
        let second_vault = vault::create_vault(&state, &user_id, "second")
            .await
            .unwrap();
        let git = Git2VaultStore::new(state.default_vault_root());
        let first_head = git
            .commit_changes(
                &first_vault_id,
                None,
                &[FileChange::Upsert {
                    path: "first.md".into(),
                    file: StoredFile::Text {
                        bytes: b"first".to_vec(),
                    },
                }],
                "seed first",
            )
            .await
            .unwrap();
        let second_head = git
            .commit_changes(
                &second_vault.id,
                None,
                &[FileChange::Upsert {
                    path: "second.md".into(),
                    file: StoredFile::Text {
                        bytes: b"second".to_vec(),
                    },
                }],
                "seed second",
            )
            .await
            .unwrap();

        let output = list_vaults(&state, &user_id).await.unwrap();

        assert_eq!(output.vaults.len(), 2);
        assert_eq!(output.vaults[0].id, first_vault_id);
        assert_eq!(output.vaults[0].name, "main");
        assert_eq!(
            output.vaults[0].head_commit.as_deref(),
            Some(first_head.as_str())
        );
        assert_eq!(output.vaults[1].id, second_vault.id);
        assert_eq!(output.vaults[1].name, "second");
        assert_eq!(
            output.vaults[1].head_commit.as_deref(),
            Some(second_head.as_str())
        );
    }

    #[tokio::test]
    async fn read_file_blob_requires_blob_ref_for_vault() {
        let (state, user_id, vault_id, _tmp) = state_user_vault().await;
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        let blob = LocalFsBlobStore::new(state.default_blob_root());
        blob.put_verified(&hash, data.clone()).await.unwrap();
        let git = Git2VaultStore::new(state.default_vault_root());
        let commit = git
            .commit_changes(
                &vault_id,
                None,
                &[FileChange::Upsert {
                    path: "img.png".into(),
                    file: StoredFile::BlobPointer {
                        hash: hash.clone(),
                        size: 5,
                        mime: Some("image/png".into()),
                    },
                }],
                "seed",
            )
            .await
            .unwrap();

        let err = read_file(
            &state,
            &user_id,
            ReadFileInput {
                vault_id: vault_id.clone(),
                path: "img.png".into(),
            },
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("blob not referenced by vault"));

        state
            .blob_refs
            .add_refs(&vault_id, &commit, std::slice::from_ref(&hash))
            .await
            .unwrap();
        let output = read_file(
            &state,
            &user_id,
            ReadFileInput {
                vault_id,
                path: "img.png".into(),
            },
        )
        .await
        .unwrap();

        assert!(output.is_binary);
        assert_eq!(output.encoding.as_deref(), Some("base64"));
    }
}
