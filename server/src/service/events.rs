use dashmap::DashMap;
use git2::{Delta, Oid, Repository};
use serde::ser::SerializeStruct;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::storage::git::{parse_blob_pointer, storage_vault_path};

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

#[derive(Debug, Clone)]
pub enum EventKind {
    Commit,
    Rollback {
        from_commit: String,
        to_commit: String,
    },
}

#[derive(Debug, Clone)]
pub struct VaultEvent {
    pub commit: String,
    pub parent: Option<String>,
    pub source_device_id: String,
    pub at: i64,
    pub kind: EventKind,
    pub changes: Vec<EventChange>,
}

impl Serialize for VaultEvent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match &self.kind {
            EventKind::Commit => {
                let mut state = serializer.serialize_struct("VaultEvent", 6)?;
                state.serialize_field("commit", &self.commit)?;
                state.serialize_field("parent", &self.parent)?;
                state.serialize_field("source_device_id", &self.source_device_id)?;
                state.serialize_field("at", &self.at)?;
                state.serialize_field("kind", "commit")?;
                state.serialize_field("changes", &self.changes)?;
                state.end()
            }
            EventKind::Rollback {
                from_commit,
                to_commit,
            } => {
                let mut state = serializer.serialize_struct("VaultEvent", 7)?;
                state.serialize_field("commit", &self.commit)?;
                state.serialize_field("parent", &self.parent)?;
                state.serialize_field("source_device_id", &self.source_device_id)?;
                state.serialize_field("at", &self.at)?;
                state.serialize_field("kind", "rollback")?;
                state.serialize_field("from_commit", from_commit)?;
                state.serialize_field("to_commit", to_commit)?;
                state.end()
            }
        }
    }
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
        let should_prune = match self.inner.get(vault_id) {
            Some(tx) if tx.receiver_count() == 0 => true,
            Some(tx) => tx.send(event).is_err() && tx.receiver_count() == 0,
            None => false,
        };
        if should_prune {
            self.inner
                .remove_if(vault_id, |_, tx| tx.receiver_count() == 0);
        }
    }

    pub fn remove(&self, vault_id: &str) {
        self.inner.remove(vault_id);
    }

    /// Sum of live broadcast receivers across all vaults. Used by the admin
    /// dashboard's "Sync Status" card so the displayed number reflects actual
    /// SSE clients rather than a static placeholder. Receivers that have been
    /// dropped are not counted (tokio::broadcast tracks per-`Sender`).
    pub fn total_subscribers(&self) -> usize {
        self.prune_idle();
        self.inner
            .iter()
            .map(|entry| entry.value().receiver_count())
            .sum()
    }

    fn prune_idle(&self) {
        let idle_vaults: Vec<String> = self
            .inner
            .iter()
            .filter(|entry| entry.value().receiver_count() == 0)
            .map(|entry| entry.key().clone())
            .collect();
        for vault_id in idle_vaults {
            self.inner
                .remove_if(&vault_id, |_, tx| tx.receiver_count() == 0);
        }
    }

    #[cfg(test)]
    pub fn len_for_tests(&self) -> usize {
        self.inner.len()
    }
}

pub async fn replay_events_after(
    vault_root: &Path,
    vault_id: &str,
    last_event_id: &str,
) -> anyhow::Result<ReplayEvents> {
    let vault_path = match storage_vault_path(vault_root, vault_id) {
        Ok(path) => path,
        Err(_) => return Ok(ReplayEvents::Events(Vec::new())),
    };
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
        return Ok(ReplayEvents::Lagged);
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
            kind: EventKind::Commit,
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
                    let change = new_tree
                        .get_path(Path::new(&path))
                        .ok()
                        .and_then(|entry| repo.find_blob(entry.id()).ok())
                        .map(|blob| classify_replay_change(&path, blob.content()))
                        .unwrap_or(EventChange::TextRef {
                            path: path.clone(),
                            size: 0,
                        });
                    changes.push(change);
                }
            }
            _ => {}
        }
    }
    Ok(changes)
}

/// Decide whether a stored git blob represents an attachment (blob pointer JSON
/// emitted by `storage::git::encode_file`) or a plain text payload. Mirrors the
/// publish-time emission in `service::sync` so that SSE replay after reconnect
/// reports the same `kind` clients saw on the live channel — without this,
/// reconnecting subscribers would see every attachment as `text_ref` with the
/// JSON-pointer length as `size`, and would also fall through to refetch text
/// via `/files`.
fn classify_replay_change(path: &str, blob_bytes: &[u8]) -> EventChange {
    if let Some((hash, size)) = detect_blob_pointer(blob_bytes) {
        return EventChange::Blob {
            path: path.to_string(),
            blob_hash: hash,
            size,
        };
    }
    EventChange::TextRef {
        path: path.to_string(),
        size: blob_bytes.len() as u64,
    }
}

