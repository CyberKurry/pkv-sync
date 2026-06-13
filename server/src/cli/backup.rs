use crate::config::Config;
use chrono::{DateTime, Utc};
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::ConnectOptions;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

pub const MANIFEST_FILE: &str = "MANIFEST.json";
const PRIVATE_DIR_MODE: u32 = 0o700;
const PRIVATE_FILE_MODE: u32 = 0o600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(alias = "version")]
    pub manifest_schema: u64,
    pub pkvsyncd_version: String,
    pub created_at: DateTime<Utc>,
    pub source_data_dir: String,
    pub components: BTreeMap<String, ComponentManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentManifest {
    #[serde(alias = "hash")]
    pub sha256: String,
    pub size: u64,
    pub count: u64,
}

pub fn run(
    config: &Config,
    config_path: Option<&Path>,
    output: &Path,
    gzip: bool,
) -> anyhow::Result<Manifest> {
    if gzip {
        if output.exists() {
            anyhow::bail!("backup archive already exists: {}", output.display());
        }
        let staging = gzip_staging_dir(output)?;
        let result = run_to_dir(config, config_path, &staging);
        match result {
            Ok(manifest) => {
                write_tar_gz(&staging, output)?;
                fs::remove_dir_all(&staging)?;
                println!("backup archive written to {}", output.display());
                Ok(manifest)
            }
            Err(err) => {
                if staging.exists() {
                    let _ = fs::remove_dir_all(&staging);
                }
                Err(err)
            }
        }
    } else {
        run_to_dir(config, config_path, output)
    }
}

fn run_to_dir(
    config: &Config,
    config_path: Option<&Path>,
    output: &Path,
) -> anyhow::Result<Manifest> {
    ensure_absent_or_empty(output, "output directory")?;
    create_private_dir_all(output)?;

    let metadata_out = output.join("metadata.db");
    vacuum_into(&config.storage.db_path, &metadata_out)?;
    restrict_private_file(&metadata_out)?;

    copy_dir_if_exists(
        &config.storage.data_dir.join("vaults"),
        &output.join("vaults"),
    )?;
    copy_dir_if_exists(
        &config.storage.data_dir.join("blobs"),
        &output.join("blobs"),
    )?;
    if let Some(config_path) = config_path {
        if config_path.exists() {
            copy_private_file(config_path, &output.join("config.toml"))?;
        }
    }

    let manifest = build_manifest(config, output)?;
    let manifest_path = output.join(MANIFEST_FILE);
    write_private_file(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;
    println!(
        "backup written to {} ({} components)",
        output.display(),
        manifest.components.len()
    );
    Ok(manifest)
}

pub fn ensure_absent_or_empty(path: &Path, label: &str) -> anyhow::Result<()> {
    if path.exists() && fs::read_dir(path)?.next().is_some() {
        anyhow::bail!("{label} exists and is not empty: {}", path.display());
    }
    Ok(())
}

pub fn copy_dir_if_exists(src: &Path, dst: &Path) -> anyhow::Result<()> {
    if !src.exists() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(src).follow_links(false) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let out = dst.join(rel);
        if entry.file_type().is_symlink() {
            continue;
        } else if entry.file_type().is_dir() {
            create_private_dir_all(&out)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = out.parent() {
                create_private_dir_all(parent)?;
            }
            copy_private_file(entry.path(), &out)?;
        }
    }
    Ok(())
}

pub fn remove_dir_contents(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        let ty = entry.file_type()?;
        if ty.is_symlink() {
            remove_symlink_entry(&path)?;
        } else if ty.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn remove_symlink_entry(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        #[cfg(windows)]
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => fs::remove_dir(path),
        other => other,
    }
}

pub fn read_manifest(root: &Path) -> anyhow::Result<Manifest> {
    let path = root.join(MANIFEST_FILE);
    Ok(serde_json::from_slice(&fs::read(&path)?)?)
}

