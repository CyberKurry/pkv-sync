use axum::body::Body;
use axum::http::{Request, StatusCode};
use pkv_sync_server::api;
use pkv_sync_server::auth::{password, token, AuthenticatedUser};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{BlobRefRepo, NewToken, NewUser, TokenRepo, UserRepo, VaultRepo};
use pkv_sync_server::service::events::{EventKind, VaultEvent};
use pkv_sync_server::service::sync::{push, PushChange, PushReq};
use pkv_sync_server::service::vault::{self, rollback_to_commit, RollbackError};
use pkv_sync_server::service::AppState;
use pkv_sync_server::storage::git::{Git2VaultStore, GitVaultStore, StoredFile};
use std::time::Duration;
use tower::ServiceExt;

struct TestCtx {
    state: AppState,
    owner: AuthenticatedUser,
    other: AuthenticatedUser,
    admin: AuthenticatedUser,
    owner_token: String,
    other_token: String,
    vault_id: String,
    _tmp: tempfile::TempDir,
}

async fn setup() -> TestCtx {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("metadata.db");
    let p = pool::connect(&db_path).await.unwrap();
    sqlx::migrate!("./migrations").run(&p).await.unwrap();
    let state = AppState::new(p, tmp.path().to_path_buf(), "t".into(), false)
        .await
        .unwrap();
    let (owner, owner_token) = create_auth_user(&state, "owner", false).await;
    let (other, other_token) = create_auth_user(&state, "other", false).await;
    let (admin, _) = create_auth_user(&state, "admin", true).await;
    let vault = vault::create_vault(&state, &owner.user_id, "main")
        .await
        .unwrap();

    TestCtx {
        state,
        owner,
        other,
        admin,
        owner_token,
        other_token,
        vault_id: vault.id,
        _tmp: tmp,
    }
}

async fn create_auth_user(
    state: &AppState,
    username: &str,
    is_admin: bool,
) -> (AuthenticatedUser, String) {
    let user = state
        .users
        .create(NewUser {
            username: username.into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin,
        })
        .await
        .unwrap();
    let raw = token::generate();
    let token_row = state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &token::hash(&raw),
            device_id: &format!("device-{username}"),
            device_name: username,
        })
        .await
        .unwrap();
    (
        AuthenticatedUser {
            user_id: user.id,
            username: user.username,
            is_admin,
            token_id: token_row.id,
            device_id: token_row.device_id,
        },
        raw,
    )
}

async fn push_text(
    state: &AppState,
    user: &AuthenticatedUser,
    vault_id: &str,
    parent: Option<&str>,
    content: &str,
) -> String {
    push(
        state,
        user,
        vault_id,
        parent,
        None,
        PushReq {
            device_name: None,
            changes: vec![PushChange::Text {
                path: "note.md".into(),
                content: content.into(),
            }],
        },
    )
    .await
    .unwrap()
    .new_commit
}

fn restore_request(
    vault_id: &str,
    raw_token: &str,
    commit: &str,
    confirm_vault_name: &str,
) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/api/vaults/{vault_id}/restore"))
        .header("authorization", format!("Bearer {raw_token}"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "commit": commit,
                "confirm_vault_name": confirm_vault_name,
            })
            .to_string(),
        ))
        .unwrap()
}

async fn response_json(resp: axum::response::Response) -> serde_json::Value {
    serde_json::from_slice(&axum::body::to_bytes(resp.into_body(), 4096).await.unwrap()).unwrap()
}

