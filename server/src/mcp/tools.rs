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
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

const SEARCH_MAX_TREE_FILES: usize = 5000;
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
}

#[derive(Debug, Deserialize)]
pub struct SearchInput {
    pub vault_id: String,
    pub query: String,
    pub at: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct SearchOutput {
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Serialize)]
pub struct SearchMatch {
    pub path: String,
    pub line: String,
    pub line_number: usize,
}

#[derive(Debug, Deserialize)]
pub struct LinkGraphInput {
    pub vault_id: String,
    pub at: Option<String>,
    pub path_prefix: Option<String>,
    pub limit: Option<usize>,
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
}

#[derive(Debug, Deserialize)]
pub struct DeleteFileInput {
    pub vault_id: String,
    pub path: String,
    pub parent_commit: String,
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
}

#[derive(Debug, Deserialize)]
pub struct MoveFileInput {
    pub vault_id: String,
    pub parent_commit: String,
    pub from: String,
    pub to: String,
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
}

struct WriteToolRequest {
    vault_id: String,
    parent_commit: String,
    activity_action: &'static str,
    activity_path: String,
    activity_size_bytes: usize,
    change: PushChange,
}

struct WriteBatchRequest {
    vault_id: String,
    parent_commit: String,
    activity_action: &'static str,
    activity_path: String,
    activity_size_bytes: usize,
    changes: Vec<PushChange>,
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
        .await?
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
        return Ok(SearchOutput {
            matches: Vec::new(),
        });
    }

    let git = state.git_store();
    let filter = sync::vault_path_filter(state, &input.vault_id)
        .await
        .map_err(api_error_to_anyhow)?;
    let tree = git.list_tree(&input.vault_id, input.at.as_deref()).await?;
    let visible_count = tree
        .iter()
        .filter(|entry| sync::path_visible_on_read(&filter, &entry.path))
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

    for entry in tree
        .into_iter()
        .filter(|entry| sync::path_visible_on_read(&filter, &entry.path))
    {
        if matches.len() >= limit {
            break;
        }
        if entry.is_blob_pointer || !classifier.is_text_path(&entry.path) {
            continue;
        }
        let entry_size = usize::try_from(entry.size).unwrap_or(usize::MAX);
        searched_bytes = searched_bytes.saturating_add(entry_size);
        if searched_bytes > SEARCH_MAX_TOTAL_BYTES {
            bail!("search content budget exceeded");
        }
        let Some(StoredFile::Text { bytes }) = git
            .read_file(&input.vault_id, &entry.path, input.at.as_deref())
            .await?
        else {
            continue;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        for (idx, line) in text.lines().enumerate() {
            if contains_ascii_case_insensitive(line, &needle) {
                let line = line.to_string();
                matches.push(SearchMatch {
                    path: entry.path.clone(),
                    line,
                    line_number: idx + 1,
                });
                if matches.len() >= limit {
                    break;
                }
            }
        }
    }

    Ok(SearchOutput { matches })
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
    for entry in tree.into_iter().filter(|entry| {
        !entry.is_blob_pointer
            && classifier.is_text_path(&entry.path)
            && entry.path.starts_with(&prefix)
            && !crate::service::exclude::is_hidden_path(&entry.path)
            && sync::path_visible_on_read(&filter, &entry.path)
    }) {
        if graph_files.len() >= limit {
            truncated = true;
            break;
        }
        let Some(text) =
            read_text_bytes(&git, &input.vault_id, &entry.path, input.at.as_deref()).await?
        else {
            continue;
        };
        scanned_bytes = scanned_bytes.saturating_add(text.len());
        if scanned_bytes > LINK_GRAPH_MAX_TOTAL_BYTES {
            truncated = true;
            break;
        }
        graph_files.push((entry.path.clone(), text));
    }

    let paths_by_path = graph_files
        .iter()
        .map(|(path, _)| path.clone())
        .collect::<HashSet<_>>();
    let mut by_basename: HashMap<String, Vec<String>> = HashMap::new();
    for (path, _) in &graph_files {
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
    let visible = git
        .list_changes_between(&input.vault_id, &input.since_commit, &head)
        .await?
        .into_iter()
        .filter(|change| {
            change.path.starts_with(&prefix)
                && mcp_path_visible(&filter, &change.path)
                && change
                    .old_path
                    .as_ref()
                    .is_none_or(|old_path| mcp_path_visible(&filter, old_path))
        });

    let mut changes = Vec::new();
    let mut truncated = false;
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

async fn read_text_bytes(
    git: &Git2VaultStore,
    vault_id: &str,
    path: &str,
    at: Option<&str>,
) -> Result<Option<String>> {
    let Some(StoredFile::Text { bytes }) = git.read_file(vault_id, path, at).await? else {
        return Ok(None);
    };
    Ok(String::from_utf8(bytes).ok())
}

pub async fn write_file(
    state: &AppState,
    user: &AuthenticatedUser,
    input: WriteFileInput,
) -> Result<WriteToolOutput> {
    let path = normalize_mcp_path(input.path)?;
    let size_bytes = input.content.len();
    let max_file_size = state.runtime_cfg.snapshot().await.max_file_size;
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
            activity_path: path.clone(),
            activity_size_bytes: size_bytes,
            change: PushChange::Text {
                path,
                content: input.content,
            },
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
            activity_path: path.clone(),
            activity_size_bytes: 0,
            change: PushChange::Delete { path },
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

    let max_file_size = state.runtime_cfg.snapshot().await.max_file_size;
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
            bail!("file '{path}' exceeds max_file_size of {max_file_size} bytes");
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

    apply_write_batch(
        state,
        user,
        WriteBatchRequest {
            vault_id: input.vault_id,
            parent_commit: input.parent_commit,
            activity_action: "mcp_write",
            activity_path: format!("{total} files"),
            activity_size_bytes: total_bytes,
            changes,
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
    let activity_size_bytes = content.len();

    apply_preflighted_write_batch(
        state,
        user,
        WriteBatchRequest {
            vault_id: input.vault_id,
            parent_commit: input.parent_commit,
            activity_action: "mcp_write",
            activity_path: format!("{from} -> {to}"),
            activity_size_bytes,
            changes: vec![
                PushChange::Delete { path: from },
                PushChange::Text { path: to, content },
            ],
        },
    )
    .await
}

fn normalize_mcp_path(raw_path: String) -> Result<String> {
    path::normalize(&raw_path).map_err(|err| anyhow!("invalid_path: {err}"))
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
            "Search text files in a vault with case-insensitive substring matching.",
            object(serde_json::json!({
                "type": "object",
                "required": ["vault_id", "query"],
                "properties": {
                    "vault_id": { "type": "string" },
                    "query": { "type": "string" },
                    "at": { "type": ["string", "null"] },
                    "limit": { "type": ["integer", "null"], "minimum": 1, "maximum": SEARCH_MAX_LIMIT }
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
                    "limit": { "type": ["integer", "null"], "minimum": 1, "maximum": LINK_GRAPH_MAX_NODES }
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
                    "parent_commit": { "type": "string" }
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
                    "parent_commit": { "type": "string" }
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
                    "deletes": { "type": "array", "items": { "type": "string" } }
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
                    "to": { "type": "string" }
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
    apply_write_batch(
        state,
        user,
        WriteBatchRequest {
            vault_id: input.vault_id,
            parent_commit: input.parent_commit,
            activity_action: input.activity_action,
            activity_path: input.activity_path,
            activity_size_bytes: input.activity_size_bytes,
            changes: vec![input.change],
        },
    )
    .await
}

async fn apply_write_batch(
    state: &AppState,
    user: &AuthenticatedUser,
    input: WriteBatchRequest,
) -> Result<WriteToolOutput> {
    if let Some(conflict) =
        apply_write_preflight(state, user, &input.vault_id, input.parent_commit.clone()).await?
    {
        return Ok(conflict);
    }
    apply_preflighted_write_batch(state, user, input).await
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
            record_mcp_write_activity(
                state,
                user,
                &input.vault_id,
                input.activity_action,
                &input.activity_path,
                input.activity_size_bytes,
                &resp.new_commit,
            )
            .await?;
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

async fn record_mcp_write_activity(
    state: &AppState,
    user: &AuthenticatedUser,
    vault_id: &str,
    action: &'static str,
    path: &str,
    size_bytes: usize,
    commit: &str,
) -> Result<()> {
    #[derive(Serialize)]
    struct McpWriteDetails<'a> {
        path: &'a str,
        commit: &'a str,
        size_bytes: usize,
    }
    let details = serde_json::to_string(&McpWriteDetails {
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
        .await?
        .ok_or_else(|| anyhow!("file not found"))?;
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
                    content,
                    mime,
                    encoding: Some("utf-8".into()),
                });
            }
            Err(err) => {
                let bytes = err.into_bytes();
                return Ok(binary_output(path, bytes, mime));
            }
        }
    }
    Ok(binary_output(path, bytes, mime))
}

fn binary_output(path: String, bytes: Vec<u8>, mime: Option<String>) -> ReadFileOutput {
    ReadFileOutput {
        path,
        is_binary: true,
        content: base64::engine::general_purpose::STANDARD.encode(bytes),
        mime,
        encoding: Some("base64".into()),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::password;
    use crate::db::pool;
    use crate::db::repos::{NewUser, RuntimeConfigRepo, UserRepo};
    use crate::service::{vault, vault_settings};
    use crate::storage::blob::{BlobStore, LocalFsBlobStore};
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
            },
        )
        .await
        .unwrap();

        assert_eq!(out.matches.len(), 1);
        assert_eq!(out.matches[0].path, "note.md");
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
            },
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("search content budget exceeded"));
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

    #[test]
    fn search_matcher_avoids_per_line_lowercase_allocation() {
        assert!(contains_ascii_case_insensitive(
            "Needle in mixed case",
            "needle"
        ));
        assert!(!contains_ascii_case_insensitive("unrelated line", "needle"));

        let source = include_str!("tools.rs");
        let fn_start = source.find("pub async fn search").expect("search exists");
        let fn_end = source[fn_start..]
            .find("\npub async fn write_file")
            .map(|idx| fn_start + idx)
            .expect("write_file follows search");
        let search_source = &source[fn_start..fn_end];

        assert!(
            !search_source.contains("line.to_ascii_lowercase()"),
            "search should not allocate a lowercased String for every line"
        );
    }

    #[test]
    fn mcp_write_activity_serializes_details_without_value_macro() {
        let source = include_str!("tools.rs");
        let fn_start = source
            .find("async fn record_mcp_write_activity")
            .expect("activity helper exists");
        let fn_end = source[fn_start..]
            .find("\nasync fn read_file_inner")
            .map(|idx| fn_start + idx)
            .expect("read_file_inner follows helper");
        let implementation = &source[fn_start..fn_end];

        assert!(
            !implementation.contains("serde_json::json!"),
            "activity details should avoid building an intermediate Value"
        );
    }

    #[test]
    fn tool_definitions_are_cached_in_static_lazy_lock() {
        let source = include_str!("tools.rs");
        let fn_start = source
            .find("pub fn tool_definitions")
            .expect("tool_definitions exists");
        let fn_end = source[fn_start..]
            .find("async fn apply_write_tool")
            .map(|idx| fn_start + idx)
            .expect("tool_definitions should end before apply_write_tool");
        let before_function_end = &source[..fn_end];
        let function = &source[fn_start..fn_end];

        assert!(
            before_function_end.contains("LazyLock<Vec<Tool>>"),
            "tool definitions should be stored in a static LazyLock"
        );
        assert!(
            function.contains("TOOL_DEFINITIONS.clone()"),
            "tool_definitions should clone cached tool definitions instead of rebuilding schemas"
        );
        assert!(
            !function.contains("vec!["),
            "tool_definitions should not allocate and rebuild schemas on each call"
        );
    }

    #[test]
    fn search_and_link_graph_filter_tree_entries_without_extra_vec_collect() {
        let source = include_str!("tools.rs");
        let search_start = source.find("pub async fn search").expect("search exists");
        let link_graph_start = source[search_start..]
            .find("pub async fn link_graph")
            .map(|idx| search_start + idx)
            .expect("link_graph exists");
        let changes_since_start = source[link_graph_start..]
            .find("pub async fn changes_since")
            .map(|idx| link_graph_start + idx)
            .expect("changes_since exists");
        let search_fn = &source[search_start..link_graph_start];
        let link_graph_fn = &source[link_graph_start..changes_since_start];

        assert!(
            !search_fn.contains("let entries = git"),
            "search should iterate filtered tree entries without collecting an extra Vec"
        );
        assert!(
            !link_graph_fn.contains("let mut visible = git"),
            "link_graph should iterate filtered tree entries without collecting an extra Vec"
        );
        assert!(
            !link_graph_fn.contains("visible.sort_by"),
            "link_graph should sort the existing tree entries instead of a filtered copy"
        );
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
