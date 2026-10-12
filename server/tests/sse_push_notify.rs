use pkv_sync_server::auth::{password, token, AuthenticatedUser};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
use pkv_sync_server::service::events::{
    replay_events_after, EventChange, EventKind, ReplayEvents, VaultEvent, VaultEventBus,
};
use pkv_sync_server::service::sync::{push, PushChange, PushReq};
use pkv_sync_server::service::vault;
use pkv_sync_server::service::AppState;
use pkv_sync_server::storage::blob::LocalFsBlobStore;
use pkv_sync_server::storage::git::{Git2VaultStore, GitVaultStore};
use tokio::sync::broadcast;

async fn setup() -> (AppState, AuthenticatedUser, String, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("metadata.db");
    let p = pool::connect(&db_path).await.unwrap();
    sqlx::migrate!("./migrations").run(&p).await.unwrap();
    let state = AppState::new(p, tmp.path().to_path_buf(), "t".into(), false)
        .await
        .unwrap();
    let user = state
        .users
        .create(NewUser {
            username: "u".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: false,
        })
        .await
        .unwrap();
    let raw = token::generate();
    let token_row = state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &token::hash(&raw),
            device_id: "device-sse-test",
            device_name: "sse-test",
        })
        .await
        .unwrap();
    let vault = vault::create_vault(&state, &user.id, "main").await.unwrap();
    let device_id = token_row.device_id.clone();
    let auth = AuthenticatedUser {
        user_id: user.id,
        username: user.username,
        is_admin: false,
        token_id: token_row.id,
        device_id,
    };
    (state, auth, vault.id, tmp)
}

#[tokio::test]
async fn push_small_text_emits_text_inline() {
    let (state, user, vid, _tmp) = setup().await;
    let mut rx = state.events.subscribe(&vid);

    push(
        &state,
        &user,
        &vid,
        None,
        None,
        PushReq {
            device_name: None,
            changes: vec![PushChange::Text {
                path: "note.md".into(),
                content: "hello".into(),
            }],
        },
    )
    .await
    .unwrap();

    let event = rx.try_recv().unwrap();
    assert!(event.parent.is_none());
    assert_eq!(event.changes.len(), 1);
    match &event.changes[0] {
        EventChange::TextInline { path, content } => {
            assert_eq!(path, "note.md");
            assert_eq!(content, "hello");
        }
        other => panic!("expected TextInline, got {:?}", other),
    }
}

#[tokio::test]
async fn push_large_text_emits_text_ref() {
    let (state, user, vid, _tmp) = setup().await;
    let mut rx = state.events.subscribe(&vid);

    let large_content = "x".repeat(8193);
    push(
        &state,
        &user,
        &vid,
        None,
        None,
        PushReq {
            device_name: None,
            changes: vec![PushChange::Text {
                path: "big.md".into(),
                content: large_content.clone(),
            }],
        },
    )
    .await
    .unwrap();

    let event = rx.try_recv().unwrap();
    assert_eq!(event.changes.len(), 1);
    match &event.changes[0] {
        EventChange::TextRef { path, size } => {
            assert_eq!(path, "big.md");
            assert_eq!(*size, large_content.len() as u64);
        }
        other => panic!("expected TextRef, got {:?}", other),
    }
}

#[tokio::test]
async fn push_caps_total_inline_text_in_single_event() {
    let (state, user, vid, _tmp) = setup().await;
    let mut rx = state.events.subscribe(&vid);
    let content = "x".repeat(8192);

    push(
        &state,
        &user,
        &vid,
        None,
        None,
        PushReq {
            device_name: None,
            changes: (0..9)
                .map(|idx| PushChange::Text {
                    path: format!("note-{idx}.md"),
                    content: content.clone(),
                })
                .collect(),
        },
    )
    .await
    .unwrap();

    let event = rx.try_recv().unwrap();
    let inline_bytes: usize = event
        .changes
        .iter()
        .map(|change| match change {
            EventChange::TextInline { content, .. } => content.len(),
            _ => 0,
        })
        .sum();
    let ref_count = event
        .changes
        .iter()
        .filter(|change| matches!(change, EventChange::TextRef { .. }))
        .count();

    assert_eq!(event.changes.len(), 9);
    assert_eq!(inline_bytes, 64 * 1024);
    assert_eq!(ref_count, 1);
}