fn detect_blob_pointer(bytes: &[u8]) -> Option<(String, u64)> {
    let ptr = parse_blob_pointer(bytes)?;
    if !ptr.has_magic() {
        return None;
    }
    Some((ptr.blob, ptr.size))
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
    use crate::storage::git::{FileChange, Git2VaultStore, GitVaultStore, StoredFile};

    #[test]
    fn replay_events_after_uses_storage_vault_path_guard() {
        let source = include_str!("events.rs");
        let fn_start = source.find("pub async fn replay_events_after").unwrap();
        let next_fn = source[fn_start + 1..]
            .find("\nfn replay_events_after_blocking")
            .map(|idx| fn_start + 1 + idx)
            .unwrap();
        let implementation = &source[fn_start..next_fn];
        let raw_join = ["vault_root", ".join(vault_id)"].concat();
        let guarded_join = ["storage_vault", "_path(vault_root, vault_id)"].concat();

        assert!(
            !implementation.contains(&raw_join),
            "event replay should not join unvalidated vault ids directly"
        );
        assert!(
            implementation.contains(&guarded_join),
            "event replay should use the shared storage vault path guard"
        );
    }

    #[test]
    fn classify_replay_change_detects_blob_pointer() {
        let pointer = serde_json::json!({
            "pkvsync_pointer": 1,
            "blob": "a".repeat(64),
            "size": 42_u64,
            "mime": "image/png",
        });
        let bytes = serde_json::to_vec(&pointer).unwrap();
        match classify_replay_change("img.png", &bytes) {
            EventChange::Blob {
                path,
                blob_hash,
                size,
            } => {
                assert_eq!(path, "img.png");
                assert_eq!(blob_hash, "a".repeat(64));
                assert_eq!(size, 42);
            }
            other => panic!("expected Blob, got {other:?}"),
        }
    }

    #[test]
    fn classify_replay_change_treats_text_as_text_ref() {
        let bytes = b"# regular markdown content\nno pointer fields here";
        match classify_replay_change("note.md", bytes) {
            EventChange::TextRef { path, size } => {
                assert_eq!(path, "note.md");
                assert_eq!(size, bytes.len() as u64);
            }
            other => panic!("expected TextRef, got {other:?}"),
        }
    }

    #[test]
    fn parse_blob_pointer_fast_rejects_non_json_payloads() {
        let source = include_str!("../storage/git.rs");
        let fn_start = source
            .find("pub(crate) fn parse_blob_pointer")
            .expect("parse_blob_pointer implementation exists");
        let next_fn = source[fn_start + 1..]
            .find("\npub")
            .or_else(|| source[fn_start + 1..].find("\nfn "))
            .map(|idx| fn_start + 1 + idx)
            .expect("following function exists");
        let implementation = &source[fn_start..next_fn];

        assert!(implementation.contains("!bytes.starts_with(b\"{\")"));
    }

    #[test]
    fn classify_replay_change_rejects_pointer_with_bad_hash() {
        // JSON parses, claims to be a pointer, but hash is not 64 hex chars —
        // treat as text rather than emit an invalid Blob event downstream.
        let pointer = serde_json::json!({
            "pkvsync_pointer": 1,
            "blob": "not-a-hash",
            "size": 0_u64,
        });
        let bytes = serde_json::to_vec(&pointer).unwrap();
        match classify_replay_change("x", &bytes) {
            EventChange::TextRef { .. } => {}
            other => panic!("expected TextRef fallback, got {other:?}"),
        }
    }

    #[test]
    fn detect_blob_pointer_rejects_non_json_fast() {
        // Behavioral: non-JSON bytes must return None without attempting a full parse.
        assert!(detect_blob_pointer(b"not json at all").is_none());
        assert!(detect_blob_pointer(b"[1,2,3]").is_none());
        assert!(detect_blob_pointer(b"").is_none());
    }

    #[tokio::test]
    async fn replay_after_unreachable_last_event_id_reports_lagged() {
        let dir = tempfile::tempdir().unwrap();
        let store = Git2VaultStore::new(dir.path().to_path_buf());
        let first = store
            .commit_changes(
                "v1",
                None,
                &[FileChange::Upsert {
                    path: "a.md".into(),
                    file: StoredFile::Text {
                        bytes: b"one".to_vec(),
                    },
                }],
                "sync: first-device",
            )
            .await
            .unwrap();
        let second = store
            .commit_changes(
                "v1",
                Some(&first),
                &[FileChange::Upsert {
                    path: "b.md".into(),
                    file: StoredFile::Text {
                        bytes: b"two".to_vec(),
                    },
                }],
                "sync: second-device",
            )
            .await
            .unwrap();
        store
            .set_main_ref("v1", &first, "rewind head for replay test")
            .await
            .unwrap();

        let replay = replay_events_after(dir.path(), "v1", &second)
            .await
            .unwrap();

        assert!(matches!(replay, ReplayEvents::Lagged));
    }

    #[tokio::test]
    async fn subscribe_receives_published_event() {
        let bus = VaultEventBus::new(16);
        let mut rx = bus.subscribe("vault1");
        let event = VaultEvent {
            commit: "abc".into(),
            parent: None,
            source_device_id: "dev1".into(),
            at: 0,
            kind: EventKind::Commit,
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
            kind: EventKind::Commit,
            changes: vec![],
        };
        bus.publish("nonexistent", event);
    }

    #[tokio::test]
    async fn total_subscribers_sums_receivers_across_vaults() {
        let bus = VaultEventBus::new(16);
        assert_eq!(bus.total_subscribers(), 0);
        let r1 = bus.subscribe("vault-a");
        let r2 = bus.subscribe("vault-a");
        let r3 = bus.subscribe("vault-b");
        assert_eq!(bus.total_subscribers(), 3);
        drop(r1);
        assert_eq!(bus.total_subscribers(), 2);
        drop(r2);
        drop(r3);
        assert_eq!(bus.total_subscribers(), 0);
    }

    #[tokio::test]
    async fn total_subscribers_prunes_vault_senders_after_last_receiver_drops() {
        let bus = VaultEventBus::new(16);
        let rx = bus.subscribe("vault-a");
        assert_eq!(bus.len_for_tests(), 1);

        drop(rx);

        assert_eq!(bus.total_subscribers(), 0);
        assert_eq!(bus.len_for_tests(), 0);
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
            kind: EventKind::Commit,
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
                    kind: EventKind::Commit,
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
