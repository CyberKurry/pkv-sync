use crate::storage::blob::is_sha256_hex;
use crate::storage::text_kind::TextClassifier;
use async_trait::async_trait;
use git2::{Delta, DiffFindOptions, ObjectType, Oid, Repository, Signature, Tree};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const MAIN_REF: &str = "refs/heads/main";
#[cfg(test)]
const MAX_REACHABLE_WALK: usize = 3;
#[cfg(not(test))]
const MAX_REACHABLE_WALK: usize = 10_000;
#[cfg(test)]
const LIST_CHANGES_MAX: usize = 8;
#[cfg(not(test))]
const LIST_CHANGES_MAX: usize = 10_000;
#[cfg(test)]
const MAX_TREE_DEPTH: usize = 4;
#[cfg(not(test))]
const MAX_TREE_DEPTH: usize = 256;
pub const POINTER_MAGIC_KEY: &str = "pkvsync_pointer";
pub const POINTER_VERSION: u64 = 1;
const POINTER_BLOB_MAX_BYTES: usize = 512;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StoredFile {
    Text {
        bytes: Vec<u8>,
    },
    BlobPointer {
        hash: String,
        size: u64,
        mime: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FileChange {
    Upsert { path: String, file: StoredFile },
    Delete { path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreeEntry {
    pub path: String,
    pub git_oid: String,
    pub size: u64,
    pub is_blob_pointer: bool,
    #[serde(skip)]
    pub blob_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangedEntry {
    pub path: String,
    pub status: ChangeStatus,
    pub old_path: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum GitStoreError {
    #[error("git: {0}")]
    Git(#[from] git2::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("path not found")]
    NotFound,
    #[error("path conflict between file and directory: {0}")]
    PathConflict(String),
    #[error("invalid vault id")]
    InvalidVaultId,
    #[error("blocking task panicked")]
    Panic,
}

#[async_trait]
pub trait GitVaultStore: Send + Sync {
    async fn ensure_repo(&self, vault_id: &str) -> Result<(), GitStoreError>;
    async fn head(&self, vault_id: &str) -> Result<Option<String>, GitStoreError>;
    async fn commit_changes(
        &self,
        vault_id: &str,
        parent: Option<&str>,
        changes: &[FileChange],
        message: &str,
    ) -> Result<String, GitStoreError>;
    async fn list_tree(
        &self,
        vault_id: &str,
        at: Option<&str>,
    ) -> Result<Vec<TreeEntry>, GitStoreError>;
    async fn read_file(
        &self,
        vault_id: &str,
        path: &str,
        at: Option<&str>,
    ) -> Result<Option<StoredFile>, GitStoreError>;
    async fn list_changes_between(
        &self,
        vault_id: &str,
        from: &str,
        to: &str,
    ) -> Result<Vec<ChangedEntry>, GitStoreError>;
    async fn is_ancestor(
        &self,
        vault_id: &str,
        ancestor: &str,
        descendant: &str,
    ) -> Result<bool, GitStoreError>;
}

#[derive(Clone)]
pub struct Git2VaultStore {
    root: Arc<PathBuf>,
}

impl Git2VaultStore {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root: Arc::new(root),
        }
    }

    pub fn from_shared_root(root: Arc<PathBuf>) -> Self {
        Self { root }
    }

    fn repo_path(&self, vault_id: &str) -> Result<PathBuf, GitStoreError> {
        if !is_valid_storage_vault_id(vault_id) {
            return Err(GitStoreError::InvalidVaultId);
        }
        Ok(self.root.join(vault_id))
    }

    pub async fn list_tree_map(
        &self,
        vault_id: &str,
        at: Option<&str>,
    ) -> Result<std::collections::BTreeMap<String, TreeEntry>, GitStoreError> {
        let entries = self.list_tree(vault_id, at).await?;
        Ok(entries.into_iter().map(|e| (e.path.clone(), e)).collect())
    }

    pub async fn file_size_at(
        &self,
        vault_id: &str,
        path: &str,
        at: Option<&str>,
    ) -> Result<Option<u64>, GitStoreError> {
        let p = self.repo_path(vault_id)?;
        let path = path.to_string();
        let at = at.map(str::to_string);
        tokio::task::spawn_blocking(move || -> Result<Option<u64>, GitStoreError> {
            let repo = Repository::open_bare(&p)?;
            let oid = match at {
                Some(h) => Oid::from_str(&h)?,
                None => main_ref_target(&repo)?.ok_or(GitStoreError::NotFound)?,
            };
            let commit = repo.find_commit(oid)?;
            let tree = commit.tree()?;
            let Ok(entry) = tree.get_path(Path::new(&path)) else {
                return Ok(None);
            };
            if entry.kind() != Some(ObjectType::Blob) {
                return Ok(None);
            }
            let blob = repo.find_blob(entry.id())?;
            let pointer = parse_blob_pointer_if_candidate(&blob)
                .and_then(|pointer| pointer.into_file_for_path(&path));
            let size = match pointer {
                Some(StoredFile::BlobPointer { size, .. }) => size,
                _ => blob.size() as u64,
            };
            Ok(Some(size))
        })
        .await
        .map_err(|_| GitStoreError::Panic)?
    }

    pub async fn commit_parent(
        &self,
        vault_id: &str,
        commit: &str,
    ) -> Result<Option<String>, GitStoreError> {
        let p = self.repo_path(vault_id)?;
        let commit = commit.to_string();
        tokio::task::spawn_blocking(move || -> Result<Option<String>, GitStoreError> {
            let repo = Repository::open_bare(&p)?;
            let oid = Oid::from_str(&commit)?;
            let commit = repo.find_commit(oid)?;
            if commit.parent_count() == 0 {
                return Ok(None);
            }
            Ok(Some(commit.parent_id(0)?.to_string()))
        })
        .await
        .map_err(|_| GitStoreError::Panic)?
    }

    pub async fn commit_reachable_from_head(
        &self,
        vault_id: &str,
        commit: &str,
    ) -> Result<bool, GitStoreError> {
        let p = self.repo_path(vault_id)?;
        let commit = commit.to_string();
        tokio::task::spawn_blocking(move || -> Result<bool, GitStoreError> {
            let repo = Repository::open_bare(&p)?;
            let Ok(target) = Oid::from_str(&commit) else {
                return Ok(false);
            };
            if repo.find_commit(target).is_err() {
                return Ok(false);
            }
            let Some(head) = main_ref_target(&repo)? else {
                return Ok(false);
            };
            let mut walk = repo.revwalk()?;
            walk.push(head)?;
            for (idx, oid) in walk.enumerate() {
                if idx >= MAX_REACHABLE_WALK {
                    return Ok(false);
                }
                if oid? == target {
                    return Ok(true);
                }
            }
            Ok(false)
        })
        .await
        .map_err(|_| GitStoreError::Panic)?
    }

    pub async fn set_main_ref(
        &self,
        vault_id: &str,
        commit: &str,
        message: &str,
    ) -> Result<(), GitStoreError> {
        let p = self.repo_path(vault_id)?;
        let commit = commit.to_string();
        let message = message.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), GitStoreError> {
            let repo = Repository::open_bare(&p)?;
            let target = Oid::from_str(&commit)?;
            repo.find_commit(target)?;
            repo.reference(MAIN_REF, target, true, &message)?;
            repo.set_head(MAIN_REF)?;
            Ok(())
        })
        .await
        .map_err(|_| GitStoreError::Panic)?
    }

    pub async fn tree_diff(
        &self,
        vault_id: &str,
        parent: Option<&str>,
        commit: &str,
    ) -> Result<Vec<crate::service::diff::CommitChange>, GitStoreError> {
        let p = self.repo_path(vault_id)?;
        let parent = parent.map(str::to_string);
        let commit = commit.to_string();
        tokio::task::spawn_blocking(
            move || -> Result<Vec<crate::service::diff::CommitChange>, GitStoreError> {
                let repo = Repository::open_bare(&p)?;
                let commit = repo.find_commit(Oid::from_str(&commit)?)?;
                let new_tree = commit.tree()?;
                let old_commit = match parent {
                    Some(parent) => Some(repo.find_commit(Oid::from_str(&parent)?)?),
                    None => None,
                };
                let old_tree = old_commit
                    .as_ref()
                    .map(|commit| commit.tree())
                    .transpose()?;
                let mut diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), None)?;
                let mut find = DiffFindOptions::new();
                find.renames(true);
                let _ = diff.find_similar(Some(&mut find));
                let mut changes = Vec::new();
                let mut pointer_cache = HashMap::new();
                for delta in diff.deltas() {
                    let status = delta.status();
                    let (path, old_path, change_type) = match status {
                        Delta::Added => (
                            delta_path(delta.new_file().path())?,
                            None,
                            crate::service::diff::ChangeType::Added,
                        ),
                        Delta::Deleted => (
                            delta_path(delta.old_file().path())?,
                            None,
                            crate::service::diff::ChangeType::Deleted,
                        ),
                        Delta::Modified | Delta::Typechange => (
                            delta_path(delta.new_file().path())?,
                            None,
                            crate::service::diff::ChangeType::Modified,
                        ),
                        Delta::Renamed => (
                            delta_path(delta.new_file().path())?,
                            Some(delta_path(delta.old_file().path())?),
                            crate::service::diff::ChangeType::Modified,
                        ),
                        Delta::Copied => (
                            delta_path(delta.new_file().path())?,
                            Some(delta_path(delta.old_file().path())?),
                            crate::service::diff::ChangeType::Added,
                        ),
                        _ => continue,
                    };
                    let binary = is_binary_delta(
                        &repo,
                        old_tree.as_ref(),
                        &new_tree,
                        &path,
                        old_path.as_deref(),
                        &mut pointer_cache,
                    )?;
                    changes.push(crate::service::diff::CommitChange {
                        path,
                        change_type,
                        old_path,
                        binary,
                    });
                }
                changes.sort_by(|a, b| a.path.cmp(&b.path));
                Ok(changes)
            },
        )
        .await
        .map_err(|_| GitStoreError::Panic)?
    }
}

fn delta_path(path: Option<&Path>) -> Result<String, GitStoreError> {
    Ok(path
        .ok_or(GitStoreError::NotFound)?
        .to_string_lossy()
        .replace('\\', "/"))
}

fn parse_full_oid(value: &str) -> Result<Oid, GitStoreError> {
    if value.len() != 40 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(git2::Error::from_str("invalid object id").into());
    }
    Ok(Oid::from_str(value)?)
}

