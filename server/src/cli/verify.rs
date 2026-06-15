use crate::config::{Config, LoggingConfig, NetworkConfig, ServerConfig, StorageConfig};
use crate::storage::blob::{is_sha256_hex, sharded_blob_path};
use git2::{ObjectType, Repository};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::ConnectOptions;
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct VerifyReport {
    pub referenced_blobs: BTreeSet<String>,
    pub checked_blobs: u64,
    pub missing_blobs: Vec<String>,
    pub corrupt_blobs: Vec<String>,
    pub orphan_blobs: Vec<String>,
    pub git_errors: Vec<String>,
    pub vaults_checked: u64,
}

impl VerifyReport {
    pub fn is_clean(&self) -> bool {
        self.missing_blobs.is_empty() && self.corrupt_blobs.is_empty() && self.git_errors.is_empty()
    }

    pub fn should_exit_success(&self, no_fail: bool) -> bool {
        no_fail || self.is_clean()
    }

    pub fn print(&self) {
        print!("{}", self.render());
        for hash in &self.missing_blobs {
            eprintln!("missing blob: {hash}");
        }
        for hash in &self.corrupt_blobs {
            eprintln!("corrupt blob: {hash}");
        }
        for err in &self.git_errors {
            eprintln!("git error: {err}");
        }
        for hash in &self.orphan_blobs {
            println!("orphan blob: {hash}");
        }
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("Blob storage:\n");
        out.push_str(&format!("  references: {}\n", self.referenced_blobs.len()));
        out.push_str(&format!("  checked files: {}\n", self.checked_blobs));
        out.push_str(&format!("  missing files: {}\n", self.missing_blobs.len()));
        out.push_str(&format!(
            "  corrupt files (SHA mismatch): {}\n",
            self.corrupt_blobs.len()
        ));
        out.push_str(&format!("  orphans: {}\n\n", self.orphan_blobs.len()));
        out.push_str("Git repositories:\n");
        out.push_str(&format!("  vaults checked: {}\n", self.vaults_checked));
        out.push_str(&format!("  errors: {}\n\n", self.git_errors.len()));
        out.push_str("Verdict: ");
        if self.is_clean() {
            if self.orphan_blobs.is_empty() {
                out.push_str("clean.");
            } else {
                out.push_str(&format!(
                    "{} orphan blob(s) found; not blocking.",
                    self.orphan_blobs.len()
                ));
            }
        } else {
            out.push_str("verification failed.");
        }
        out.push('\n');
        out
    }
}

pub fn run(config: &Config) -> anyhow::Result<VerifyReport> {
    run_data_dir(&config.storage.data_dir, &config.storage.db_path)
}

pub fn run_data_dir(data_dir: &Path, db_path: &Path) -> anyhow::Result<VerifyReport> {
    let mut report = VerifyReport::default();
    if let Err(err) = collect_db_blob_refs(db_path, &mut report) {
        report
            .git_errors
            .push(format!("database blob_refs query failed: {err}"));
    }
    verify_git_repos(&data_dir.join("vaults"), &mut report)?;
    verify_blob_store(&data_dir.join("blobs"), &mut report)?;
    Ok(report)
}

pub fn config_for_data_dir(data_dir: &Path) -> Config {
    Config {
        server: ServerConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            deployment_key: "k_verify".to_string(),
            public_host: None,
        },
        storage: StorageConfig {
            data_dir: data_dir.to_path_buf(),
            db_path: data_dir.join("metadata.db"),
        },
        network: NetworkConfig {
            trusted_proxies: Vec::new(),
        },
        logging: LoggingConfig::default(),
        update_check: Default::default(),
        mcp: Default::default(),
    }
}

fn collect_db_blob_refs(db_path: &Path, report: &mut VerifyReport) -> anyhow::Result<()> {
    if !db_path.exists() {
        return Ok(());
    }
    let db_path = db_path.to_path_buf();
    let hashes = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async move {
            let opts = SqliteConnectOptions::new()
                .filename(&db_path)
                .create_if_missing(false)
                .disable_statement_logging();
            let pool = SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(opts)
                .await?;
            let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT blob_hash FROM blob_refs")
                .fetch_all(&pool)
                .await?;
            pool.close().await;
            Ok::<_, anyhow::Error>(rows.into_iter().map(|row| row.0).collect::<Vec<_>>())
        })
    })
    .join()
    .map_err(|_| anyhow::anyhow!("SQLite verify task panicked"))??;

    for hash in hashes {
        if is_sha256_hex(&hash) {
            report.referenced_blobs.insert(hash);
        } else {
            report
                .corrupt_blobs
                .push(format!("invalid blob_refs hash: {hash}"));
        }
    }
    Ok(())
}

