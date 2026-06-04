use crate::api::error::ApiError;
use crate::auth::AuthenticatedUser;
use crate::db::repos::{BlobRefRepo, NewActivity, SyncActivityRepo, Vault, VaultRepo};
use crate::service::sync::{self, PushChange, PushReq, RequestMetadata};
use crate::service::vault::ensure_user_vault;
use crate::service::AppState;
use crate::storage::blob::{BlobStore, LocalFsBlobStore};
use crate::storage::git::{Git2VaultStore, GitVaultStore, StoredFile};
use crate::storage::path;
use crate::storage::text_kind::TextClassifier;
use anyhow::{anyhow, bail, Result};
use base64::Engine;
use rmcp::model::{object, Tool, ToolAnnotations};
use serde::{Deserialize, Serialize};

const SEARCH_MAX_TREE_FILES: usize = 5000;
const DEFAULT_SEARCH_LIMIT: usize = 100;
const SEARCH_MAX_LIMIT: usize = 500;
const LIST_VAULTS_HEAD_CONCURRENCY: usize = 16;
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
    pub snippet: String,
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

pub async fn list_vaults(state: &AppState, user_id: &str) -> Result<ListVaultsOutput> {
    let vault_root = state.default_vault_root();
    let vaults = state.vaults.list_for_user(user_id).await?;
    let mut iter = vaults.into_iter().enumerate();
    let mut tasks = tokio::task::JoinSet::new();
    let mut summaries = Vec::new();

    for _ in 0..LIST_VAULTS_HEAD_CONCURRENCY {
        spawn_next_vault_head_task(&mut iter, &mut tasks, &vault_root);
    }

    while let Some(result) = tasks.join_next().await {
        summaries.push(result??);
        spawn_next_vault_head_task(&mut iter, &mut tasks, &vault_root);
    }

    summaries.sort_by_key(|(index, _)| *index);
    Ok(ListVaultsOutput {
        vaults: summaries.into_iter().map(|(_, summary)| summary).collect(),
    })
}

fn spawn_next_vault_head_task<I>(
    iter: &mut I,
    tasks: &mut tokio::task::JoinSet<Result<(usize, VaultSummary)>>,
    vault_root: &std::path::Path,
) where
    I: Iterator<Item = (usize, Vault)>,
{
    if let Some((index, vault)) = iter.next() {
        let vault_root = vault_root.to_path_buf();
        tasks.spawn(async move {
            let head_commit = Git2VaultStore::new(vault_root).head(&vault.id).await?;
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
    let git = Git2VaultStore::new(state.default_vault_root());
    let mut paths = git
        .list_tree(&input.vault_id, input.at.as_deref())
        .await?
        .into_iter()
        .map(|entry| entry.path)
        .collect::<Vec<_>>();
    paths.sort();
    Ok(ListFilesOutput { paths })
}

pub async fn read_file(
    state: &AppState,
    user_id: &str,
    input: ReadFileInput,
) -> Result<ReadFileOutput> {
    let normalized_path = normalize_mcp_path(input.path)?;
    read_file_inner(state, user_id, input.vault_id, normalized_path, None).await
}

pub async fn read_file_at_commit(
    state: &AppState,
    user_id: &str,
    input: ReadFileAtCommitInput,
) -> Result<ReadFileOutput> {
    let normalized_path = normalize_mcp_path(input.path)?;
    read_file_inner(
        state,
        user_id,
        input.vault_id,
        normalized_path,
        Some(input.commit),
    )
    .await
}

pub async fn search(state: &AppState, user_id: &str, input: SearchInput) -> Result<SearchOutput> {
    ensure_owned_vault(state, user_id, &input.vault_id).await?;
    if input.query.is_empty() {
        return Ok(SearchOutput {
            matches: Vec::new(),
        });
    }

    let git = Git2VaultStore::new(state.default_vault_root());
    let entries = git.list_tree(&input.vault_id, input.at.as_deref()).await?;
    if entries.len() > SEARCH_MAX_TREE_FILES {
        bail!("too many files to search: {}", entries.len());
    }

    let classifier = TextClassifier::default();
    let needle = input.query.to_ascii_lowercase();
    let limit = input
        .limit
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .min(SEARCH_MAX_LIMIT);
    let mut matches = Vec::new();

    for entry in entries {
        if matches.len() >= limit {
            break;
        }
        if entry.is_blob_pointer || !classifier.is_text_path(&entry.path) {
            continue;
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
            if line.to_ascii_lowercase().contains(&needle) {
                matches.push(SearchMatch {
                    path: entry.path.clone(),
                    line: line.to_string(),
                    line_number: idx + 1,
                    snippet: line.to_string(),
                });
                if matches.len() >= limit {
                    break;
                }
            }
        }
    }

    Ok(SearchOutput { matches })
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

fn normalize_mcp_path(raw_path: String) -> Result<String> {
    path::normalize(&raw_path).map_err(|err| anyhow!("invalid_path: {err}"))
}

pub fn tool_definitions() -> Vec<Tool> {
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
    ]
}

async fn apply_write_tool(
    state: &AppState,
    user: &AuthenticatedUser,
    input: WriteToolRequest,
) -> Result<WriteToolOutput> {
    state
        .mcp_write_limiter
        .try_record(&user.token_id, &input.vault_id)
        .map_err(|retry_after| {
            anyhow!(
                "rate_limited: mcp write rate limit (60/min) exceeded for this token+vault; retry in {}s",
                retry_after.as_secs().max(1)
            )
        })?;
    let parent = (!input.parent_commit.is_empty()).then_some(input.parent_commit.as_str());
    match sync::push_with_cas(
        state,
        user,
        &input.vault_id,
        parent,
        RequestMetadata::default(),
        PushReq {
            changes: vec![input.change],
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

async fn record_mcp_write_activity(
    state: &AppState,
    user: &AuthenticatedUser,
    vault_id: &str,
    action: &'static str,
    path: &str,
    size_bytes: usize,
    commit: &str,
) -> Result<()> {
    let details = serde_json::json!({
        "path": path,
        "commit": commit,
        "size_bytes": size_bytes,
    })
    .to_string();
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
    let git = Git2VaultStore::new(state.default_vault_root());
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
            let blob = LocalFsBlobStore::new(state.default_blob_root());
            let bytes = blob
                .get(&hash)
                .await?
                .ok_or_else(|| anyhow!("blob not found: {hash}"))?;
            render_bytes(path, bytes.to_vec(), mime)
        }
    }
}

fn render_bytes(path: String, bytes: Vec<u8>, mime: Option<String>) -> Result<ReadFileOutput> {
    let is_text = TextClassifier::default().is_text_path(&path) || mime_is_text(mime.as_deref());
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
    use crate::db::repos::{BlobRefRepo, NewUser, UserRepo};
    use crate::service::vault;
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
