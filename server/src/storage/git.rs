use crate::storage::text_kind::TextClassifier;
use async_trait::async_trait;
use git2::{ObjectType, Oid, Repository, Signature, Tree};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MAIN_REF: &str = "refs/heads/main";
const POINTER_MAGIC_KEY: &str = "pkvsync_pointer";
const POINTER_VERSION: u64 = 1;

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

    fn repo_path(&self, vault_id: &str) -> PathBuf {
        self.root.join(vault_id)
    }

    pub async fn list_tree_map(
        &self,
        vault_id: &str,
        at: Option<&str>,
    ) -> Result<std::collections::BTreeMap<String, TreeEntry>, GitStoreError> {
        let entries = self.list_tree(vault_id, at).await?;
        Ok(entries.into_iter().map(|e| (e.path.clone(), e)).collect())
    }
}

fn sig() -> Result<Signature<'static>, git2::Error> {
    Signature::now("PKV Sync", "pkv-sync@example.invalid")
}

fn init_bare_main(path: &Path) -> Result<Repository, GitStoreError> {
    let repo = Repository::init_bare(path)?;
    repo.set_head(MAIN_REF)?;
    Ok(repo)
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

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

fn decode_file(path: &str, bytes: Vec<u8>) -> StoredFile {
    if let Some(pointer) = is_pointer_bytes(&bytes) {
        return pointer;
    }
    if !TextClassifier::default().is_text_path(path) {
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

    fn insert(parts: &[&str], file: &StoredFile, node: &mut BTreeMap<String, TreeNode>) {
        if parts.len() == 1 {
            node.insert(parts[0].to_string(), TreeNode::File(file.clone()));
        } else {
            let child = node
                .entry(parts[0].to_string())
                .or_insert_with(|| TreeNode::Dir(BTreeMap::new()));
            if let TreeNode::Dir(map) = child {
                insert(&parts[1..], file, map);
            }
        }
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
        insert(&parts, file, &mut root);
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
                    if TextClassifier::default().is_text_path(&path) {
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
        let p = self.repo_path(vault_id);
        tokio::task::spawn_blocking(move || -> Result<(), GitStoreError> {
            if !p.exists() {
                std::fs::create_dir_all(&p)?;
                init_bare_main(&p)?;
            }
            Ok(())
        })
        .await
        .map_err(|_| GitStoreError::Panic)?
    }

    async fn head(&self, vault_id: &str) -> Result<Option<String>, GitStoreError> {
        let p = self.repo_path(vault_id);
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
        let p = self.repo_path(vault_id);
        let changes = changes.to_vec();
        let message = message.to_string();
        let parent = parent.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || -> Result<String, GitStoreError> {
            if !p.exists() {
                std::fs::create_dir_all(&p)?;
                init_bare_main(&p)?;
            }
            let repo = Repository::open_bare(&p)?;
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
        let p = self.repo_path(vault_id);
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
        let p = self.repo_path(vault_id);
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
}