fn verify_git_repos(vaults_dir: &Path, report: &mut VerifyReport) -> anyhow::Result<()> {
    if !vaults_dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(vaults_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let vault_id = entry.file_name().to_string_lossy().to_string();
        report.vaults_checked += 1;
        match inspect_repo(entry.path()) {
            Ok(()) => {}
            Err(err) => report.git_errors.push(format!("{vault_id}: {err}")),
        }
    }
    Ok(())
}

fn inspect_repo(path: PathBuf) -> anyhow::Result<()> {
    let repo = Repository::open_bare(&path)?;
    let head = match repo.head() {
        Ok(head) => head,
        Err(err) if err.code() == git2::ErrorCode::UnbornBranch => return Ok(()),
        Err(err) if err.code() == git2::ErrorCode::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    let Some(oid) = head.target() else {
        return Ok(());
    };
    let commit = repo.find_commit(oid)?;
    let tree = commit.tree()?;
    walk_tree_for_integrity(&repo, &tree, 0)?;
    Ok(())
}

const VERIFY_MAX_TREE_DEPTH: usize = 256;

fn walk_tree_for_integrity(
    repo: &Repository,
    tree: &git2::Tree<'_>,
    depth: usize,
) -> anyhow::Result<()> {
    if depth > VERIFY_MAX_TREE_DEPTH {
        anyhow::bail!("tree depth exceeds {VERIFY_MAX_TREE_DEPTH}");
    }
    for entry in tree.iter() {
        match entry.kind() {
            Some(ObjectType::Tree) => {
                let sub = repo.find_tree(entry.id())?;
                walk_tree_for_integrity(repo, &sub, depth + 1)?;
            }
            Some(ObjectType::Blob) => {
                repo.find_blob(entry.id())?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn verify_blob_store(blobs_dir: &Path, report: &mut VerifyReport) -> anyhow::Result<()> {
    let mut seen = HashSet::new();
    if blobs_dir.exists() {
        for entry in walkdir::WalkDir::new(blobs_dir) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_sha256_hex(&name) {
                continue;
            }
            seen.insert(name.clone());
            report.checked_blobs += 1;
            let bytes = fs::read(entry.path())?;
            let actual = hex::encode(Sha256::digest(&bytes));
            if actual != name {
                report.corrupt_blobs.push(name);
            }
        }
    }

    for hash in &report.referenced_blobs {
        let Some(path) = sharded_blob_path(blobs_dir, hash) else {
            continue;
        };
        if !path.exists() {
            report.missing_blobs.push(hash.clone());
        }
    }

    let referenced: HashSet<&str> = report.referenced_blobs.iter().map(String::as_str).collect();
    report.orphan_blobs = seen
        .into_iter()
        .filter(|hash| !referenced.contains(hash.as_str()))
        .collect();
    report.orphan_blobs.sort();
    report.missing_blobs.sort();
    report.corrupt_blobs.sort();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sharded_blob_path_rejects_invalid_hash() {
        assert!(sharded_blob_path(Path::new("/data/blobs"), "../not-a-sha").is_none());
    }

    #[test]
    fn verify_run_does_not_accept_no_fail_flag() {
        let verify_source = include_str!("verify.rs");
        let main_source = include_str!("../main.rs");
        let restore_source = include_str!("restore.rs");
        let ops_cli_source = include_str!("../../tests/ops_cli.rs");
        let run_with_flag_signature = concat!("pub fn run(config: &Config, _", ": bool)");
        let main_call_with_flag = concat!("verify::run(&cfg, no", "_fail)");
        let restore_call_with_flag = concat!("verify::run(&cfg, false)");
        let test_call_with_false = concat!("verify", "::", "run(&cfg, false)");
        let test_call_with_true = concat!("verify", "::", "run(&cfg, true)");

        assert!(!verify_source.contains(run_with_flag_signature));
        assert!(!main_source.contains(main_call_with_flag));
        assert!(!restore_source.contains(restore_call_with_flag));
        assert!(!ops_cli_source.contains(test_call_with_false));
        assert!(!ops_cli_source.contains(test_call_with_true));
    }
}