pub fn component_stats(path: &Path) -> anyhow::Result<ComponentManifest> {
    let mut files = Vec::new();
    if path.exists() {
        for entry in walkdir::WalkDir::new(path) {
            let entry = entry?;
            if entry.file_type().is_file() {
                let rel = entry
                    .path()
                    .strip_prefix(path)?
                    .to_string_lossy()
                    .replace('\\', "/");
                files.push((rel, entry.path().to_path_buf()));
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    let mut size = 0u64;
    for (rel, path) in &files {
        hasher.update(rel.as_bytes());
        hasher.update([0]);
        let bytes = fs::read(path)?;
        size = size.saturating_add(bytes.len() as u64);
        hasher.update(bytes);
        hasher.update([0]);
    }
    Ok(ComponentManifest {
        sha256: hex::encode(hasher.finalize()),
        size,
        count: files.len() as u64,
    })
}

pub fn file_component_stats(path: &Path) -> anyhow::Result<ComponentManifest> {
    let bytes = fs::read(path)?;
    Ok(ComponentManifest {
        sha256: hex::encode(Sha256::digest(&bytes)),
        size: bytes.len() as u64,
        count: 1,
    })
}

fn build_manifest(config: &Config, output: &Path) -> anyhow::Result<Manifest> {
    let mut components = BTreeMap::new();
    components.insert(
        "metadata.db".to_string(),
        file_component_stats(&output.join("metadata.db"))?,
    );
    components.insert(
        "vaults".to_string(),
        component_stats(&output.join("vaults"))?,
    );
    components.insert("blobs".to_string(), component_stats(&output.join("blobs"))?);
    let config_out = output.join("config.toml");
    if config_out.exists() {
        components.insert(
            "config.toml".to_string(),
            file_component_stats(&config_out)?,
        );
    }
    Ok(Manifest {
        manifest_schema: 1,
        pkvsyncd_version: env!("CARGO_PKG_VERSION").to_string(),
        created_at: Utc::now(),
        source_data_dir: config.storage.data_dir.to_string_lossy().to_string(),
        components,
    })
}

fn gzip_staging_dir(output: &Path) -> anyhow::Result<std::path::PathBuf> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("backup");
    let stamp = Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| Utc::now().timestamp_micros() * 1000);
    Ok(parent.join(format!(".{name}.staging-{stamp}")))
}

fn write_tar_gz(src_dir: &Path, output: &Path) -> anyhow::Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = create_private_file(output)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = tar::Builder::new(encoder);
    builder.append_dir_all(".", src_dir)?;
    let encoder = builder.into_inner()?;
    let file = encoder.finish()?;
    drop(file);
    restrict_private_file(output)?;
    Ok(())
}

fn vacuum_into(db_path: &Path, output: &Path) -> anyhow::Result<()> {
    let db_path = db_path.to_path_buf();
    let output = output.to_path_buf();
    if let Some(parent) = output.parent() {
        create_private_dir_all(parent)?;
    }
    let sqlite_output = output.clone();
    std::thread::spawn(move || {
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
            let output = sqlite_output.to_string_lossy().into_owned();
            sqlx::query("VACUUM INTO ?")
                .bind(output)
                .execute(&pool)
                .await?;
            pool.close().await;
            Ok::<_, anyhow::Error>(())
        })
    })
    .join()
    .map_err(|_| anyhow::anyhow!("SQLite backup task panicked"))??;
    restrict_private_file(&output)?;
    Ok(())
}

fn copy_private_file(src: &Path, dst: &Path) -> anyhow::Result<()> {
    if let Some(parent) = dst.parent() {
        create_private_dir_all(parent)?;
    }
    fs::copy(src, dst)?;
    restrict_private_file(dst)
}

fn write_private_file(path: &Path, bytes: Vec<u8>) -> anyhow::Result<()> {
    let mut file = create_private_file(path)?;
    file.write_all(&bytes)?;
    file.flush()?;
    drop(file);
    restrict_private_file(path)
}

fn create_private_file(path: &Path) -> anyhow::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(PRIVATE_FILE_MODE);
    }
    let file = options.open(path)?;
    restrict_private_file(path)?;
    Ok(file)
}

fn create_private_dir_all(path: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(path)?;
    restrict_private_dir(path)
}

fn restrict_private_dir(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_DIR_MODE))?;
    }
    #[cfg(not(unix))]
    {
        let _ = PRIVATE_DIR_MODE;
        let _ = path;
    }
    Ok(())
}

fn restrict_private_file(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_FILE_MODE))?;
    }
    #[cfg(not(unix))]
    {
        let _ = PRIVATE_FILE_MODE;
        let _ = path;
    }
    Ok(())
}
