//! Materialize a vault's bare git repository into a plain file tree.
//!
//! Walks the git tree at the specified commit (default: HEAD) and writes each
//! file to the output directory. Text files are written as-is; binary files
//! (stored as `pkvsync_pointer` JSON) are resolved by copying the actual blob
//! from the server's sharded blob storage.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;
use crate::storage::blob::{is_sha256_hex, sharded_blob_path};
use crate::storage::git::{is_valid_vault_id, BlobPointerJson};

/// Expand a vault's git + blob storage into a plain file tree on disk.
///
/// # Arguments
///
/// * `config` - Server configuration (provides `data_dir` for vault/blob paths)
/// * `vault_id` - Identifier of the vault to materialize
/// * `output` - Destination directory (must not exist or be empty)
/// * `at` - Optional commit SHA; defaults to HEAD of the vault's main branch
///
/// # Errors
///
/// Returns an error if the output directory exists and is not empty, the vault
/// is not found, the commit SHA is invalid, or any blob file is missing.
pub fn run(config: &Config, vault_id: &str, output: &Path, at: Option<&str>) -> anyhow::Result<()> {
    validate_vault_id(vault_id)?;
    if output.exists() && fs::read_dir(output)?.next().is_some() {
        anyhow::bail!(
            "output directory exists and is not empty: {}",
            output.display()
        );
    }

    let repo_path = config.storage.data_dir.join("vaults").join(vault_id);
    if !repo_path.exists() {
        anyhow::bail!("vault not found: {}", vault_id);
    }

    let blobs_dir = config.storage.data_dir.join("blobs");

    let repo = git2::Repository::open_bare(&repo_path)?;
    let commit_oid = match at {
        Some(s) => git2::Oid::from_str(s)?,
        None => repo
            .head()?
            .target()
            .ok_or_else(|| anyhow::anyhow!("no HEAD in vault"))?,
    };
    let commit = repo.find_commit(commit_oid)?;
    let tree = commit.tree()?;

    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let staging = create_staging_dir(parent, output)?;
    if let Err(err) = walk_tree(&repo, &tree, &staging, &blobs_dir, Path::new("")) {
        let _ = fs::remove_dir_all(&staging);
        return Err(err);
    }

    if output.exists() {
        fs::remove_dir(output)?;
    }
    fs::rename(&staging, output)?;
    Ok(())
}

