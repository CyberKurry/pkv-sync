use crate::config::UpdateCheckConfig;
use crate::service::AppState;
use serde::Deserialize;
use std::time::Duration;

const NOTES_EXCERPT_CHARS: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateStatus {
    pub latest_version: String,
    pub current_version: String,
    pub release_url: String,
    pub notes_excerpt: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: String,
}

pub fn spawn_update_check(state: AppState, cfg: UpdateCheckConfig) {
    if !cfg.enabled {
        return;
    }
    let interval = Duration::from_secs(cfg.interval_seconds.max(60));
    let first_delay = interval.min(Duration::from_secs(5));
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let api_url = github_latest_release_url(&cfg.repo);
    let client = reqwest_client(&current_version);
    tokio::spawn(async move {
        tokio::time::sleep(first_delay).await;
        loop {
            match check_once(&current_version, &api_url, &client).await {
                Ok(status) => {
                    *state.update_status.write().await = status;
                }
                Err(err) => {
                    tracing::debug!(error = %err, "failed to check for updates");
                }
            }
            tokio::time::sleep(interval).await;
        }
    });
}

pub async fn check_once(
    current_version: &str,
    api_url: &str,
    client: &reqwest::Client,
) -> Result<Option<UpdateStatus>, reqwest::Error> {
    let response = client.get(api_url).send().await?;
    if !response.status().is_success() {
        tracing::debug!(status = %response.status(), "update check returned non-success status");
        return Ok(None);
    }
    let release = response.json::<GitHubRelease>().await?;
    let Some(latest_version) = normalize_release_tag(&release.tag_name) else {
        return Ok(None);
    };
    let Some(current) = normalize_release_tag(current_version) else {
        return Ok(None);
    };
    if compare_versions(&latest_version, &current) <= 0 {
        return Ok(None);
    }
    Ok(Some(UpdateStatus {
        latest_version,
        current_version: current,
        release_url: release.html_url,
        notes_excerpt: excerpt(&release.body),
    }))
}

fn reqwest_client(current_version: &str) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent(format!("PKVSync/{current_version}"))
        .build()
        .expect("update check HTTP client should build")
}

fn github_latest_release_url(repo: &str) -> String {
    format!(
        "https://api.github.com/repos/{}/releases/latest",
        repo.trim_matches('/')
    )
}

fn normalize_release_tag(tag: &str) -> Option<String> {
    let version = tag.trim().trim_start_matches('v');
    if version.is_empty() || version.contains('-') {
        return None;
    }
    parse_version(version)?;
    Some(version.to_string())
}

fn compare_versions(left: &str, right: &str) -> i8 {
    let left = parse_version(left).unwrap_or([0, 0, 0]);
    let right = parse_version(right).unwrap_or([0, 0, 0]);
    match left.cmp(&right) {
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
    }
}

fn parse_version(value: &str) -> Option<[u32; 3]> {
    let mut parts = value.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some([major, minor, patch])
}

fn excerpt(notes: &str) -> String {
    notes
        .trim()
        .chars()
        .take(NOTES_EXCERPT_CHARS)
        .collect::<String>()
}
