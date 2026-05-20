use pkv_sync_server::auth::{password, token, AuthenticatedUser};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
use pkv_sync_server::service::events::{EventChange, VaultEvent, VaultEventBus};
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
