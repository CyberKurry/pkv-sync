use async_trait::async_trait;
use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;
use tokio::io::AsyncWriteExt;

#[derive(Debug, thiserror::Error)]
pub enum BlobError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("invalid hash")]
    InvalidHash,
}

pub type BlobResult<T> = Result<T, BlobError>;

#[async_trait]
pub trait BlobStore: Send + Sync {
    async fn has(&self, hash: &str) -> BlobResult<bool>;
    async fn put_verified(&self, expected_hash: &str, bytes: Bytes) -> BlobResult<()>;
    async fn get(&self, hash: &str) -> BlobResult<Option<Bytes>>;
    async fn list_hashes(&self) -> BlobResult<HashSet<String>>;
    async fn delete(&self, hash: &str) -> BlobResult<bool>;
}

#[derive(Clone)]
pub struct LocalFsBlobStore {
    root: PathBuf,
}

impl LocalFsBlobStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn validate_hash(hash: &str) -> BlobResult<()> {
        if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(BlobError::InvalidHash);
        }
        Ok(())
    }

    fn path_for(&self, hash: &str) -> BlobResult<PathBuf> {
        Self::validate_hash(hash)?;
        Ok(self.root.join(&hash[0..2]).join(&hash[2..4]).join(hash))
    }

    pub fn sha256(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    pub async fn list_hashes_with_mtime(&self) -> BlobResult<Vec<(String, SystemTime)>> {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            if !root.exists() {
                return Ok(out);
            }
            for entry in walkdir::WalkDir::new(&root)
                .into_iter()
                .filter_map(Result::ok)
            {
                if entry.file_type().is_file() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if Self::validate_hash(&name).is_ok() {
                        let metadata = std::fs::metadata(entry.path())?;
                        out.push((name, metadata.modified()?));
                    }
                }
            }
            Ok(out)
        })
        .await
        .map_err(|e| BlobError::Io(std::io::Error::other(e)))?
    }
}

#[async_trait]
impl BlobStore for LocalFsBlobStore {
    async fn has(&self, hash: &str) -> BlobResult<bool> {
        Ok(self.path_for(hash)?.exists())
    }

    async fn put_verified(&self, expected_hash: &str, bytes: Bytes) -> BlobResult<()> {
        Self::validate_hash(expected_hash)?;
        let actual = Self::sha256(&bytes);
        if actual != expected_hash {
            return Err(BlobError::HashMismatch {
                expected: expected_hash.into(),
                actual,
            });
        }

        let path = self.path_for(expected_hash)?;
        if path.exists() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let tmp = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4().simple()));
        let mut f = tokio::fs::File::create(&tmp).await?;
        f.write_all(&bytes).await?;
        f.sync_all().await?;
        tokio::fs::rename(&tmp, &path).await?;
        Ok(())
    }

    async fn get(&self, hash: &str) -> BlobResult<Option<Bytes>> {
        let path = self.path_for(hash)?;
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(Bytes::from(tokio::fs::read(path).await?)))
    }

    async fn list_hashes(&self) -> BlobResult<HashSet<String>> {
        let mut set = HashSet::new();
        if !self.root.exists() {
            return Ok(set);
        }
        for entry in walkdir::WalkDir::new(&self.root)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.file_type().is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                if Self::validate_hash(&name).is_ok() {
                    set.insert(name);
                }
            }
        }
        Ok(set)
    }

    async fn delete(&self, hash: &str) -> BlobResult<bool> {
        let p = self.path_for(hash)?;
        if !p.exists() {
            return Ok(false);
        }
        tokio::fs::remove_file(p).await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[tokio::test]
    async fn put_get_has_delete_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalFsBlobStore::new(dir.path().join("blobs"));
        let data = Bytes::from_static(b"hello");
        let hash = LocalFsBlobStore::sha256(&data);
        assert!(!store.has(&hash).await.unwrap());
        store.put_verified(&hash, data.clone()).await.unwrap();
        assert!(store.has(&hash).await.unwrap());
        assert_eq!(store.get(&hash).await.unwrap().unwrap(), data);
        assert!(store.delete(&hash).await.unwrap());
        assert!(!store.has(&hash).await.unwrap());
    }

    #[tokio::test]
    async fn rejects_hash_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalFsBlobStore::new(dir.path().join("blobs"));
        let wrong = "0".repeat(64);
        let err = store
            .put_verified(&wrong, Bytes::from_static(b"hello"))
            .await
            .unwrap_err();
        assert!(matches!(err, BlobError::HashMismatch { .. }));
    }

    #[tokio::test]
    async fn list_hashes_returns_valid_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalFsBlobStore::new(dir.path().join("blobs"));
        let a = Bytes::from_static(b"a");
        let ah = LocalFsBlobStore::sha256(&a);
        store.put_verified(&ah, a).await.unwrap();
        let set = store.list_hashes().await.unwrap();
        assert!(set.contains(&ah));
    }
}
