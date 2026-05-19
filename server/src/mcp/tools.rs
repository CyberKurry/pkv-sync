use crate::db::repos::VaultRepo;
use crate::service::vault::ensure_user_vault;
use crate::service::AppState;
use crate::storage::blob::{BlobStore, LocalFsBlobStore};
use crate::storage::git::{Git2VaultStore, GitVaultStore, StoredFile};
use crate::storage::text_kind::TextClassifier;
use anyhow::{anyhow, bail, Result};
use base64::Engine;
use rmcp::model::{object, Tool, ToolAnnotations};
use serde::{Deserialize, Serialize};

const SEARCH_MAX_TREE_FILES: usize = 5000;
const DEFAULT_SEARCH_LIMIT: usize = 100;

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

pub async fn list_vaults(state: &AppState, user_id: &str) -> Result<ListVaultsOutput> {
    let git = Git2VaultStore::new(state.default_vault_root());
    let mut vaults = Vec::new();
    for vault in state.vaults.list_for_user(user_id).await? {
        let head_commit = git.head(&vault.id).await?;
        vaults.push(VaultSummary {
            id: vault.id,
            name: vault.name,
            head_commit,
            file_count: vault.file_count,
            size_bytes: vault.size_bytes,
        });
    }
    Ok(ListVaultsOutput { vaults })
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
    read_file_inner(state, user_id, input.vault_id, input.path, None).await
}

pub async fn read_file_at_commit(
    state: &AppState,
    user_id: &str,
    input: ReadFileAtCommitInput,
) -> Result<ReadFileOutput> {
    read_file_inner(
        state,
        user_id,
        input.vault_id,
        input.path,
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
    let limit = input.limit.unwrap_or(DEFAULT_SEARCH_LIMIT);
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
                    "limit": { "type": ["integer", "null"], "minimum": 1 }
                },
                "additionalProperties": false
            })),
        ),
    ]
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
    render_file(state, path, file).await
}

async fn render_file(state: &AppState, path: String, file: StoredFile) -> Result<ReadFileOutput> {
    match file {
        StoredFile::Text { bytes } => render_bytes(path, bytes, None),
        StoredFile::BlobPointer { hash, mime, .. } => {
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

async fn ensure_owned_vault(state: &AppState, user_id: &str, vault_id: &str) -> Result<()> {
    ensure_user_vault(state, user_id, vault_id)
        .await
        .map(|_| ())
        .map_err(|e| anyhow!(e.message))
}
