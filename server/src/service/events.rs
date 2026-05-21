use dashmap::DashMap;
use git2::{Delta, Oid, Repository};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;

pub const MAX_SSE_REPLAY_COMMITS: usize = 64;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum EventChange {
    #[serde(rename = "text_inline")]
    TextInline { path: String, content: String },
    #[serde(rename = "text_ref")]
    TextRef { path: String, size: u64 },
    #[serde(rename = "blob")]
    Blob {
        path: String,
        blob_hash: String,
        size: u64,
    },
    #[serde(rename = "delete")]
    Delete { path: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct VaultEvent {
    pub commit: String,
    pub parent: Option<String>,
    pub source_device_id: String,
    pub at: i64,
    pub changes: Vec<EventChange>,
}

#[derive(Debug, Clone)]
pub enum ReplayEvents {
    Events(Vec<VaultEvent>),
    Lagged,
}

#[derive(Clone)]
pub struct VaultEventBus {
    inner: Arc<DashMap<String, broadcast::Sender<VaultEvent>>>,
    capacity: usize,
}

impl VaultEventBus {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
            capacity,
        }
    }

    pub fn subscribe(&self, vault_id: &str) -> broadcast::Receiver<VaultEvent> {
        self.inner
            .entry(vault_id.to_string())
            .or_insert_with(|| broadcast::channel(self.capacity).0)
            .subscribe()
    }

    pub fn publish(&self, vault_id: &str, event: VaultEvent) {
        if let Some(tx) = self.inner.get(vault_id) {
            let _ = tx.send(event);
        }
    }
}

pub async fn replay_events_after(
    vault_root: PathBuf,
    vault_id: &str,
    last_event_id: &str,
) -> anyhow::Result<ReplayEvents> {
    let vault_path = vault_root.join(vault_id);
    let last = match Oid::from_str(last_event_id) {
        Ok(oid) => oid,
        Err(_) => return Ok(ReplayEvents::Events(Vec::new())),
    };
    tokio::task::spawn_blocking(move || replay_events_after_blocking(vault_path, last))
        .await
        .map_err(|_| anyhow::anyhow!("blocking task panicked"))?
}

fn replay_events_after_blocking(vault_path: PathBuf, last: Oid) -> anyhow::Result<ReplayEvents> {
    let repo = match Repository::open_bare(vault_path) {
        Ok(repo) => repo,
        Err(_) => return Ok(ReplayEvents::Events(Vec::new())),
    };
    if repo.find_commit(last).is_err() {
        return Ok(ReplayEvents::Events(Vec::new()));
    }
    let mut walk = repo.revwalk()?;
    walk.push_head()?;
    let mut commits = Vec::new();
    let mut found_last = false;
    for oid in walk {
        let oid = oid?;
        if oid == last {
            found_last = true;
            break;
        }
        if commits.len() >= MAX_SSE_REPLAY_COMMITS {
            return Ok(ReplayEvents::Lagged);
        }
        commits.push(oid);
    }
    if !found_last {
        return Ok(ReplayEvents::Events(Vec::new()));
    }
    commits.reverse();

    let mut out = Vec::new();
    for oid in commits {
        let commit = repo.find_commit(oid)?;
        let parent = if commit.parent_count() > 0 {
            Some(commit.parent_id(0)?.to_string())
        } else {
            None
        };
        let changes = replay_changes_for_commit(&repo, &commit)?;
        out.push(VaultEvent {
            commit: oid.to_string(),
            parent,
            source_device_id: replay_source_device(commit.message().unwrap_or("")),
            at: commit.time().seconds(),
            changes,
        });
    }
    Ok(ReplayEvents::Events(out))
}

fn replay_changes_for_commit(
    repo: &Repository,
    commit: &git2::Commit<'_>,
) -> anyhow::Result<Vec<EventChange>> {
    let new_tree = commit.tree()?;
    let old_commit = if commit.parent_count() > 0 {
        Some(commit.parent(0)?)
    } else {
        None
    };
    let old_tree = old_commit
        .as_ref()
        .map(|commit| commit.tree())
        .transpose()?;
    let diff = repo.diff_tree_to_tree(old_tree.as_ref(), Some(&new_tree), None)?;
    let mut changes = Vec::new();
    for delta in diff.deltas() {
        match delta.status() {
            Delta::Deleted => {
                if let Some(path) = delta.old_file().path().and_then(display_path) {
                    changes.push(EventChange::Delete { path });
                }
            }
            Delta::Added | Delta::Modified | Delta::Typechange | Delta::Renamed | Delta::Copied => {
                if let Some(path) = delta.new_file().path().and_then(display_path) {
                    let size = new_tree
                        .get_path(Path::new(&path))
                        .ok()
                        .and_then(|entry| repo.find_blob(entry.id()).ok())
                        .map(|blob| blob.content().len() as u64)
                        .unwrap_or(0);
                    changes.push(EventChange::TextRef { path, size });
                }
            }
            _ => {}
        }
    }
    Ok(changes)
}

fn display_path(path: &Path) -> Option<String> {
    Some(path.to_string_lossy().replace('\\', "/"))
}

fn replay_source_device(message: &str) -> String {
    message
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("sync: "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("replay")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn subscribe_receives_published_event() {
        let bus = VaultEventBus::new(16);
        let mut rx = bus.subscribe("vault1");
        let event = VaultEvent {
            commit: "abc".into(),
            parent: None,
            source_device_id: "dev1".into(),
            at: 0,
            changes: vec![EventChange::TextInline {
                path: "note.md".into(),
                content: "hello".into(),
            }],
        };
        bus.publish("vault1", event.clone());
        let received = rx.try_recv().unwrap();
        assert_eq!(received.commit, "abc");
    }

    #[tokio::test]
    async fn publish_without_receiver_does_not_panic() {
        let bus = VaultEventBus::new(16);
        let event = VaultEvent {
            commit: "abc".into(),
            parent: None,
            source_device_id: "dev1".into(),
            at: 0,
            changes: vec![],
        };
        bus.publish("nonexistent", event);
    }

    #[tokio::test]
    async fn two_receivers_both_get_event() {
        let bus = VaultEventBus::new(16);
        let mut rx1 = bus.subscribe("vault1");
        let mut rx2 = bus.subscribe("vault1");
        let event = VaultEvent {
            commit: "abc".into(),
            parent: None,
            source_device_id: "dev1".into(),
            at: 0,
            changes: vec![],
        };
        bus.publish("vault1", event.clone());
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }

    #[tokio::test]
    async fn capacity_overflow_yields_lagged() {
        let bus = VaultEventBus::new(4);
        let mut rx = bus.subscribe("vault1");
        for i in 0..10u64 {
            bus.publish(
                "vault1",
                VaultEvent {
                    commit: format!("c{i}"),
                    parent: None,
                    source_device_id: "dev1".into(),
                    at: i as i64,
                    changes: vec![],
                },
            );
        }
        let result = rx.try_recv();
        assert!(matches!(
            result,
            Err(broadcast::error::TryRecvError::Lagged(_))
        ));
    }
}
