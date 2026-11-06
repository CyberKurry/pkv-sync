//! cfg(test)-only concurrency probe (MEL-07).
//!
//! Phase gates keyed by (vault, phase): a test arms a gate for a vault,
//! the production path calls `checkpoint` at the phase boundary, and the
//! test observes/holds the exact intermediate state — no sleeps. Only
//! compiled into cfg(test) builds; the release binary never links it.

#![cfg(test)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use tokio::sync::Notify;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Phase {
    SnapshotCaptured,
    #[allow(dead_code)]
    AfterReadMetadata,
    BeforeCommit,
}
type Key = (String, Phase);

#[derive(Default)]
struct Gate {
    entered: Notify,
    release: Notify,
}

static GATES: OnceLock<Mutex<HashMap<Key, Arc<Gate>>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<Key, Arc<Gate>>> {
    GATES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) struct ArmedGate {
    key: Key,
    gate: Arc<Gate>,
}

/// Register a gate for (vault, phase); the next `checkpoint` on that key
/// blocks until `release()`. Panics if one is already armed (tests must
/// not silently steal each other's probes).
pub(crate) fn arm(vault: &str, phase: Phase) -> ArmedGate {
    let key = (vault.to_owned(), phase);
    let gate = Arc::new(Gate::default());
    let inserted = registry()
        .lock()
        .unwrap()
        .insert(key.clone(), gate.clone())
        .is_none();
    assert!(inserted, "gate already armed for {key:?}");
    ArmedGate { key, gate }
}

impl ArmedGate {
    /// Resolves once the production path has entered the phase.
    pub(crate) async fn entered(&self) {
        self.gate.entered.notified().await;
    }
    /// Lets the production path continue.
    pub(crate) fn release(&self) {
        self.gate.release.notify_one();
    }
}

impl Drop for ArmedGate {
    fn drop(&mut self) {
        let mut gates = registry().lock().unwrap();
        if gates
            .get(&self.key)
            .is_some_and(|g| Arc::ptr_eq(g, &self.gate))
        {
            gates.remove(&self.key);
        }
        self.gate.release.notify_one();
    }
}

/// Production-path call site: if a gate is armed for (vault, phase),
/// signal entry and wait for release; otherwise return immediately.
pub(crate) async fn checkpoint(vault: &str, phase: Phase) {
    let gate = {
        registry()
            .lock()
            .unwrap()
            .remove(&(vault.to_owned(), phase))
    };
    if let Some(gate) = gate {
        gate.entered.notify_one();
        gate.release.notified().await;
    }
}
