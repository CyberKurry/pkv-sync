use anyhow::{anyhow, bail, Context};
use reqwest::header::{ACCEPT, USER_AGENT};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const REPO: &str = "cyberkurry/pkv-sync";
const IMAGE: &str = "ghcr.io/cyberkurry/pkv-sync";

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub dry_run: bool,
    pub yes: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerProbe {
    pub docker_env_exists: bool,
    pub kubernetes_service_host: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradePlan {
    pub version: String,
    pub asset_name: String,
    pub target_path: PathBuf,
    pub release_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

pub async fn run(options: RunOptions) -> anyhow::Result<()> {
    if let Err(err) = try_run(options).await {
        println!("Upgrade could not complete: {err}");
        println!();
        println!("{}", manual_upgrade_guidance());
    }
    Ok(())
}

async fn try_run(options: RunOptions) -> anyhow::Result<()> {
    let probe = current_container_probe();
    if probe.is_container() {
        println!("{}", container_guidance(&probe));
        return Ok(());
    }

    let triple = target_triple();
    let asset_name = asset_name_for(triple)
        .ok_or_else(|| anyhow!("no PKV Sync server binary asset is published for {triple}"))?;
    let current_binary =
        env::current_exe().context("locating the currently running pkvsyncd binary")?;
    let target_path = new_binary_path_for(&current_binary);
    let client = http_client()?;
    let release = fetch_release(&client, options.version.as_deref()).await?;
    let version = normalize_release_tag(&release.tag_name)
        .ok_or_else(|| anyhow!("release tag '{}' is not a stable version", release.tag_name))?;
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == asset_name)
        .cloned()
        .ok_or_else(|| {
            anyhow!(
                "release v{version} does not include asset {asset_name}; download manually from {}",
                release.html_url
            )
        })?;
    let plan = UpgradePlan {
        version,
        asset_name,
        target_path,
        release_url: release.html_url.clone(),
    };

    if options.dry_run {
        println!("{}", render_dry_run(&plan));
        return Ok(());
    }
    if !options.yes && !confirm_download(&plan)? {
        println!("Upgrade cancelled. No files were changed.");
        return Ok(());
    }

    let checksum = expected_sha256(&client, &release, &plan.asset_name).await?;
    let bytes = download_verified_binary(&client, &asset, &plan.target_path, &checksum).await?;
    println!(
        "Downloaded {} bytes for PKV Sync v{} to {}.",
        bytes,
        plan.version,
        plan.target_path.display()
    );
    println!("Verified SHA256: {checksum}");
    println!();
    println!("{}", render_next_steps(&plan));
    Ok(())
}

pub fn target_triple() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "aarch64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64")
    )))]
    {
        "unsupported"
    }
}

pub fn asset_name_for(triple: &str) -> Option<String> {
    match triple {
        "x86_64-unknown-linux-gnu" | "aarch64-unknown-linux-gnu" => {
            Some(format!("pkvsyncd-{triple}"))
        }
        "x86_64-pc-windows-msvc" => Some("pkvsyncd-x86_64-pc-windows-msvc.exe".into()),
        _ => None,
    }
}

pub fn new_binary_path_for(current_binary: &Path) -> PathBuf {
    let file_name = current_binary
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("pkvsyncd");
    let new_name = if cfg!(windows) && file_name.to_ascii_lowercase().ends_with(".exe") {
        let stem = &file_name[..file_name.len() - 4];
        format!("{stem}.new.exe")
    } else {
        format!("{file_name}.new")
    };
    current_binary
        .parent()
        .map(|parent| parent.join(&new_name))
        .unwrap_or_else(|| PathBuf::from(new_name))
}

pub fn parse_sha256sums(content: &str, asset_name: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(hash) = parts.next() else {
            continue;
        };
        if !is_sha256_hex(hash) {
            continue;
        }
        let Some(path) = parts.next() else {
            continue;
        };
        let normalized = path.trim_start_matches('*').replace('\\', "/");
        let base_name = normalized
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
            .unwrap_or(&normalized);
        if normalized == asset_name || base_name == asset_name {
            return Some(hash.to_ascii_lowercase());
        }
    }
    None
}

pub fn container_guidance(probe: &ContainerProbe) -> String {
    let detected = match (
        probe.docker_env_exists,
        probe.kubernetes_service_host.as_deref(),
    ) {
        (true, Some(_)) => "Docker/Kubernetes",
        (true, None) => "Docker",
        (false, Some(_)) => "Kubernetes",
        (false, None) => "container",
    };
    format!(
        "PKV Sync appears to be running inside {detected}.\n\n\
         Binary self-upgrade is disabled for Docker/Kubernetes deployments. Upgrade the image instead:\n\
           docker pull {IMAGE}:latest\n\
           docker compose pull && docker compose up -d\n\n\
         For Kubernetes, update the image tag in your workload and restart the rollout."
    )
}

