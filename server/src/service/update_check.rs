use crate::config::UpdateCheckConfig;
use crate::db::repos::RuntimeConfig;
use crate::service::AppState;
use crate::version::{compare_versions, normalize_release_tag};
use serde::Deserialize;
use std::cmp::Ordering;
use std::time::Duration;

const NOTES_EXCERPT_CHARS: usize = 500;
const DISABLED_RECHECK_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdateCheckSchedule {
    Check(Duration),
    Disabled(Duration),
}

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
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let api_url = github_latest_release_url(&cfg.repo);
    let client = reqwest_client(&current_version);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(5)).await;
        loop {
            let sleep_for = match runtime_update_check_schedule(&state.runtime_cfg.snapshot().await)
            {
                UpdateCheckSchedule::Disabled(delay) => delay,
                UpdateCheckSchedule::Check(interval) => {
                    match check_once(&current_version, &api_url, &client).await {
                        Ok(Some(status)) => {
                            *state.update_status.write().await = Some(status);
                            *state.last_update_check_at.write().await =
                                Some(chrono::Utc::now().timestamp());
                        }
                        Ok(None) => {
                            // Either the remote response was a transient non-success
                            // (HTTP 4xx/5xx, rate-limit) or it definitively reports
                            // we're on the latest version. Either way, do not clobber
                            // a previously-known update banner; the next successful
                            // check will refresh it. We still record the timestamp so
                            // the dashboard can show "Last checked" liveness.
                            *state.last_update_check_at.write().await =
                                Some(chrono::Utc::now().timestamp());
                        }
                        Err(err) => {
                            tracing::debug!(error = %err, "failed to check for updates");
                        }
                    }
                    interval
                }
            };
            tokio::select! {
                _ = tokio::time::sleep(sleep_for) => {}
                _ = state.update_check_runtime_changed.notified() => {}
            }
        }
    });
}

fn runtime_update_check_schedule(cfg: &RuntimeConfig) -> UpdateCheckSchedule {
    if !cfg.update_check_enabled {
        return UpdateCheckSchedule::Disabled(DISABLED_RECHECK_INTERVAL);
    }
    UpdateCheckSchedule::Check(Duration::from_secs(
        cfg.update_check_interval_seconds.max(60),
    ))
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
    if compare_versions(&latest_version, &current) != Ordering::Greater {
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

fn excerpt(notes: &str) -> String {
    notes
        .trim()
        .chars()
        .take(NOTES_EXCERPT_CHARS)
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repos::RuntimeConfig;

    #[test]
    fn runtime_schedule_disables_checks_temporarily() {
        let cfg = RuntimeConfig {
            update_check_enabled: false,
            update_check_interval_seconds: 3600,
            ..RuntimeConfig::default()
        };

        assert_eq!(
            runtime_update_check_schedule(&cfg),
            UpdateCheckSchedule::Disabled(Duration::from_secs(60))
        );
    }

    #[test]
    fn runtime_schedule_clamps_enabled_interval_to_one_minute() {
        let cfg = RuntimeConfig {
            update_check_enabled: true,
            update_check_interval_seconds: 1,
            ..RuntimeConfig::default()
        };

        assert_eq!(
            runtime_update_check_schedule(&cfg),
            UpdateCheckSchedule::Check(Duration::from_secs(60))
        );
    }
}