/// Regression M-8 (ultrareview 2026-10-11 §2.3): the push Commit event used
/// to be broadcast BEFORE `record_push_metadata` inserted the push's
/// `blob_refs` rows. Pulls read the git tree directly, so the moment a
/// subscriber saw the event, the new `BlobPointer` was already visible —
/// yet `download_blob` (which gates on `blob_refs.is_referenced_by_vault`)
/// still 404'd until the later metadata transaction committed. This pins
/// the causal order: when the Commit event arrives, the blob must already
/// be authorized for download, i.e. its `blob_refs` row must exist.
///
/// Guard-quality note (ultrareview-r2 T-4): this probe RACES the fix — the
/// probe task below only runs after `push` yields the runtime, so its
/// SELECT competes with the metadata transaction and can observe the
/// committed row even under the old publish-first ordering (measured: it
/// passes ~1/5 runs on the reverted code). It is kept because it pins the
/// client-visible behavior; the DETERMINISTIC ordering guard is
/// `push::tests::push_metadata_commits_before_commit_event_is_broadcast`
/// (server/src/service/sync/push.rs), which reads a cfg(test) probe log
/// recorded at the two seam points and fails 5/5 on the reverted order.
#[tokio::test]
async fn blob_refs_are_durable_before_commit_event_is_broadcast() {
    let (state, user, vid, _tmp) = setup().await;
    let mut rx = state.events.subscribe(&vid);

    let data = bytes::Bytes::from_static(b"hello");
    let hash = LocalFsBlobStore::sha256(&data);
    pkv_sync_server::service::sync::upload_blob(&state, &user.user_id, &vid, &hash, data)
        .await
        .unwrap();

    // The subscriber's first receive fires while `push` is still between
    // `events.publish` and `record_push_metadata` — the event is already
    // in the broadcast channel by then. Whatever the push path does after
    // the broadcast, the blob_refs row must already be visible in the same
    // database the download endpoint authorizes against.
    let probe_state = state.clone();
    let probe_vault = vid.clone();
    let probe_hash = hash.clone();
    let seen = tokio::spawn(async move {
        let event = rx.recv().await.expect("commit event");
        assert!(matches!(event.kind, EventKind::Commit));
        assert_eq!(event.commit.len(), 40);
        // Exactly the gate `download_blob` performs; with the old
        // publish-before-record ordering this deterministically counts 0.
        let referenced: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM blob_refs WHERE vault_id = ? AND blob_hash = ?")
                .bind(&probe_vault)
                .bind(&probe_hash)
                .fetch_one(&probe_state.pool)
                .await
                .unwrap();
        referenced.0
    });

    push(
        &state,
        &user,
        &vid,
        None,
        None,
        PushReq {
            device_name: None,
            changes: vec![PushChange::Blob {
                path: "img.png".into(),
                blob_hash: hash,
                size: 5,
                mime: None,
            }],
        },
    )
    .await
    .unwrap();

    let refs_at_event_time = seen.await.unwrap();
    assert_eq!(
        refs_at_event_time, 1,
        "blob_refs must be committed before the Commit event is broadcast; \
         download_blob would 404 for the freshly pushed blob otherwise"
    );
}

