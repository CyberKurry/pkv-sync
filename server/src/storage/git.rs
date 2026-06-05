use crate::storage::blob::is_sha256_hex;
use crate::storage::text_kind::TextClassifier;
use async_trait::async_trait;
use git2::{Delta, DiffFindOptions, ObjectType, Oid, Repository, Signature, Tree};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MAIN_REF: &str = "refs/heads/main";
pub const POINTER_MAGIC_KEY: &str = "pkvsync_pointer";
pub const POINTER_VERSION: u64 = 1;

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
}

#[derive(Clone)]
pub struct Git2VaultStore {
    root: PathBuf,
}

impl Git2VaultStore {
    pub fn new(root: PathBuf) -> Self {
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
            for oid in walk {
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

fn is_binary_delta(
    repo: &Repository,
    old_tree: Option<&Tree<'_>>,
    new_tree: &Tree<'_>,
    path: &str,
    old_path: Option<&str>,
) -> Result<bool, GitStoreError> {
    let classifier = TextClassifier::default_ref();
    let text_path = classifier.is_text_path(path)
        || old_path
            .map(|old_path| classifier.is_text_path(old_path))
            .unwrap_or(false);
    if !text_path {
        return Ok(true);
    }
    if tree_path_is_pointer(repo, Some(new_tree), path)? {
        return Ok(true);
    }
    if let Some(old_path) = old_path.or(Some(path)) {
        if tree_path_is_pointer(repo, old_tree, old_path)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn tree_path_is_pointer(
    repo: &Repository,
    tree: Option<&Tree<'_>>,
    path: &str,
) -> Result<bool, GitStoreError> {
    let Some(tree) = tree else {
        return Ok(false);
    };
    let Ok(entry) = tree.get_path(Path::new(path)) else {
        return Ok(false);
    };
    let blob = repo.find_blob(entry.id())?;
    Ok(is_pointer_bytes(blob.content()).is_some()
        || (!TextClassifier::default_ref().is_text_path(path)
            && is_legacy_pointer_bytes(blob.content()).is_some()))
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

fn encode_file(f: &StoredFile) -> Result<Vec<u8>, serde_json::Error> {
    match f {
        StoredFile::Text { bytes } => Ok(bytes.clone()),
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
    out: &mut BTreeMap<String, StoredFile>,
) -> Result<(), GitStoreError> {
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
                read_tree_recursive(repo, &subtree, &path, out)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn build_tree_recursive(
    repo: &Repository,
    files: &BTreeMap<String, StoredFile>,
) -> Result<Oid, GitStoreError> {
    #[derive(Clone)]
    enum TreeNode {
        File(StoredFile),
        Dir(BTreeMap<String, TreeNode>),
    }

    fn insert(
        full_path: &str,
        parts: &[&str],
        file: &StoredFile,
        node: &mut BTreeMap<String, TreeNode>,
    ) -> Result<(), GitStoreError> {
        if parts.len() == 1 {
            if matches!(node.get(parts[0]), Some(TreeNode::Dir(_))) {
                return Err(GitStoreError::PathConflict(full_path.to_string()));
            }
            node.insert(parts[0].to_string(), TreeNode::File(file.clone()));
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
        node: &BTreeMap<String, TreeNode>,
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
        insert(path, &parts, file, &mut root)?;
    }
    write_node(repo, &root)
}

fn tree_entries_recursive(
    repo: &Repository,
    tree: &Tree<'_>,
    prefix: &str,
    out: &mut Vec<TreeEntry>,
) -> Result<(), GitStoreError> {
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
                let bytes = blob.content();
                let pointer = is_pointer_bytes(bytes).or_else(|| {
                    if TextClassifier::default_ref().is_text_path(&path) {
                        None
                    } else {
                        is_legacy_pointer_bytes(bytes)
                    }
                });
                let size = match &pointer {
                    Some(StoredFile::BlobPointer { size, .. }) => *size,
                    _ => bytes.len() as u64,
                };
                out.push(TreeEntry {
                    path,
                    git_oid: entry.id().to_string(),
                    size,
                    is_blob_pointer: pointer.is_some(),
                });
            }
            Some(ObjectType::Tree) => {
                let subtree = repo.find_tree(entry.id())?;
                tree_entries_recursive(repo, &subtree, &path, out)?;
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
                read_tree_recursive(&repo, &tree, "", &mut current)?;
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
            let tree_oid = build_tree_recursive(&repo, &current)?;
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
            tree_entries_recursive(&repo, &tree, "", &mut out)?;
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