fn is_binary_delta(
    repo: &Repository,
    old_tree: Option<&Tree<'_>>,
    new_tree: &Tree<'_>,
    path: &str,
    old_path: Option<&str>,
    pointer_cache: &mut HashMap<Oid, PointerProbe>,
) -> Result<bool, GitStoreError> {
    let classifier = TextClassifier::default_ref();
    let text_path = classifier.is_text_path(path)
        || old_path
            .map(|old_path| classifier.is_text_path(old_path))
            .unwrap_or(false);
    if !text_path {
        return Ok(true);
    }
    if tree_path_is_pointer(repo, Some(new_tree), path, pointer_cache)? {
        return Ok(true);
    }
    if let Some(old_path) = old_path.or(Some(path)) {
        if tree_path_is_pointer(repo, old_tree, old_path, pointer_cache)? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[derive(Clone, Copy)]
struct PointerProbe {
    modern: bool,
    legacy: bool,
}

fn tree_path_is_pointer(
    repo: &Repository,
    tree: Option<&Tree<'_>>,
    path: &str,
    pointer_cache: &mut HashMap<Oid, PointerProbe>,
) -> Result<bool, GitStoreError> {
    let Some(tree) = tree else {
        return Ok(false);
    };
    let Ok(entry) = tree.get_path(Path::new(path)) else {
        return Ok(false);
    };
    let probe = match pointer_cache.get(&entry.id()).copied() {
        Some(probe) => probe,
        None => {
            let blob = repo.find_blob(entry.id())?;
            let candidate = parse_blob_pointer_if_candidate(&blob);
            let probe = PointerProbe {
                modern: candidate.as_ref().is_some_and(|pointer| pointer.has_magic),
                legacy: candidate.is_some(),
            };
            pointer_cache.insert(entry.id(), probe);
            probe
        }
    };
    Ok(probe.modern || (!TextClassifier::default_ref().is_text_path(path) && probe.legacy))
}

#[derive(Clone)]
struct BlobPointerCandidate {
    has_magic: bool,
    file: StoredFile,
}

impl BlobPointerCandidate {
    fn into_file_for_path(self, path: &str) -> Option<StoredFile> {
        if self.has_magic || !TextClassifier::default_ref().is_text_path(path) {
            Some(self.file)
        } else {
            None
        }
    }
}

fn parse_blob_pointer_if_candidate(blob: &git2::Blob<'_>) -> Option<BlobPointerCandidate> {
    if blob.size() > POINTER_BLOB_MAX_BYTES {
        return None;
    }
    let bytes = blob.content();
    if bytes.first() != Some(&b'{') {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let has_magic = value.get(POINTER_MAGIC_KEY).and_then(|v| v.as_u64()) == Some(POINTER_VERSION);
    let hash = value.get("blob")?.as_str()?.to_string();
    if !is_sha256_hex(&hash) {
        return None;
    }
    let size = value.get("size")?.as_u64()?;
    let mime = value
        .get("mime")
        .and_then(|m| m.as_str())
        .map(str::to_string);
    Some(BlobPointerCandidate {
        has_magic,
        file: StoredFile::BlobPointer { hash, size, mime },
    })
}

fn is_valid_storage_vault_id(vault_id: &str) -> bool {
    !vault_id.is_empty()
        && vault_id.len() <= 128
        && vault_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn sig() -> Result<Signature<'static>, git2::Error> {
    Signature::now("PKV Sync", "pkv-sync@example.invalid")
}

fn init_bare_main(path: &Path) -> Result<Repository, GitStoreError> {
    let repo = Repository::init_bare(path)?;
    repo.set_head(MAIN_REF)?;
    Ok(repo)
}

fn open_or_init_bare_main(path: &Path) -> Result<Repository, GitStoreError> {
    std::fs::create_dir_all(path)?;
    match Repository::open_bare(path) {
        Ok(repo) => Ok(repo),
        Err(_) => match init_bare_main(path) {
            Ok(repo) => Ok(repo),
            // If another thread/process initialized the repo after our failed
            // open but before init_bare_main, accept that completed repo.
            Err(init_err) => match Repository::open_bare(path) {
                Ok(repo) => Ok(repo),
                Err(_) => Err(init_err),
            },
        },
    }
}

fn main_ref_target(repo: &Repository) -> Result<Option<Oid>, GitStoreError> {
    match repo.find_reference(MAIN_REF) {
        Ok(r) => Ok(r.target()),
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn encode_file(f: StoredFile) -> Result<Vec<u8>, serde_json::Error> {
    match f {
        StoredFile::Text { bytes } => Ok(bytes),
        StoredFile::BlobPointer { hash, size, mime } => serde_json::to_vec(&serde_json::json!({
            POINTER_MAGIC_KEY: POINTER_VERSION,
            "blob": hash,
            "size": size,
            "mime": mime,
        })),
    }
}

fn is_pointer_bytes(bytes: &[u8]) -> Option<StoredFile> {
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if v.get(POINTER_MAGIC_KEY)?.as_u64()? != POINTER_VERSION {
        return None;
    }
    pointer_from_value(&v)
}

fn is_legacy_pointer_bytes(bytes: &[u8]) -> Option<StoredFile> {
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    pointer_from_value(&v)
}

fn pointer_from_value(v: &serde_json::Value) -> Option<StoredFile> {
    let hash = v.get("blob")?.as_str()?.to_string();
    if !is_sha256_hex(&hash) {
        return None;
    }
    let size = v.get("size")?.as_u64()?;
    let mime = v
        .get("mime")
        .and_then(|m| m.as_str())
        .map(|s| s.to_string());
    Some(StoredFile::BlobPointer { hash, size, mime })
}

fn decode_file(path: &str, bytes: Vec<u8>) -> StoredFile {
    if let Some(pointer) = is_pointer_bytes(&bytes) {
        return pointer;
    }
    if !TextClassifier::default_ref().is_text_path(path) {
        if let Some(pointer) = is_legacy_pointer_bytes(&bytes) {
            return pointer;
        }
    }
    StoredFile::Text { bytes }
}

fn read_tree_recursive(
    repo: &Repository,
    tree: &Tree<'_>,
    prefix: &str,
    depth: usize,
    out: &mut BTreeMap<String, StoredFile>,
) -> Result<(), GitStoreError> {
    if depth > MAX_TREE_DEPTH {
        return Err(git2::Error::from_str("tree depth exceeds maximum").into());
    }
    for entry in tree.iter() {
        let Some(name) = entry.name() else {
            continue;
        };
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        match entry.kind() {
            Some(ObjectType::Blob) => {
                let blob = repo.find_blob(entry.id())?;
                let bytes = blob.content().to_vec();
                let file = decode_file(&path, bytes);
                out.insert(path, file);
            }
            Some(ObjectType::Tree) => {
                let subtree = repo.find_tree(entry.id())?;
                read_tree_recursive(repo, &subtree, &path, depth + 1, out)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn build_tree_recursive(
    repo: &Repository,
    files: BTreeMap<String, StoredFile>,
) -> Result<Oid, GitStoreError> {
    enum TreeNode {
        File(StoredFile),
        Dir(BTreeMap<String, TreeNode>),
    }

    fn insert(
        full_path: &str,
        parts: &[&str],
        file: StoredFile,
        node: &mut BTreeMap<String, TreeNode>,
    ) -> Result<(), GitStoreError> {
        if parts.len() == 1 {
            if matches!(node.get(parts[0]), Some(TreeNode::Dir(_))) {
                return Err(GitStoreError::PathConflict(full_path.to_string()));
            }
            node.insert(parts[0].to_string(), TreeNode::File(file));
        } else {
            let child = node
                .entry(parts[0].to_string())
                .or_insert_with(|| TreeNode::Dir(BTreeMap::new()));
            match child {
                TreeNode::Dir(map) => insert(full_path, &parts[1..], file, map)?,
                TreeNode::File(_) => {
                    return Err(GitStoreError::PathConflict(full_path.to_string()))
                }
            }
        }
        Ok(())
    }

    fn write_node(
        repo: &Repository,
        node: BTreeMap<String, TreeNode>,
    ) -> Result<Oid, GitStoreError> {
        let mut builder = repo.treebuilder(None)?;
        for (name, value) in node {
            match value {
                TreeNode::File(file) => {
                    let bytes = encode_file(file)?;
                    let oid = repo.blob(&bytes)?;
                    builder.insert(name, oid, 0o100644)?;
                }
                TreeNode::Dir(children) => {
                    let oid = write_node(repo, children)?;
                    builder.insert(name, oid, 0o040000)?;
                }
            }
        }
        Ok(builder.write()?)
    }

    let mut root = BTreeMap::new();
    for (path, file) in files {
        let parts: Vec<&str> = path.split('/').collect();
        insert(&path, &parts, file, &mut root)?;
    }
    write_node(repo, root)
}

fn tree_entries_recursive(
    repo: &Repository,
    tree: &Tree<'_>,
    prefix: &str,
    depth: usize,
    out: &mut Vec<TreeEntry>,
) -> Result<(), GitStoreError> {
    if depth > MAX_TREE_DEPTH {
        return Err(git2::Error::from_str("tree depth exceeds maximum").into());
    }
    for entry in tree.iter() {
        let Some(name) = entry.name() else {
            continue;
        };
        let path = if prefix.is_empty() {
            name.to_string()
        } else {
            format!("{prefix}/{name}")
        };
        match entry.kind() {
            Some(ObjectType::Blob) => {
                let blob = repo.find_blob(entry.id())?;
                let pointer = parse_blob_pointer_if_candidate(&blob)
                    .and_then(|pointer| pointer.into_file_for_path(&path));
                let size = match &pointer {
                    Some(StoredFile::BlobPointer { size, .. }) => *size,
                    _ => blob.size() as u64,
                };
                let blob_hash = match &pointer {
                    Some(StoredFile::BlobPointer { hash, .. }) => Some(hash.clone()),
                    _ => None,
                };
                out.push(TreeEntry {
                    path,
                    git_oid: entry.id().to_string(),
                    size,
                    is_blob_pointer: pointer.is_some(),
                    blob_hash,
                });
            }
            Some(ObjectType::Tree) => {
                let subtree = repo.find_tree(entry.id())?;
                tree_entries_recursive(repo, &subtree, &path, depth + 1, out)?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[async_trait]
impl GitVaultStore for Git2VaultStore {
    async fn ensure_repo(&self, vault_id: &str) -> Result<(), GitStoreError> {
        let p = self.repo_path(vault_id)?;
        tokio::task::spawn_blocking(move || -> Result<(), GitStoreError> {
            let _repo = open_or_init_bare_main(&p)?;
            Ok(())
        })
        .await
        .map_err(|_| GitStoreError::Panic)?
    }

    async fn head(&self, vault_id: &str) -> Result<Option<String>, GitStoreError> {
        let p = self.repo_path(vault_id)?;
        tokio::task::spawn_blocking(move || -> Result<Option<String>, GitStoreError> {
            if !p.exists() {
                return Ok(None);
            }
            let repo = Repository::open_bare(&p)?;
            Ok(main_ref_target(&repo)?.map(|o| o.to_string()))
        })
        .await
        .map_err(|_| GitStoreError::Panic)?
    }

    async fn commit_changes(
        &self,
        vault_id: &str,
        parent: Option<&str>,
        changes: &[FileChange],
        message: &str,
    ) -> Result<String, GitStoreError> {
        let p = self.repo_path(vault_id)?;
        let changes = changes.to_vec();
        let message = message.to_string();
        let parent = parent.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || -> Result<String, GitStoreError> {
            let repo = open_or_init_bare_main(&p)?;
            let mut current: BTreeMap<String, StoredFile> = BTreeMap::new();
            let parent_commit = match parent {
                Some(ref h) => Some(repo.find_commit(Oid::from_str(h)?)?),
                None => main_ref_target(&repo)?.and_then(|oid| repo.find_commit(oid).ok()),
            };
            if let Some(pc) = &parent_commit {
                let tree = pc.tree()?;
                read_tree_recursive(&repo, &tree, "", 0, &mut current)?;
            }
            for ch in changes {
                match ch {
                    FileChange::Upsert { path, file } => {
                        current.insert(path, file);
                    }
                    FileChange::Delete { path } => {
                        current.remove(&path);
                    }
                }
            }
            let tree_oid = build_tree_recursive(&repo, current)?;
            let tree = repo.find_tree(tree_oid)?;
            let sig = sig()?;
            let oid = match parent_commit {
                Some(ref pc) => repo.commit(Some(MAIN_REF), &sig, &sig, &message, &tree, &[pc])?,
                None => repo.commit(Some(MAIN_REF), &sig, &sig, &message, &tree, &[])?,
            };
            Ok(oid.to_string())
        })
        .await
        .map_err(|_| GitStoreError::Panic)?
    }

    async fn list_tree(
        &self,
        vault_id: &str,
        at: Option<&str>,
    ) -> Result<Vec<TreeEntry>, GitStoreError> {
        let p = self.repo_path(vault_id)?;
        let at = at.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || -> Result<Vec<TreeEntry>, GitStoreError> {
            let repo = Repository::open_bare(&p)?;
            let oid = match at {
                Some(h) => Oid::from_str(&h)?,
                None => main_ref_target(&repo)?.ok_or(GitStoreError::NotFound)?,
            };
            let commit = repo.find_commit(oid)?;
            let tree = commit.tree()?;
            let mut out = Vec::new();
            tree_entries_recursive(&repo, &tree, "", 0, &mut out)?;
            Ok(out)
        })
        .await
        .map_err(|_| GitStoreError::Panic)?
    }

    async fn read_file(
        &self,
        vault_id: &str,
        path: &str,
        at: Option<&str>,
    ) -> Result<Option<StoredFile>, GitStoreError> {
        let p = self.repo_path(vault_id)?;
        let path = path.to_string();
        let at = at.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || -> Result<Option<StoredFile>, GitStoreError> {
            let repo = Repository::open_bare(&p)?;
            let oid = match at {
                Some(h) => Oid::from_str(&h)?,
                None => main_ref_target(&repo)?.ok_or(GitStoreError::NotFound)?,
            };
            let commit = repo.find_commit(oid)?;
            let tree = commit.tree()?;
            let Ok(entry) = tree.get_path(Path::new(&path)) else {
                return Ok(None);
            };
            let blob = repo.find_blob(entry.id())?;
            let bytes = blob.content().to_vec();
            Ok(Some(decode_file(&path, bytes)))
        })
        .await
        .map_err(|_| GitStoreError::Panic)?
    }

    async fn list_changes_between(
        &self,
        vault_id: &str,
        from: &str,
        to: &str,
    ) -> Result<Vec<ChangedEntry>, GitStoreError> {
        let p = self.repo_path(vault_id)?;
        let from = from.to_string();
        let to = to.to_string();
        tokio::task::spawn_blocking(move || -> Result<Vec<ChangedEntry>, GitStoreError> {
            let repo = Repository::open_bare(&p)?;
            let from_commit = repo.find_commit(parse_full_oid(&from)?)?;
            let to_commit = repo.find_commit(parse_full_oid(&to)?)?;
            let from_tree = from_commit.tree()?;
            let to_tree = to_commit.tree()?;
            let mut diff = repo.diff_tree_to_tree(Some(&from_tree), Some(&to_tree), None)?;
            let mut find = DiffFindOptions::new();
            find.renames(true);
            diff.find_similar(Some(&mut find))?;

            let mut out = Vec::new();
            for delta in diff.deltas() {
                if out.len() >= LIST_CHANGES_MAX {
                    break;
                }
                let status = delta.status();
                let entry = match status {
                    Delta::Added => ChangedEntry {
                        path: delta_path(delta.new_file().path())?,
                        status: ChangeStatus::Added,
                        old_path: None,
                    },
                    Delta::Deleted => ChangedEntry {
                        path: delta_path(delta.old_file().path())?,
                        status: ChangeStatus::Deleted,
                        old_path: None,
                    },
                    Delta::Renamed => ChangedEntry {
                        path: delta_path(delta.new_file().path())?,
                        status: ChangeStatus::Renamed,
                        old_path: Some(delta_path(delta.old_file().path())?),
                    },
                    Delta::Modified | Delta::Typechange => ChangedEntry {
                        path: delta_path(delta.new_file().path())?,
                        status: ChangeStatus::Modified,
                        old_path: None,
                    },
                    Delta::Copied => ChangedEntry {
                        path: delta_path(delta.new_file().path())?,
                        status: ChangeStatus::Added,
                        old_path: None,
                    },
                    _ => continue,
                };
                out.push(entry);
            }
            out.sort_by(|left, right| {
                left.path
                    .cmp(&right.path)
                    .then_with(|| left.old_path.cmp(&right.old_path))
            });
            Ok(out)
        })
        .await
        .map_err(|_| GitStoreError::Panic)?
    }

    async fn is_ancestor(
        &self,
        vault_id: &str,
        ancestor: &str,
        descendant: &str,
    ) -> Result<bool, GitStoreError> {
        let p = self.repo_path(vault_id)?;
        let ancestor = ancestor.to_string();
        let descendant = descendant.to_string();
        tokio::task::spawn_blocking(move || -> Result<bool, GitStoreError> {
            let repo = Repository::open_bare(&p)?;
            let ancestor = Oid::from_str(&ancestor)?;
            let descendant = Oid::from_str(&descendant)?;
            repo.find_commit(ancestor)?;
            repo.find_commit(descendant)?;
            Ok(repo.graph_descendant_of(descendant, ancestor)?)
        })
        .await
        .map_err(|_| GitStoreError::Panic)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_diff_caches_pointer_checks_per_blob_oid() {
        let source = include_str!("git.rs");
        let tree_diff_start = source.find("pub async fn tree_diff").unwrap();
        let delta_path_start = source[tree_diff_start..]
            .find("fn delta_path")
            .map(|idx| tree_diff_start + idx)
            .unwrap();
        let is_binary_delta_start = source.find("fn is_binary_delta").unwrap();
        let tree_path_is_pointer_start = source.find("fn tree_path_is_pointer").unwrap();
        let tree_diff = &source[tree_diff_start..delta_path_start];
        let is_binary_delta = &source[is_binary_delta_start..tree_path_is_pointer_start];

        assert!(
            tree_diff.contains("pointer_cache"),
            "tree_diff should create a pointer result cache for all deltas"
        );
        assert!(
            is_binary_delta.contains("pointer_cache"),
            "is_binary_delta should reuse the pointer result cache"
        );
        assert!(
            is_binary_delta.matches("tree_path_is_pointer(").count() == 2,
            "is_binary_delta should keep exactly the new/old path pointer checks"
        );
    }

    #[test]
    fn pointer_detection_rejects_large_or_non_json_blobs_before_parsing() {
        let source = include_str!("git.rs");
        let tree_entries_start = source.find("fn tree_entries_recursive").unwrap();
        let list_tree_start = source[tree_entries_start..]
            .find("async fn list_tree")
            .map(|idx| tree_entries_start + idx)
            .unwrap();
        let helper_start = source[..tree_entries_start]
            .find("fn parse_blob_pointer_if_candidate")
            .unwrap_or(tree_entries_start);
        let helper_and_tree_entries = &source[helper_start..list_tree_start];

        assert!(
            helper_and_tree_entries.contains("POINTER_BLOB_MAX_BYTES"),
            "pointer detection should skip blobs larger than the pointer JSON bound"
        );
        assert!(
            helper_and_tree_entries.contains("bytes.first() != Some(&b'{')"),
            "pointer detection should reject non-JSON-looking blobs before serde parsing"
        );
    }

    #[tokio::test]
    async fn commit_and_read_text() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        store.ensure_repo("v1").await.unwrap();
        assert!(store.head("v1").await.unwrap().is_none());
        let c = store
            .commit_changes(
                "v1",
                None,
                &[FileChange::Upsert {
                    path: "note.md".into(),
                    file: StoredFile::Text {
                        bytes: b"hello".to_vec(),
                    },
                }],
                "initial",
            )
            .await
            .unwrap();
        assert_eq!(store.head("v1").await.unwrap().unwrap(), c);
        let got = store
            .read_file("v1", "note.md", None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            got,
            StoredFile::Text {
                bytes: b"hello".to_vec()
            }
        );
    }

    #[tokio::test]
    async fn commit_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let c1 = store
            .commit_changes(
                "v1",
                None,
                &[FileChange::Upsert {
                    path: "a.md".into(),
                    file: StoredFile::Text {
                        bytes: b"a".to_vec(),
                    },
                }],
                "c1",
            )
            .await
            .unwrap();
        let _c2 = store
            .commit_changes(
                "v1",
                Some(&c1),
                &[FileChange::Delete {
                    path: "a.md".into(),
                }],
                "c2",
            )
            .await
            .unwrap();
        assert!(store.read_file("v1", "a.md", None).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn ensure_repo_initializes_existing_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        std::fs::create_dir_all(dir.path().join("v1")).unwrap();

        store.ensure_repo("v1").await.unwrap();

        assert!(store.head("v1").await.unwrap().is_none());
        let commit = store
            .commit_changes(
                "v1",
                None,
                &[FileChange::Upsert {
                    path: "note.md".into(),
                    file: StoredFile::Text {
                        bytes: b"hello".to_vec(),
                    },
                }],
                "seed",
            )
            .await
            .unwrap();
        assert_eq!(
            store.head("v1").await.unwrap().as_deref(),
            Some(commit.as_str())
        );
    }

    #[tokio::test]
    async fn commit_reachable_from_head_stops_after_walk_budget() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let first = store
            .commit_changes(
                "v1",
                None,
                &[FileChange::Upsert {
                    path: "note-0.md".into(),
                    file: StoredFile::Text {
                        bytes: b"0".to_vec(),
                    },
                }],
                "c0",
            )
            .await
            .unwrap();
        let mut parent = first.clone();
        for idx in 1..=4 {
            parent = store
                .commit_changes(
                    "v1",
                    Some(&parent),
                    &[FileChange::Upsert {
                        path: format!("note-{idx}.md"),
                        file: StoredFile::Text {
                            bytes: idx.to_string().into_bytes(),
                        },
                    }],
                    &format!("c{idx}"),
                )
                .await
                .unwrap();
        }

        assert!(store
            .commit_reachable_from_head("v1", &parent)
            .await
            .unwrap());
        assert!(
            !store
                .commit_reachable_from_head("v1", &first)
                .await
                .unwrap(),
            "old commits beyond the test walk budget should not force an unbounded revwalk"
        );
    }

    #[tokio::test]
    async fn list_changes_between_reports_add_modify_delete_and_rename() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let base = store
            .commit_changes(
                "v1",
                None,
                &[
                    FileChange::Upsert {
                        path: "a.md".into(),
                        file: StoredFile::Text {
                            bytes: b"old".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "b.md".into(),
                        file: StoredFile::Text {
                            bytes: b"delete me".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "old-name.md".into(),
                        file: StoredFile::Text {
                            bytes: b"same content".to_vec(),
                        },
                    },
                ],
                "base",
            )
            .await
            .unwrap();
        let head = store
            .commit_changes(
                "v1",
                Some(&base),
                &[
                    FileChange::Upsert {
                        path: "a.md".into(),
                        file: StoredFile::Text {
                            bytes: b"new".to_vec(),
                        },
                    },
                    FileChange::Delete {
                        path: "b.md".into(),
                    },
                    FileChange::Upsert {
                        path: "c.md".into(),
                        file: StoredFile::Text {
                            bytes: b"added".to_vec(),
                        },
                    },
                    FileChange::Delete {
                        path: "old-name.md".into(),
                    },
                    FileChange::Upsert {
                        path: "new-name.md".into(),
                        file: StoredFile::Text {
                            bytes: b"same content".to_vec(),
                        },
                    },
                ],
                "head",
            )
            .await
            .unwrap();

        let changes = store
            .list_changes_between("v1", &base, &head)
            .await
            .unwrap();

        assert!(changes.contains(&ChangedEntry {
            path: "a.md".into(),
            status: ChangeStatus::Modified,
            old_path: None,
        }));
        assert!(changes.contains(&ChangedEntry {
            path: "b.md".into(),
            status: ChangeStatus::Deleted,
            old_path: None,
        }));
        assert!(changes.contains(&ChangedEntry {
            path: "c.md".into(),
            status: ChangeStatus::Added,
            old_path: None,
        }));
        assert!(changes.contains(&ChangedEntry {
            path: "new-name.md".into(),
            status: ChangeStatus::Renamed,
            old_path: Some("old-name.md".into()),
        }));
    }

    #[tokio::test]
    async fn list_changes_between_rejects_refs_and_short_oids() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let base = store
            .commit_changes(
                "v1",
                None,
                &[FileChange::Upsert {
                    path: "a.md".into(),
                    file: StoredFile::Text {
                        bytes: b"a".to_vec(),
                    },
                }],
                "base",
            )
            .await
            .unwrap();
        let head = store
            .commit_changes(
                "v1",
                Some(&base),
                &[FileChange::Upsert {
                    path: "b.md".into(),
                    file: StoredFile::Text {
                        bytes: b"b".to_vec(),
                    },
                }],
                "head",
            )
            .await
            .unwrap();

        assert!(store
            .list_changes_between("v1", "HEAD", &head)
            .await
            .is_err());
        assert!(store
            .list_changes_between("v1", &base, "HEAD")
            .await
            .is_err());
        assert!(store
            .list_changes_between("v1", &base[..7], &head)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn list_changes_between_caps_output() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let base = store
            .commit_changes(
                "v1",
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
        let changes: Vec<_> = (0..9)
            .map(|idx| FileChange::Upsert {
                path: format!("note-{idx}.md"),
                file: StoredFile::Text {
                    bytes: idx.to_string().into_bytes(),
                },
            })
            .collect();
        let head = store
            .commit_changes("v1", Some(&base), &changes, "head")
            .await
            .unwrap();

        let changes = store
            .list_changes_between("v1", &base, &head)
            .await
            .unwrap();

        assert_eq!(changes.len(), 8);
    }

    #[tokio::test]
    async fn is_ancestor_reports_true_for_ancestor_and_false_for_unrelated() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let base = store
            .commit_changes(
                "v1",
                None,
                &[FileChange::Upsert {
                    path: "a.md".into(),
                    file: StoredFile::Text {
                        bytes: b"a".to_vec(),
                    },
                }],
                "base",
            )
            .await
            .unwrap();
        let head = store
            .commit_changes(
                "v1",
                Some(&base),
                &[FileChange::Upsert {
                    path: "b.md".into(),
                    file: StoredFile::Text {
                        bytes: b"b".to_vec(),
                    },
                }],
                "head",
            )
            .await
            .unwrap();
        store
            .set_main_ref("v1", &base, "rewind for sibling")
            .await
            .unwrap();
        let unrelated = store
            .commit_changes(
                "v1",
                Some(&base),
                &[FileChange::Upsert {
                    path: "c.md".into(),
                    file: StoredFile::Text {
                        bytes: b"c".to_vec(),
                    },
                }],
                "unrelated",
            )
            .await
            .unwrap();
        store
            .set_main_ref("v1", &head, "restore head")
            .await
            .unwrap();

        assert!(store.is_ancestor("v1", &base, &head).await.unwrap());
        assert!(!store.is_ancestor("v1", &unrelated, &head).await.unwrap());
    }

    #[tokio::test]
    async fn stores_blob_pointer() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let _c = store
            .commit_changes(
                "v1",
                None,
                &[FileChange::Upsert {
                    path: "img.png".into(),
                    file: StoredFile::BlobPointer {
                        hash: "a".repeat(64),
                        size: 12,
                        mime: Some("image/png".into()),
                    },
                }],
                "c",
            )
            .await
            .unwrap();
        let got = store
            .read_file("v1", "img.png", None)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(got, StoredFile::BlobPointer { .. }));
    }

    #[tokio::test]
    async fn text_json_with_blob_shape_stays_text() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let content = serde_json::json!({
            "blob": "a".repeat(64),
            "size": 123,
            "mime": "text/plain"
        })
        .to_string()
        .into_bytes();
        let _c = store
            .commit_changes(
                "v1",
                None,
                &[FileChange::Upsert {
                    path: "data.json".into(),
                    file: StoredFile::Text {
                        bytes: content.clone(),
                    },
                }],
                "json text",
            )
            .await
            .unwrap();

        let got = store
            .read_file("v1", "data.json", None)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(got, StoredFile::Text { bytes: content });
    }

    #[tokio::test]
    async fn list_tree_reports_blob_pointer_declared_size() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let _c = store
            .commit_changes(
                "v1",
                None,
                &[FileChange::Upsert {
                    path: "img.png".into(),
                    file: StoredFile::BlobPointer {
                        hash: "b".repeat(64),
                        size: 12,
                        mime: Some("image/png".into()),
                    },
                }],
                "blob",
            )
            .await
            .unwrap();

        let entries = store.list_tree("v1", None).await.unwrap();

        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_blob_pointer);
        assert_eq!(entries[0].size, 12);
        assert_eq!(entries[0].blob_hash.as_ref(), Some(&"b".repeat(64)));
    }

    #[tokio::test]
    async fn file_size_at_reports_target_file_size_without_directory_errors() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let commit = store
            .commit_changes(
                "v1",
                None,
                &[
                    FileChange::Upsert {
                        path: "note.md".into(),
                        file: StoredFile::Text {
                            bytes: b"hello".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "img.png".into(),
                        file: StoredFile::BlobPointer {
                            hash: "c".repeat(64),
                            size: 12,
                            mime: Some("image/png".into()),
                        },
                    },
                    FileChange::Upsert {
                        path: "dir/a.md".into(),
                        file: StoredFile::Text {
                            bytes: b"nested".to_vec(),
                        },
                    },
                ],
                "seed",
            )
            .await
            .unwrap();

        assert_eq!(
            store
                .file_size_at("v1", "note.md", Some(&commit))
                .await
                .unwrap(),
            Some(5)
        );
        assert_eq!(
            store
                .file_size_at("v1", "img.png", Some(&commit))
                .await
                .unwrap(),
            Some(12)
        );
        assert_eq!(
            store
                .file_size_at("v1", "dir", Some(&commit))
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .file_size_at("v1", "missing.md", Some(&commit))
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn supports_nested_paths() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let _c = store
            .commit_changes(
                "v1",
                None,
                &[
                    FileChange::Upsert {
                        path: "folder/note.md".into(),
                        file: StoredFile::Text {
                            bytes: b"nested".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "folder/sub/other.md".into(),
                        file: StoredFile::Text {
                            bytes: b"deep".to_vec(),
                        },
                    },
                ],
                "nested",
            )
            .await
            .unwrap();
        let got = store
            .read_file("v1", "folder/note.md", None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            got,
            StoredFile::Text {
                bytes: b"nested".to_vec()
            }
        );
        let listed: Vec<String> = store
            .list_tree("v1", None)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.path)
            .collect();
        assert!(listed.contains(&"folder/note.md".to_string()));
        assert!(listed.contains(&"folder/sub/other.md".to_string()));
    }

    #[tokio::test]
    async fn list_tree_rejects_trees_deeper_than_explicit_limit() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let deep_path = format!("{}/note.md", ["a", "b", "c", "d", "e"].join("/"));
        let commit = store
            .commit_changes(
                "v1",
                None,
                &[FileChange::Upsert {
                    path: deep_path,
                    file: StoredFile::Text {
                        bytes: b"deep".to_vec(),
                    },
                }],
                "deep",
            )
            .await
            .unwrap();

        let err = store.list_tree("v1", Some(&commit)).await.unwrap_err();

        assert!(err.to_string().contains("tree depth"));
    }

    #[tokio::test]
    async fn rejects_file_directory_conflict_in_same_commit() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());

        let result = store
            .commit_changes(
                "v1",
                None,
                &[
                    FileChange::Upsert {
                        path: "notes".into(),
                        file: StoredFile::Text {
                            bytes: b"file".to_vec(),
                        },
                    },
                    FileChange::Upsert {
                        path: "notes/todo.md".into(),
                        file: StoredFile::Text {
                            bytes: b"nested".to_vec(),
                        },
                    },
                ],
                "conflict",
            )
            .await;

        assert!(result.is_err(), "conflicting paths must not commit");
    }

    #[tokio::test]
    async fn rejects_file_directory_conflict_against_existing_tree() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let c1 = store
            .commit_changes(
                "v1",
                None,
                &[FileChange::Upsert {
                    path: "notes/todo.md".into(),
                    file: StoredFile::Text {
                        bytes: b"nested".to_vec(),
                    },
                }],
                "nested",
            )
            .await
            .unwrap();

        let result = store
            .commit_changes(
                "v1",
                Some(&c1),
                &[FileChange::Upsert {
                    path: "notes".into(),
                    file: StoredFile::Text {
                        bytes: b"file".to_vec(),
                    },
                }],
                "conflict",
            )
            .await;

        assert!(result.is_err(), "conflicting paths must not commit");
    }

    #[tokio::test]
    async fn list_tree_map_contains_paths() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let c = store
            .commit_changes(
                "v1",
                None,
                &[FileChange::Upsert {
                    path: "a.md".into(),
                    file: StoredFile::Text {
                        bytes: b"a".to_vec(),
                    },
                }],
                "c",
            )
            .await
            .unwrap();
        let map = store.list_tree_map("v1", Some(&c)).await.unwrap();
        assert!(map.contains_key("a.md"));
    }

    #[test]
    fn repo_path_rejects_traversal_vault_ids() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());

        let result = store.repo_path("../outside");

        assert!(matches!(result, Err(GitStoreError::InvalidVaultId)));
    }
}