fn create_staging_dir(parent: &Path, output: &Path) -> anyhow::Result<PathBuf> {
    let name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("materialize");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for attempt in 0..16 {
        let path = parent.join(format!(
            ".{name}.pkvsync-materialize-{}-{nanos}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err.into()),
        }
    }
    anyhow::bail!("could not create materialize staging directory")
}

fn validate_vault_id(vault_id: &str) -> anyhow::Result<()> {
    if !is_valid_vault_id(vault_id) {
        anyhow::bail!("invalid vault id: {vault_id}");
    }
    Ok(())
}

/// Recursively walk a git tree, writing entries to the output directory.
///
/// For blob entries that are `pkvsync_pointer` JSON, the actual binary content
/// is copied from the sharded blob store (`blobs/<xx>/<xx>/<hash>`). Plain text
/// blobs are written directly.
fn walk_tree(
    repo: &git2::Repository,
    tree: &git2::Tree,
    out_root: &Path,
    blobs_dir: &Path,
    rel: &Path,
) -> anyhow::Result<()> {
    for entry in tree.iter() {
        let name = entry
            .name()
            .ok_or_else(|| anyhow::anyhow!("non-utf8 entry name"))?;
        validate_tree_entry_name(name)?;
        let entry_rel = rel.join(name);
        match entry.kind() {
            Some(git2::ObjectType::Tree) => {
                let sub = repo.find_tree(entry.id())?;
                walk_tree(repo, &sub, out_root, blobs_dir, &entry_rel)?;
            }
            Some(git2::ObjectType::Blob) => {
                let blob = repo.find_blob(entry.id())?;
                let content = blob.content();
                let out_path = out_root.join(&entry_rel);
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                if let Some(hash) = parse_pointer(content)? {
                    let blob_path = sharded_blob_path(blobs_dir, &hash)
                        .ok_or_else(|| anyhow::anyhow!("invalid blob hash: {hash}"))?;
                    if !blob_path.exists() {
                        anyhow::bail!("blob file missing: {} (for {})", hash, entry_rel.display());
                    }
                    fs::copy(&blob_path, &out_path)?;
                } else {
                    fs::write(&out_path, content)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_tree_entry_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
    {
        anyhow::bail!("unsafe git tree entry name: {name:?}");
    }
    Ok(())
}

/// Parse a `pkvsync_pointer` JSON blob and extract the SHA-256 hash.
///
/// Returns `Some(hash)` if the content is a valid pointer with version 1,
/// `None` otherwise (including non-JSON content, which is treated as text).
/// Errors if the pointer carries the magic key but the hash is malformed.
fn parse_pointer(content: &[u8]) -> anyhow::Result<Option<String>> {
    let Ok(ptr) = serde_json::from_slice::<BlobPointerJson>(content) else {
        return Ok(None);
    };
    if !ptr.has_magic() {
        return Ok(None);
    }
    if !is_sha256_hex(&ptr.blob) {
        anyhow::bail!("invalid blob pointer hash: {}", ptr.blob);
    }
    Ok(Some(ptr.blob))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pointer_valid() {
        let json = serde_json::json!({
            "pkvsync_pointer": 1,
            "blob": "a".repeat(64),
            "size": 42,
            "mime": "image/png"
        })
        .to_string()
        .into_bytes();
        let hash = parse_pointer(&json).unwrap().unwrap();
        assert_eq!(hash, "a".repeat(64));
    }

    #[test]
    fn parse_pointer_wrong_version() {
        let json = serde_json::json!({
            "pkvsync_pointer": 2,
            "blob": "a".repeat(64),
            "size": 42
        })
        .to_string()
        .into_bytes();
        assert!(parse_pointer(&json).unwrap().is_none());
    }

    #[test]
    fn parse_pointer_missing_version() {
        let json = serde_json::json!({
            "blob": "a".repeat(64),
            "size": 42
        })
        .to_string()
        .into_bytes();
        assert!(parse_pointer(&json).unwrap().is_none());
    }

    #[test]
    fn parse_pointer_missing_blob() {
        let json = serde_json::json!({
            "pkvsync_pointer": 1,
            "size": 42
        })
        .to_string()
        .into_bytes();
        assert!(parse_pointer(&json).unwrap().is_none());
    }

    #[test]
    fn parse_pointer_invalid_hash_returns_error() {
        let json = serde_json::json!({
            "pkvsync_pointer": 1,
            "blob": "abc",
            "size": 42
        })
        .to_string()
        .into_bytes();
        let err = parse_pointer(&json).unwrap_err();
        assert!(err.to_string().contains("invalid blob pointer hash"));
    }

    #[test]
    fn parse_pointer_non_json_returns_none() {
        assert!(parse_pointer(b"hello world").unwrap().is_none());
    }

    #[test]
    fn sharded_blob_path_matches_layout() {
        let blobs_dir = Path::new("/data/blobs");
        let hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        let path = sharded_blob_path(blobs_dir, hash).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/data/blobs/ab/cd/abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
        );
    }

    #[test]
    fn sharded_blob_path_rejects_invalid_hash() {
        assert!(sharded_blob_path(Path::new("/data/blobs"), "abc").is_none());
    }

    #[test]
    fn validate_tree_entry_name_rejects_path_traversal_segments() {
        for name in ["..", ".", "", "nested/file", r"nested\file"] {
            let err = validate_tree_entry_name(name).unwrap_err();
            assert!(err.to_string().contains("unsafe git tree entry name"));
        }
        assert!(validate_tree_entry_name("note.md").is_ok());
        assert!(validate_tree_entry_name("PKV Sync.md").is_ok());
    }
}