#[tokio::test]
async fn rollback_success_moves_head_publishes_event_and_records_activity() {
    let ctx = setup().await;
    let first = push_text(&ctx.state, &ctx.owner, &ctx.vault_id, None, "v1").await;
    let second = push_text(&ctx.state, &ctx.owner, &ctx.vault_id, Some(&first), "v2").await;
    let mut rx = ctx.state.events.subscribe(&ctx.vault_id);

    let result = rollback_to_commit(&ctx.state, &ctx.owner, &ctx.vault_id, &first)
        .await
        .unwrap();

    assert_eq!(result.from_commit.as_deref(), Some(second.as_str()));
    assert_eq!(result.to_commit, first);
    assert!(result.rolled_back);
    let git = Git2VaultStore::new(ctx.state.default_vault_root());
    assert_eq!(
        git.head(&ctx.vault_id).await.unwrap().as_deref(),
        Some(first.as_str())
    );
    let file = git
        .read_file(&ctx.vault_id, "note.md", None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        file,
        StoredFile::Text {
            bytes: b"v1".to_vec()
        }
    );

    let event = rx.try_recv().unwrap();
    assert_eq!(event.commit, first);
    assert_eq!(event.source_device_id, ctx.owner.device_id);
    match event.kind {
        EventKind::Rollback {
            from_commit,
            to_commit,
        } => {
            assert_eq!(from_commit, second);
            assert_eq!(to_commit, first);
        }
        other => panic!("expected rollback event, got {other:?}"),
    }

    let (action, commit_hash, details): (String, String, String) = sqlx::query_as(
        "SELECT action, commit_hash, details
         FROM sync_activity WHERE vault_id = ? AND action = 'vault_rollback'",
    )
    .bind(&ctx.vault_id)
    .fetch_one(&ctx.state.pool)
    .await
    .unwrap();
    let details: serde_json::Value = serde_json::from_str(&details).unwrap();
    assert_eq!(action, "vault_rollback");
    assert_eq!(commit_hash, first);
    assert_eq!(details["from_commit"], second);
    assert_eq!(details["to_commit"], first);
}

