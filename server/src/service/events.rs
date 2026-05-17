use dashmap::DashMap;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;

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