pub fn render_dry_run(plan: &UpgradePlan) -> String {
    format!(
        "PKV Sync upgrade dry run\n\n\
         Release: v{}\n\
         Asset: {}\n\
         Target: {}\n\
         Notes: {}\n\n\
         No files were downloaded or changed.\n\n{}",
        plan.version,
        plan.asset_name,
        plan.target_path.display(),
        plan.release_url,
        render_next_steps(plan)
    )
}

fn render_next_steps(plan: &UpgradePlan) -> String {
    let current_binary = current_binary_path_from_new(&plan.target_path);
    format!(
        "NEXT STEPS:\n\
         1. Stop pkvsyncd.\n\
         2. Replace the current binary with the verified side-by-side binary.\n\
         3. Start pkvsyncd again.\n\n\
         systemd example:\n\
           sudo systemctl stop pkvsyncd\n\
           sudo install -m 0755 \"{}\" \"{}\"\n\
           sudo systemctl start pkvsyncd\n\n\
         Manual example:\n\
           stop pkvsyncd, replace \"{}\" with \"{}\", then start pkvsyncd.",
        plan.target_path.display(),
        current_binary.display(),
        current_binary.display(),
        plan.target_path.display()
    )
}

fn current_binary_path_from_new(new_binary: &Path) -> PathBuf {
    let file_name = new_binary
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("pkvsyncd.new");
    let current_name = if file_name.to_ascii_lowercase().ends_with(".new.exe") {
        format!("{}.exe", &file_name[..file_name.len() - 8])
    } else if let Some(stem) = file_name.strip_suffix(".new") {
        stem.to_string()
    } else {
        "pkvsyncd".to_string()
    };
    new_binary
        .parent()
        .map(|parent| parent.join(&current_name))
        .unwrap_or_else(|| PathBuf::from(current_name))
}

fn current_container_probe() -> ContainerProbe {
    ContainerProbe {
        docker_env_exists: Path::new("/.dockerenv").exists(),
        kubernetes_service_host: env::var("KUBERNETES_SERVICE_HOST").ok(),
    }
}

impl ContainerProbe {
    fn is_container(&self) -> bool {
        self.docker_env_exists || self.kubernetes_service_host.is_some()
    }
}

fn http_client() -> anyhow::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .context("creating HTTP client")
}

async fn fetch_release(
    client: &reqwest::Client,
    requested_version: Option<&str>,
) -> anyhow::Result<GitHubRelease> {
    let url = match requested_version {
        Some(version) => format!(
            "https://api.github.com/repos/{REPO}/releases/tags/v{}",
            normalize_requested_version(version)?
        ),
        None => format!("https://api.github.com/repos/{REPO}/releases/latest"),
    };
    let response = github_get(client, &url)
        .send()
        .await
        .with_context(|| format!("requesting {url}"))?;
    if !response.status().is_success() {
        bail!("GitHub release lookup returned {}", response.status());
    }
    response
        .json::<GitHubRelease>()
        .await
        .context("parsing GitHub release response")
}

async fn expected_sha256(
    client: &reqwest::Client,
    release: &GitHubRelease,
    asset_name: &str,
) -> anyhow::Result<String> {
    if let Some(hash) = parse_sha256sums(&release.body, asset_name) {
        return Ok(hash);
    }
    let checksum_asset = release
        .assets
        .iter()
        .find(|asset| asset.name == "SHA256SUMS")
        .or_else(|| {
            release.assets.iter().find(|asset| {
                asset.name == format!("{asset_name}.sha256")
                    || asset.name == format!("{asset_name}.sha256sum")
            })
        })
        .ok_or_else(|| anyhow!("release does not include SHA256SUMS for {asset_name}"))?;
    let text = download_text(client, &checksum_asset.browser_download_url).await?;
    parse_sha256sums(&text, asset_name)
        .ok_or_else(|| anyhow!("SHA256SUMS does not include {asset_name}"))
}

async fn download_text(client: &reqwest::Client, url: &str) -> anyhow::Result<String> {
    let response = github_get(client, url)
        .send()
        .await
        .with_context(|| format!("downloading {url}"))?;
    if !response.status().is_success() {
        bail!("checksum download returned {}", response.status());
    }
    response.text().await.context("reading checksum response")
}