/// Regression M-8 (ultrareview 2026-10-11 §2.3): same ordering guarantee,
/// asserted through the public download path rather than the raw table.
/// When a client has received the push Commit event, the blob download
/// endpoint must immediately succeed — with the old ordering this 404s
/// deterministically because the broadcast completes before
/// `record_push_metadata` even starts.
///
/// Guard-quality note (ultrareview-r2 T-4): like its sibling above, this
/// probe races the fix (probe task runs only after `push` yields) and can
/// pass on the reverted publish-first order. It pins client-visible
/// behavior; the deterministic ordering guard lives in
/// `push::tests::push_metadata_commits_before_commit_event_is_broadcast`
/// (server/src/service/sync/push.rs).
#[tokio::test]
async fn blob_download_succeeds_as_soon_as_commit_event_is_received() {
    let (state, user, vid, _tmp) = setup().await;
    let mut rx = state.events.subscribe(&vid);

    let data = bytes::Bytes::from_static(b"hello");
    let hash = LocalFsBlobStore::sha256(&data);
    pkv_sync_server::service::sync::upload_blob(&state, &user.user_id, &vid, &hash, data)
        .await
        .unwrap();

    let probe_state = state.clone();
    let probe_user = user.clone();
    let probe_vault = vid.clone();
    let probe_hash = hash.clone();
    let seen = tokio::spawn(async move {
        let event = rx.recv().await.expect("commit event");
        assert!(matches!(event.kind, EventKind::Commit));
        match &event.changes[0] {
            EventChange::Blob { blob_hash, .. } => assert_eq!(blob_hash, &probe_hash),
            other => panic!("expected Blob change, got {:?}", other),
        }
        pkv_sync_server::service::sync::download_blob(
            &probe_state,
            &probe_user.user_id,
            &probe_vault,
            &probe_hash,
        )
        .await
    });

    push(
        &state,
        &user,
        &vid,
        None,
        None,
        PushReq {
            device_name: None,
            changes: vec![PushChange::Blob {
                path: "img.png".into(),
                blob_hash: hash,
                size: 5,
                mime: None,
            }],
        },
    )
    .await
    .unwrap();

    let downloaded = seen.await.unwrap().unwrap();
    assert_eq!(downloaded, Some(bytes::Bytes::from_static(b"hello")));
}

#[tokio::test]
async fn push_blob_emits_blob_event() {
    let (state, user, vid, _tmp) = setup().await;
    let mut rx = state.events.subscribe(&vid);

    let data = bytes::Bytes::from_static(b"hello");
    let hash = LocalFsBlobStore::sha256(&data);
    pkv_sync_server::service::sync::upload_blob(&state, &user.user_id, &vid, &hash, data)
        .await
        .unwrap();

    push(
        &state,
        &user,
        &vid,
        None,
        None,
        PushReq {
            device_name: None,
            changes: vec![PushChange::Blob {
                path: "img.png".into(),
                blob_hash: hash.clone(),
                size: 5,
                mime: None,
            }],
        },
    )
    .await
    .unwrap();

    let event = rx.try_recv().unwrap();
    assert_eq!(event.changes.len(), 1);
    match &event.changes[0] {
        EventChange::Blob {
            path,
            blob_hash,
            size,
        } => {
            assert_eq!(path, "img.png");
            assert_eq!(blob_hash, &hash);
            assert_eq!(*size, 5);
        }
        other => panic!("expected Blob, got {:?}", other),
    }
}

/// Regression: source_device_id in the published
/// VaultEvent must be the token's device_id (a stable per-device identifier
/// like "device-sse-test"), not the token row's database id. Using the row id
/// breaks SSE self-echo filtering because the client compares against its own
/// device_id, never the token row id, so it would receive its own push back.
#[tokio::test]
async fn push_event_carries_token_device_id_not_token_row_id() {
    let (state, user, vid, _tmp) = setup().await;
    let mut rx = state.events.subscribe(&vid);

    // Sanity: setup() uses device_id="device-sse-test" when creating the token.
    assert_eq!(user.device_id, "device-sse-test");
    assert_ne!(
        user.device_id, user.token_id,
        "test setup should distinguish device_id from token_id so the assertion is meaningful"
    );

    push(
        &state,
        &user,
        &vid,
        None,
        None,
        PushReq {
            device_name: None,
            changes: vec![PushChange::Text {
                path: "note.md".into(),
                content: "hi".into(),
            }],
        },
    )
    .await
    .unwrap();

    let event = rx.try_recv().unwrap();
    assert_eq!(
        event.source_device_id, "device-sse-test",
        "event must carry the device_id so client-side echo filtering works"
    );
    assert_ne!(
        event.source_device_id, user.token_id,
        "event must NOT carry the token row id; that was the v0.3.0 echo bug"
    );
}