#[tokio::test]
async fn rollback_refreshes_vault_stats_and_current_blob_refs() {
    let ctx = setup().await;
    let old_data = bytes::Bytes::from_static(b"old");
    let old_hash = pkv_sync_server::storage::blob::LocalFsBlobStore::sha256(&old_data);
    pkv_sync_server::service::sync::upload_blob(
        &ctx.state,
        &ctx.owner.user_id,
        &ctx.vault_id,
        &old_hash,
        old_data,
    )
    .await
    .unwrap();
    let first = push(
        &ctx.state,
        &ctx.owner,
        &ctx.vault_id,
        None,
        None,
        PushReq {
            device_name: None,
            changes: vec![PushChange::Blob {
                path: "old.png".into(),
                blob_hash: old_hash.clone(),
                size: 3,
                mime: Some("image/png".into()),
            }],
        },
    )
    .await
    .unwrap()
    .new_commit;
    let new_data = bytes::Bytes::from_static(b"new-data");
    let new_hash = pkv_sync_server::storage::blob::LocalFsBlobStore::sha256(&new_data);
    pkv_sync_server::service::sync::upload_blob(
        &ctx.state,
        &ctx.owner.user_id,
        &ctx.vault_id,
        &new_hash,
        new_data,
    )
    .await
    .unwrap();
    let _second = push(
        &ctx.state,
        &ctx.owner,
        &ctx.vault_id,
        Some(&first),
        None,
        PushReq {
            device_name: None,
            changes: vec![PushChange::Blob {
                path: "new.png".into(),
                blob_hash: new_hash.clone(),
                size: 8,
                mime: Some("image/png".into()),
            }],
        },
    )
    .await
    .unwrap()
    .new_commit;

    rollback_to_commit(&ctx.state, &ctx.owner, &ctx.vault_id, &first)
        .await
        .unwrap();

    let vault = ctx
        .state
        .vaults
        .find_by_id(&ctx.vault_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(vault.file_count, 1);
    assert_eq!(vault.size_bytes, 3);
    assert!(ctx
        .state
        .blob_refs
        .is_referenced_by_vault(&ctx.vault_id, &old_hash)
        .await
        .unwrap());
    assert!(!ctx
        .state
        .blob_refs
        .is_referenced_by_vault(&ctx.vault_id, &new_hash)
        .await
        .unwrap());
}

#[tokio::test]
async fn restore_endpoint_returns_rollback_result_for_matching_confirm_name() {
    let ctx = setup().await;
    let first = push_text(&ctx.state, &ctx.owner, &ctx.vault_id, None, "v1").await;
    let second = push_text(&ctx.state, &ctx.owner, &ctx.vault_id, Some(&first), "v2").await;
    let app = api::router().with_state(ctx.state.clone());

    let resp = app
        .oneshot(restore_request(
            &ctx.vault_id,
            &ctx.owner_token,
            &first,
            "main",
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = response_json(resp).await;
    assert_eq!(body["from_commit"], second);
    assert_eq!(body["to_commit"], first);
    assert_eq!(body["rolled_back"], true);
}

#[tokio::test]
async fn restore_endpoint_rejects_wrong_confirm_name() {
    let ctx = setup().await;
    let first = push_text(&ctx.state, &ctx.owner, &ctx.vault_id, None, "v1").await;
    let app = api::router().with_state(ctx.state.clone());

    let resp = app
        .oneshot(restore_request(
            &ctx.vault_id,
            &ctx.owner_token,
            &first,
            "wrong",
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = response_json(resp).await;
    assert_eq!(body["error"]["code"], "confirm_vault_name_mismatch");
}

#[tokio::test]
async fn restore_endpoint_rejects_unknown_commit() {
    let ctx = setup().await;
    let _first = push_text(&ctx.state, &ctx.owner, &ctx.vault_id, None, "v1").await;
    let unknown = "a".repeat(40);
    let app = api::router().with_state(ctx.state.clone());

    let resp = app
        .oneshot(restore_request(
            &ctx.vault_id,
            &ctx.owner_token,
            &unknown,
            "main",
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = response_json(resp).await;
    assert_eq!(body["error"]["code"], "unknown_commit");
}

#[tokio::test]
async fn restore_endpoint_rejects_non_owner_with_forbidden() {
    let ctx = setup().await;
    let first = push_text(&ctx.state, &ctx.owner, &ctx.vault_id, None, "v1").await;
    let _second = push_text(&ctx.state, &ctx.owner, &ctx.vault_id, Some(&first), "v2").await;
    let app = api::router().with_state(ctx.state.clone());

    let resp = app
        .oneshot(restore_request(
            &ctx.vault_id,
            &ctx.other_token,
            &first,
            "main",
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    let body = response_json(resp).await;
    assert_eq!(body["error"]["code"], "forbidden");
}

#[tokio::test]
async fn rollback_rejects_unknown_commit_distinctly() {
    let ctx = setup().await;
    let first = push_text(&ctx.state, &ctx.owner, &ctx.vault_id, None, "v1").await;
    let unknown = "a".repeat(40);

    let err = rollback_to_commit(&ctx.state, &ctx.owner, &ctx.vault_id, &unknown)
        .await
        .unwrap_err();

    assert!(matches!(err, RollbackError::UnknownCommit { .. }));
    let git = Git2VaultStore::new(ctx.state.default_vault_root());
    assert_eq!(
        git.head(&ctx.vault_id).await.unwrap().as_deref(),
        Some(first.as_str())
    );
}

#[tokio::test]
async fn rollback_to_current_head_is_noop_without_event_or_activity() {
    let ctx = setup().await;
    let first = push_text(&ctx.state, &ctx.owner, &ctx.vault_id, None, "v1").await;
    let mut rx = ctx.state.events.subscribe(&ctx.vault_id);

    let result = rollback_to_commit(&ctx.state, &ctx.owner, &ctx.vault_id, &first)
        .await
        .unwrap();

    assert_eq!(result.from_commit.as_deref(), Some(first.as_str()));
    assert_eq!(result.to_commit, first);
    assert!(!result.rolled_back);
    assert!(rx.try_recv().is_err());
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sync_activity WHERE vault_id = ? AND action = 'vault_rollback'",
    )
    .bind(&ctx.vault_id)
    .fetch_one(&ctx.state.pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn rollback_rejects_non_owner_but_allows_admin() {
    let ctx = setup().await;
    let first = push_text(&ctx.state, &ctx.owner, &ctx.vault_id, None, "v1").await;
    let second = push_text(&ctx.state, &ctx.owner, &ctx.vault_id, Some(&first), "v2").await;

    let err = rollback_to_commit(&ctx.state, &ctx.other, &ctx.vault_id, &first)
        .await
        .unwrap_err();
    assert!(matches!(err, RollbackError::Forbidden));
    let git = Git2VaultStore::new(ctx.state.default_vault_root());
    assert_eq!(
        git.head(&ctx.vault_id).await.unwrap().as_deref(),
        Some(second.as_str())
    );

    let result = rollback_to_commit(&ctx.state, &ctx.admin, &ctx.vault_id, &first)
        .await
        .unwrap();
    assert!(result.rolled_back);
    assert_eq!(
        git.head(&ctx.vault_id).await.unwrap().as_deref(),
        Some(first.as_str())
    );
}

#[tokio::test]
async fn rollback_waits_for_existing_vault_push_lock() {
    let ctx = setup().await;
    let first = push_text(&ctx.state, &ctx.owner, &ctx.vault_id, None, "v1").await;
    let second = push_text(&ctx.state, &ctx.owner, &ctx.vault_id, Some(&first), "v2").await;
    let lock = ctx.state.vault_push_lock(&ctx.vault_id);
    let guard = lock.lock().await;

    let rollback = {
        let state = ctx.state.clone();
        let user = ctx.owner.clone();
        let vault_id = ctx.vault_id.clone();
        let first = first.clone();
        tokio::spawn(async move { rollback_to_commit(&state, &user, &vault_id, &first).await })
    };

    tokio::time::timeout(Duration::from_millis(100), async {
        while !rollback.is_finished() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect_err("rollback should wait while the push lock is held");
    let git = Git2VaultStore::new(ctx.state.default_vault_root());
    assert_eq!(
        git.head(&ctx.vault_id).await.unwrap().as_deref(),
        Some(second.as_str())
    );

    drop(guard);
    let result = rollback.await.unwrap().unwrap();
    assert!(result.rolled_back);
    assert_eq!(
        git.head(&ctx.vault_id).await.unwrap().as_deref(),
        Some(first.as_str())
    );
}

#[test]
fn commit_event_serializes_with_kind_and_changes() {
    let event = VaultEvent {
        commit: "commit".into(),
        parent: None,
        source_device_id: "device".into(),
        at: 123,
        kind: EventKind::Commit,
        changes: vec![pkv_sync_server::service::events::EventChange::Delete {
            path: "note.md".into(),
        }],
    };

    let json = serde_json::to_value(event).unwrap();

    assert_eq!(json["kind"], "commit");
    assert_eq!(json["changes"][0]["kind"], "delete");
    assert_eq!(json["changes"][0]["path"], "note.md");
    assert!(json.get("from_commit").is_none());
    assert!(json.get("to_commit").is_none());
}

#[test]
fn rollback_event_serializes_with_kind_and_from_to_commits() {
    let event = VaultEvent {
        commit: "to".into(),
        parent: Some("from".into()),
        source_device_id: "device".into(),
        at: 123,
        kind: EventKind::Rollback {
            from_commit: "from".into(),
            to_commit: "to".into(),
        },
        changes: Vec::new(),
    };

    let json = serde_json::to_value(event).unwrap();

    assert_eq!(json["kind"], "rollback");
    assert_eq!(json["from_commit"], "from");
    assert_eq!(json["to_commit"], "to");
    assert!(json.get("changes").is_none());
}