async fn download_verified_binary(
    client: &reqwest::Client,
    asset: &GitHubAsset,
    target_path: &Path,
    expected_sha256: &str,
) -> anyhow::Result<u64> {
    ensure_target_parent(target_path)?;
    let temp_path = download_temp_path(target_path);
    let mut response = github_get(client, &asset.browser_download_url)
        .send()
        .await
        .with_context(|| format!("downloading {}", asset.browser_download_url))?;
    if !response.status().is_success() {
        bail!("binary download returned {}", response.status());
    }

    let mut file = fs::File::create(&temp_path)
        .with_context(|| format!("creating {}", temp_path.display()))?;
    let mut hasher = Sha256::new();
    let mut bytes_written = 0_u64;
    while let Some(chunk) = response.chunk().await.context("reading binary download")? {
        hasher.update(&chunk);
        file.write_all(&chunk)
            .with_context(|| format!("writing {}", temp_path.display()))?;
        bytes_written += chunk.len() as u64;
    }
    file.flush()
        .with_context(|| format!("flushing {}", temp_path.display()))?;
    drop(file);

    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(expected_sha256) {
        let _ = fs::remove_file(&temp_path);
        bail!(
            "checksum mismatch for {}: expected {expected_sha256}, got {actual}",
            asset.name
        );
    }
    set_executable(&temp_path)?;
    if target_path.exists() {
        fs::remove_file(target_path)
            .with_context(|| format!("removing existing {}", target_path.display()))?;
    }
    fs::rename(&temp_path, target_path).with_context(|| {
        format!(
            "moving verified binary from {} to {}",
            temp_path.display(),
            target_path.display()
        )
    })?;
    Ok(bytes_written)
}

fn ensure_target_parent(target_path: &Path) -> anyhow::Result<()> {
    prepare_download_target(target_path)
}

pub fn prepare_download_target(target_path: &Path) -> anyhow::Result<()> {
    let parent = target_path.parent().ok_or_else(|| {
        anyhow!(
            "target path {} has no parent directory",
            target_path.display()
        )
    })?;
    if !parent.is_dir() {
        bail!("target directory {} does not exist", parent.display());
    }
    if target_path.is_dir() {
        bail!("target path {} is a directory", target_path.display());
    }
    let probe_path = parent.join(format!(
        ".pkvsyncd-upgrade-write-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe_path)
    {
        Ok(_) => {
            let _ = fs::remove_file(&probe_path);
        }
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            bail!("target directory {} is not writable", parent.display());
        }
        Err(err) => {
            return Err(err)
                .with_context(|| format!("checking writability of {}", parent.display()));
        }
    }
    Ok(())
}

fn download_temp_path(target_path: &Path) -> PathBuf {
    let file_name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("pkvsyncd.new");
    target_path
        .parent()
        .map(|parent| parent.join(format!("{file_name}.download")))
        .unwrap_or_else(|| PathBuf::from(format!("{file_name}.download")))
}

#[cfg(unix)]
fn set_executable(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .with_context(|| format!("reading permissions for {}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .with_context(|| format!("marking {} executable", path.display()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

fn github_get(client: &reqwest::Client, url: &str) -> reqwest::RequestBuilder {
    client
        .get(url)
        .header(USER_AGENT, format!("PKVSync/{}", env!("CARGO_PKG_VERSION")))
        .header(ACCEPT, "application/vnd.github+json")
}

fn normalize_requested_version(version: &str) -> anyhow::Result<String> {
    let version = version.trim().trim_start_matches('v');
    if version.is_empty()
        || version.contains('-')
        || !version.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        bail!("version must look like 0.9.1");
    }
    Ok(version.to_string())
}

fn normalize_release_tag(tag: &str) -> Option<String> {
    let version = tag.trim().trim_start_matches('v');
    if version.is_empty() || version.contains('-') {
        return None;
    }
    if version.chars().all(|c| c.is_ascii_digit() || c == '.') {
        Some(version.to_string())
    } else {
        None
    }
}

fn confirm_download(plan: &UpgradePlan) -> anyhow::Result<bool> {
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        println!("Refusing to download in a non-interactive shell without --yes.");
        return Ok(false);
    }
    print!(
        "Download PKV Sync v{} to {}? [y/N] ",
        plan.version,
        plan.target_path.display()
    );
    io::stdout().flush().ok();
    let mut answer = String::new();
    stdin
        .read_line(&mut answer)
        .context("reading confirmation")?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

fn manual_upgrade_guidance() -> String {
    format!(
        "Manual upgrade: open https://github.com/{REPO}/releases, download the matching pkvsyncd asset and SHA256SUMS, verify the checksum, then replace the binary while pkvsyncd is stopped."
    )
}