#[tokio::test]
async fn live_and_replayed_sse_events_use_same_commit_timestamp() {
    let (state, user, vid, _tmp) = setup().await;
    let first = push(
        &state,
        &user,
        &vid,
        None,
        None,
        PushReq {
            device_name: Some("first".into()),
            changes: vec![PushChange::Text {
                path: "first.md".into(),
                content: "one".into(),
            }],
        },
    )
    .await
    .unwrap();
    let mut rx = state.events.subscribe(&vid);

    let second = push(
        &state,
        &user,
        &vid,
        Some(&first.new_commit),
        None,
        PushReq {
            device_name: Some("second".into()),
            changes: vec![PushChange::Text {
                path: "second.md".into(),
                content: "two".into(),
            }],
        },
    )
    .await
    .unwrap();

    let live_event = rx.try_recv().unwrap();
    assert_eq!(live_event.commit, second.new_commit);
    let replayed = replay_events_after(state.vault_root(), &vid, &first.new_commit)
        .await
        .unwrap();
    let ReplayEvents::Events(replayed_events) = replayed else {
        panic!("expected replay events");
    };
    let replay_event = replayed_events
        .into_iter()
        .find(|event| event.commit == second.new_commit)
        .expect("second commit should replay after first commit");

    assert_eq!(live_event.at, replay_event.at);
}

#[tokio::test]
async fn push_delete_emits_delete_event() {
    let (state, user, vid, _tmp) = setup().await;

    push(
        &state,
        &user,
        &vid,
        None,
        None,
        PushReq {
            device_name: None,
            changes: vec![PushChange::Text {
                path: "note.md".into(),
                content: "hello".into(),
            }],
        },
    )
    .await
    .unwrap();

    let mut rx = state.events.subscribe(&vid);

    let head = {
        let git = Git2VaultStore::new(state.default_vault_root());
        git.head(&vid).await.unwrap().unwrap()
    };

    push(
        &state,
        &user,
        &vid,
        Some(&head),
        None,
        PushReq {
            device_name: None,
            changes: vec![PushChange::Delete {
                path: "note.md".into(),
            }],
        },
    )
    .await
    .unwrap();

    let event = rx.try_recv().unwrap();
    assert_eq!(event.changes.len(), 1);
    match &event.changes[0] {
        EventChange::Delete { path } => {
            assert_eq!(path, "note.md");
        }
        other => panic!("expected Delete, got {:?}", other),
    }
}

#[tokio::test]
async fn two_receivers_both_get_event() {
    let (state, user, vid, _tmp) = setup().await;
    let mut rx1 = state.events.subscribe(&vid);
    let mut rx2 = state.events.subscribe(&vid);

    push(
        &state,
        &user,
        &vid,
        None,
        None,
        PushReq {
            device_name: None,
            changes: vec![PushChange::Text {
                path: "note.md".into(),
                content: "hello".into(),
            }],
        },
    )
    .await
    .unwrap();

    let e1 = rx1.try_recv().unwrap();
    let e2 = rx2.try_recv().unwrap();
    assert_eq!(e1.commit, e2.commit);
}

#[tokio::test]
async fn publish_without_receiver_does_not_panic() {
    let bus = VaultEventBus::new(64);
    bus.publish(
        "nonexistent",
        VaultEvent {
            commit: "abc".into(),
            parent: None,
            source_device_id: "dev1".into(),
            at: 0,
            kind: EventKind::Commit,
            changes: vec![],
        },
    );
}

#[tokio::test]
async fn capacity_overflow_yields_lagged() {
    let bus = VaultEventBus::new(4);
    let mut rx = bus.subscribe("vault1");
    for i in 0..100u64 {
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
    assert!(
        matches!(result, Err(broadcast::error::TryRecvError::Lagged(n)) if n > 0),
        "expected Lagged, got {:?}",
        result
    );
}
