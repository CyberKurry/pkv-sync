use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use bytes::Bytes;
use pkv_sync_server::auth::{password, token, AuthenticatedUser};
use pkv_sync_server::db::repos::{
    BlobRefRepo, NewToken, NewUser, RuntimeConfigRepo, TokenRepo, UserRepo, VaultRepo,
};
use pkv_sync_server::mcp::tools;
use pkv_sync_server::mcp::transport_http;
use pkv_sync_server::service::sync::{self, PushChange, PushReq};
use pkv_sync_server::service::AppState;
use pkv_sync_server::storage::blob::{BlobStore, LocalFsBlobStore};
use pkv_sync_server::storage::git::{FileChange, Git2VaultStore, GitVaultStore, StoredFile};
use serde_json::{json, Value};
use tower::ServiceExt;

const DEPLOYMENT_KEY: &str = "k_mcp_write_test";

async fn test_state() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let p = pkv_sync_server::db::pool::connect(&tmp.path().join("test.db"))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&p).await.unwrap();
    let state = AppState::new(p, tmp.path().to_path_buf(), "test".into(), true)
        .await
        .unwrap();
    (state, tmp)
}

async fn create_user_with_token(state: &AppState, username: &str) -> (AuthenticatedUser, String) {
    let user = state
        .users
        .create(NewUser {
            username: username.into(),
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
            device_id: "mcp-write-device",
            device_name: "mcp-write",
        })
        .await
        .unwrap();
    (
        AuthenticatedUser {
            user_id: user.id,
            username: user.username,
            is_admin: user.is_admin,
            token_id: token_row.id,
            device_id: token_row.device_id,
        },
        raw,
    )
}

async fn create_token_for_user(state: &AppState, user_id: &str, device_id: &str) -> String {
    let raw = token::generate();
    state
        .tokens
        .create(NewToken {
            user_id,
            token_hash: &token::hash(&raw),
            device_id,
            device_name: device_id,
        })
        .await
        .unwrap();
    raw
}

async fn post_tool(state: AppState, raw: &str, name: &str, arguments: Value) -> Value {
    let response = transport_http::router(state, DEPLOYMENT_KEY.into())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .header("x-pkvsync-deployment-key", DEPLOYMENT_KEY)
                .header("authorization", format!("Bearer {raw}"))
                .body(Body::from(
                    json!({
                        "jsonrpc": "2.0",
                        "id": name,
                        "method": "tools/call",
                        "params": {
                            "name": name,
                            "arguments": arguments
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn seed_text(
    state: &AppState,
    user: &AuthenticatedUser,
    vault_id: &str,
    parent: Option<&str>,
    path: &str,
    content: &str,
) -> String {
    sync::push(
        state,
        user,
        vault_id,
        parent,
        None,
        PushReq {
            device_name: Some("seed".into()),
            changes: vec![PushChange::Text {
                path: path.into(),
                content: content.into(),
            }],
        },
    )
    .await
    .unwrap()
    .new_commit
}

fn structured(body: &Value) -> &Value {
    &body["result"]["structuredContent"]
}

#[tokio::test]
async fn write_file_succeeds_when_parent_matches_head() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-write-ok").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let head = seed_text(&state, &user, &vault.id, None, "note.md", "old").await;

    let body = post_tool(
        state.clone(),
        &raw,
        "write_file",
        json!({
            "vault_id": vault.id,
            "path": "note.md",
            "content": "new",
            "parent_commit": head
        }),
    )
    .await;

    let commit = structured(&body)["commit"].as_str().unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    let file = git
        .read_file(&vault.id, "note.md", Some(commit))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        file,
        StoredFile::Text {
            bytes: b"new".to_vec()
        }
    );
}

#[tokio::test]
async fn write_file_returns_conflict_when_parent_stale() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-write-stale").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let first = seed_text(&state, &user, &vault.id, None, "note.md", "one").await;
    let current = seed_text(&state, &user, &vault.id, Some(&first), "note.md", "two").await;

    let body = post_tool(
        state.clone(),
        &raw,
        "write_file",
        json!({
            "vault_id": vault.id,
            "path": "note.md",
            "content": "stale write",
            "parent_commit": first
        }),
    )
    .await;

    assert_eq!(structured(&body)["conflict"], true);
    assert_eq!(structured(&body)["current_head"], current);
    let git = Git2VaultStore::new(state.default_vault_root());
    let file = git
        .read_file(&vault.id, "note.md", Some(&current))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        file,
        StoredFile::Text {
            bytes: b"two".to_vec()
        }
    );
}

#[tokio::test]
async fn write_file_respects_exclude_globs() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-write-exclude").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let head = seed_text(&state, &user, &vault.id, None, "note.md", "old").await;

    let body = post_tool(
        state,
        &raw,
        "write_file",
        json!({
            "vault_id": vault.id,
            "path": ".obsidian/workspace.json",
            "content": "{}",
            "parent_commit": head
        }),
    )
    .await;

    assert_eq!(body["error"]["code"], -32000);
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("path_excluded"));
}

#[tokio::test]
async fn delete_file_succeeds_and_creates_commit() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-delete-ok").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let head = seed_text(&state, &user, &vault.id, None, "note.md", "old").await;

    let body = post_tool(
        state.clone(),
        &raw,
        "delete_file",
        json!({
            "vault_id": vault.id,
            "path": "note.md",
            "parent_commit": head
        }),
    )
    .await;

    let commit = structured(&body)["commit"].as_str().unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    assert!(git
        .read_file(&vault.id, "note.md", Some(commit))
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn delete_file_returns_conflict_when_parent_stale() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-delete-stale").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let first = seed_text(&state, &user, &vault.id, None, "note.md", "one").await;
    let current = seed_text(&state, &user, &vault.id, Some(&first), "note.md", "two").await;

    let body = post_tool(
        state,
        &raw,
        "delete_file",
        json!({
            "vault_id": vault.id,
            "path": "note.md",
            "parent_commit": first
        }),
    )
    .await;

    assert_eq!(structured(&body)["conflict"], true);
    assert_eq!(structured(&body)["current_head"], current);
}

#[tokio::test]
async fn write_files_commits_all_or_nothing() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-write-files-ok").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let head = seed_text(&state, &user, &vault.id, None, "seed.md", "seed").await;

    let body = post_tool(
        state.clone(),
        &raw,
        "write_files",
        json!({
            "vault_id": vault.id,
            "parent_commit": head,
            "writes": [
                { "path": "wiki/a.md", "content": "alpha" },
                { "path": "wiki/b.md", "content": "bravo" },
                { "path": "wiki/c.md", "content": "charlie" }
            ]
        }),
    )
    .await;

    let commit = structured(&body)["commit"].as_str().unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    for (path, expected) in [
        ("wiki/a.md", "alpha"),
        ("wiki/b.md", "bravo"),
        ("wiki/c.md", "charlie"),
    ] {
        let file = git.read_file(&vault.id, path, Some(commit)).await.unwrap();
        assert_eq!(
            file,
            Some(StoredFile::Text {
                bytes: expected.as_bytes().to_vec()
            })
        );
    }
    let tree = git.list_tree(&vault.id, Some(commit)).await.unwrap();
    assert!(tree.iter().any(|entry| entry.path == "wiki/a.md"));
    assert!(tree.iter().any(|entry| entry.path == "wiki/b.md"));
    assert!(tree.iter().any(|entry| entry.path == "wiki/c.md"));
}

#[tokio::test]
async fn write_files_rejects_invalid_path_without_committing() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-write-files-invalid").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let head = seed_text(&state, &user, &vault.id, None, "seed.md", "seed").await;

    let body = post_tool(
        state.clone(),
        &raw,
        "write_files",
        json!({
            "vault_id": vault.id,
            "parent_commit": head,
            "writes": [
                { "path": "ok.md", "content": "must not commit" },
                { "path": "../x", "content": "bad" }
            ]
        }),
    )
    .await;

    assert_eq!(body["error"]["code"], -32000);
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("invalid_path"));
    let git = Git2VaultStore::new(state.default_vault_root());
    assert_eq!(
        git.head(&vault.id).await.unwrap().as_deref(),
        Some(head.as_str())
    );
    assert!(git
        .read_file(&vault.id, "ok.md", Some(&head))
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn write_files_conflict_on_stale_parent() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-write-files-stale").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let first = seed_text(&state, &user, &vault.id, None, "note.md", "one").await;
    let current = seed_text(&state, &user, &vault.id, Some(&first), "note.md", "two").await;

    let body = post_tool(
        state,
        &raw,
        "write_files",
        json!({
            "vault_id": vault.id,
            "parent_commit": first,
            "writes": [{ "path": "stale.md", "content": "nope" }]
        }),
    )
    .await;

    assert_eq!(structured(&body)["conflict"], true);
    assert_eq!(structured(&body)["current_head"], current);
}

#[tokio::test]
async fn write_files_rejects_empty_and_oversized() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-write-files-size").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let head = seed_text(&state, &user, &vault.id, None, "seed.md", "seed").await;

    let empty = post_tool(
        state.clone(),
        &raw,
        "write_files",
        json!({
            "vault_id": vault.id,
            "parent_commit": head,
            "writes": [],
            "deletes": []
        }),
    )
    .await;
    assert_eq!(empty["error"]["code"], -32000);
    assert!(empty["error"]["message"]
        .as_str()
        .unwrap()
        .contains("empty_batch"));

    let writes = (0..101)
        .map(|idx| json!({ "path": format!("bulk/{idx}.md"), "content": "x" }))
        .collect::<Vec<_>>();
    let oversized = post_tool(
        state,
        &raw,
        "write_files",
        json!({
            "vault_id": vault.id,
            "parent_commit": head,
            "writes": writes
        }),
    )
    .await;
    assert_eq!(oversized["error"]["code"], -32000);
    assert!(oversized["error"]["message"]
        .as_str()
        .unwrap()
        .contains("batch_too_large"));
}

#[tokio::test]
async fn write_files_counts_one_rate_limit_record() {
    let (state, _tmp) = test_state().await;
    state
        .mcp_write_limiter
        .update_config(1, std::time::Duration::from_secs(60));
    let (user, raw) = create_user_with_token(&state, "mcp-write-files-quota").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let head = seed_text(&state, &user, &vault.id, None, "seed.md", "seed").await;

    let batch = post_tool(
        state.clone(),
        &raw,
        "write_files",
        json!({
            "vault_id": vault.id,
            "parent_commit": head,
            "writes": [
                { "path": "batch/1.md", "content": "1" },
                { "path": "batch/2.md", "content": "2" },
                { "path": "batch/3.md", "content": "3" },
                { "path": "batch/4.md", "content": "4" },
                { "path": "batch/5.md", "content": "5" }
            ]
        }),
    )
    .await;
    let commit = structured(&batch)["commit"].as_str().unwrap();

    let second = post_tool(
        state,
        &raw,
        "write_file",
        json!({
            "vault_id": vault.id,
            "path": "after.md",
            "content": "blocked",
            "parent_commit": commit
        }),
    )
    .await;

    assert_eq!(second["error"]["code"], -32000);
    assert!(second["error"]["message"]
        .as_str()
        .unwrap()
        .contains("rate_limited"));
}

#[tokio::test]
async fn move_file_preserves_content_and_history() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-move-ok").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let head = seed_text(&state, &user, &vault.id, None, "a.md", "original").await;

    let body = post_tool(
        state.clone(),
        &raw,
        "move_file",
        json!({
            "vault_id": vault.id,
            "parent_commit": head,
            "from": "a.md",
            "to": "folder/b.md"
        }),
    )
    .await;

    let commit = structured(&body)["commit"].as_str().unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    assert!(git
        .read_file(&vault.id, "a.md", Some(commit))
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        git.read_file(&vault.id, "folder/b.md", Some(commit))
            .await
            .unwrap(),
        Some(StoredFile::Text {
            bytes: b"original".to_vec()
        })
    );

    let changes = post_tool(
        state,
        &raw,
        "changes_since",
        json!({
            "vault_id": vault.id,
            "since_commit": head
        }),
    )
    .await;
    assert!(structured(&changes)["changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|change| {
            change["status"] == "renamed"
                && change["path"] == "folder/b.md"
                && change["old_path"] == "a.md"
        }));
}

#[tokio::test]
async fn move_file_rejects_existing_target() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-move-target").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    let head = git
        .commit_changes(
            &vault.id,
            None,
            &[
                FileChange::Upsert {
                    path: "a.md".into(),
                    file: StoredFile::Text {
                        bytes: b"source".to_vec(),
                    },
                },
                FileChange::Upsert {
                    path: "b.md".into(),
                    file: StoredFile::Text {
                        bytes: b"target".to_vec(),
                    },
                },
            ],
            "seed move target",
        )
        .await
        .unwrap();

    let body = post_tool(
        state.clone(),
        &raw,
        "move_file",
        json!({
            "vault_id": vault.id,
            "parent_commit": head,
            "from": "a.md",
            "to": "b.md"
        }),
    )
    .await;

    assert_eq!(body["error"]["code"], -32000);
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("target_exists"));
    assert_eq!(
        git.head(&vault.id).await.unwrap().as_deref(),
        Some(head.as_str())
    );
}

#[tokio::test]
async fn move_file_rejects_binary() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-move-binary").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let blob = LocalFsBlobStore::new(state.default_blob_root());
    let data = Bytes::from_static(b"binary payload");
    let hash = LocalFsBlobStore::sha256(&data);
    blob.put_verified(&hash, data.clone()).await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    let head = git
        .commit_changes(
            &vault.id,
            None,
            &[FileChange::Upsert {
                path: "asset.bin".into(),
                file: StoredFile::BlobPointer {
                    hash: hash.clone(),
                    size: data.len() as u64,
                    mime: Some("application/octet-stream".into()),
                },
            }],
            "seed binary",
        )
        .await
        .unwrap();
    state
        .blob_refs
        .add_refs(&vault.id, &head, std::slice::from_ref(&hash))
        .await
        .unwrap();

    let body = post_tool(
        state,
        &raw,
        "move_file",
        json!({
            "vault_id": vault.id,
            "parent_commit": head,
            "from": "asset.bin",
            "to": "moved.bin"
        }),
    )
    .await;

    assert_eq!(body["error"]["code"], -32000);
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unsupported_binary_move"));
}

#[tokio::test]
async fn move_file_not_found() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-move-not-found").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let head = seed_text(&state, &user, &vault.id, None, "secret.md", "hidden").await;
    state
        .runtime_cfg_repo
        .set_extra_exclude_globs(vec!["secret.md".into()], None)
        .await
        .unwrap();
    state
        .runtime_cfg
        .replace(state.runtime_cfg_repo.load().await.unwrap())
        .await;

    let hidden = post_tool(
        state.clone(),
        &raw,
        "move_file",
        json!({
            "vault_id": vault.id,
            "parent_commit": head,
            "from": "secret.md",
            "to": "visible.md"
        }),
    )
    .await;
    assert_eq!(hidden["error"]["code"], -32000);
    assert!(hidden["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not_found"));

    let missing = post_tool(
        state,
        &raw,
        "move_file",
        json!({
            "vault_id": vault.id,
            "parent_commit": head,
            "from": "missing.md",
            "to": "visible.md"
        }),
    )
    .await;
    assert_eq!(missing["error"]["code"], -32000);
    assert!(missing["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not_found"));
}

#[tokio::test]
async fn write_file_records_normalized_mcp_path() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-write-normalize").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let head = seed_text(&state, &user, &vault.id, None, "seed.md", "seed").await;

    let body = post_tool(
        state.clone(),
        &raw,
        "write_file",
        json!({
            "vault_id": vault.id,
            "path": "folder\\note.md",
            "content": "normalized path",
            "parent_commit": head
        }),
    )
    .await;
    let commit = structured(&body)["commit"].as_str().unwrap();

    let git = Git2VaultStore::new(state.default_vault_root());
    let file = git
        .read_file(&vault.id, "folder/note.md", Some(commit))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        file,
        StoredFile::Text {
            bytes: b"normalized path".to_vec()
        }
    );

    let (details,): (String,) = sqlx::query_as(
        "SELECT details
         FROM sync_activity
         WHERE vault_id = ? AND action = 'mcp_write'
         ORDER BY id DESC
         LIMIT 1",
    )
    .bind(&vault.id)
    .fetch_one(&state.pool)
    .await
    .unwrap();
    assert!(details.contains("\"path\":\"folder/note.md\""), "{details}");
}

#[tokio::test]
async fn write_rate_limit_kicks_in_at_61st_request_in_window() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-rate-limit").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let mut head = seed_text(&state, &user, &vault.id, None, "note.md", "seed").await;

    for idx in 0..60 {
        let body = post_tool(
            state.clone(),
            &raw,
            "write_file",
            json!({
                "vault_id": vault.id,
                "path": "note.md",
                "content": format!("write {idx}"),
                "parent_commit": head
            }),
        )
        .await;
        head = structured(&body)["commit"].as_str().unwrap().to_string();
    }

    let body = post_tool(
        state,
        &raw,
        "write_file",
        json!({
            "vault_id": vault.id,
            "path": "note.md",
            "content": "write 61",
            "parent_commit": head
        }),
    )
    .await;

    assert_eq!(body["error"]["code"], -32000);
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("rate_limited"));
}

#[tokio::test]
async fn oversized_write_file_is_rejected_before_consuming_write_quota() {
    let (state, _tmp) = test_state().await;
    state
        .mcp_write_limiter
        .update_config(1, std::time::Duration::from_secs(60));
    state
        .runtime_cfg_repo
        .set_max_file_size(1024, None)
        .await
        .unwrap();
    state
        .runtime_cfg
        .replace(state.runtime_cfg_repo.load().await.unwrap())
        .await;
    let (user, raw) = create_user_with_token(&state, "mcp-write-size").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let head = seed_text(&state, &user, &vault.id, None, "note.md", "seed").await;

    let oversized = post_tool(
        state.clone(),
        &raw,
        "write_file",
        json!({
            "vault_id": vault.id,
            "path": "large.md",
            "content": "x".repeat(1025),
            "parent_commit": head
        }),
    )
    .await;
    assert_eq!(oversized["error"]["code"], -32000);
    assert!(oversized["error"]["message"]
        .as_str()
        .unwrap()
        .contains("max_file_size"));

    let valid = post_tool(
        state,
        &raw,
        "write_file",
        json!({
            "vault_id": vault.id,
            "path": "note.md",
            "content": "small",
            "parent_commit": head
        }),
    )
    .await;
    assert!(
        structured(&valid)["commit"].is_string(),
        "oversized failures must not consume MCP write quota: {valid}"
    );
}

#[tokio::test]
async fn different_token_or_different_vault_has_independent_quota() {
    let (state, _tmp) = test_state().await;
    state
        .mcp_write_limiter
        .update_config(1, std::time::Duration::from_secs(60));
    let (user, raw_a) = create_user_with_token(&state, "mcp-rate-scope").await;
    let raw_b = create_token_for_user(&state, &user.user_id, "mcp-write-device-b").await;
    let vault_a = state.vaults.create(&user.user_id, "a").await.unwrap();
    let vault_b = state.vaults.create(&user.user_id, "b").await.unwrap();
    let mut head_a = seed_text(&state, &user, &vault_a.id, None, "note.md", "a").await;
    let head_b = seed_text(&state, &user, &vault_b.id, None, "note.md", "b").await;

    let first = post_tool(
        state.clone(),
        &raw_a,
        "write_file",
        json!({
            "vault_id": vault_a.id,
            "path": "note.md",
            "content": "a1",
            "parent_commit": head_a
        }),
    )
    .await;
    head_a = structured(&first)["commit"].as_str().unwrap().to_string();

    let same_scope = post_tool(
        state.clone(),
        &raw_a,
        "write_file",
        json!({
            "vault_id": vault_a.id,
            "path": "note.md",
            "content": "a2",
            "parent_commit": head_a
        }),
    )
    .await;
    assert!(same_scope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("rate_limited"));

    let different_token = post_tool(
        state.clone(),
        &raw_b,
        "write_file",
        json!({
            "vault_id": vault_a.id,
            "path": "note.md",
            "content": "b token",
            "parent_commit": head_a
        }),
    )
    .await;
    assert!(structured(&different_token)["commit"].is_string());

    let different_vault = post_tool(
        state,
        &raw_a,
        "write_file",
        json!({
            "vault_id": vault_b.id,
            "path": "note.md",
            "content": "b vault",
            "parent_commit": head_b
        }),
    )
    .await;
    assert!(structured(&different_vault)["commit"].is_string());
}

#[tokio::test]
async fn move_file_checks_rate_limit_before_tree_or_content_probe() {
    let (state, _tmp) = test_state().await;
    state
        .mcp_write_limiter
        .update_config(1, std::time::Duration::from_secs(60));
    let (user, raw) = create_user_with_token(&state, "mcp-move-preflight-rate").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    let head = git
        .commit_changes(
            &vault.id,
            None,
            &[
                FileChange::Upsert {
                    path: "source.md".into(),
                    file: StoredFile::Text {
                        bytes: b"source".to_vec(),
                    },
                },
                FileChange::Upsert {
                    path: "target.md".into(),
                    file: StoredFile::Text {
                        bytes: b"target".to_vec(),
                    },
                },
            ],
            "seed move preflight rate",
        )
        .await
        .unwrap();

    let first = post_tool(
        state.clone(),
        &raw,
        "write_file",
        json!({
            "vault_id": vault.id,
            "path": "quota.md",
            "content": "consume quota",
            "parent_commit": head
        }),
    )
    .await;
    let _commit = structured(&first)["commit"].as_str().unwrap();

    let limited = post_tool(
        state,
        &raw,
        "move_file",
        json!({
            "vault_id": vault.id,
            "parent_commit": head,
            "from": "source.md",
            "to": "target.md"
        }),
    )
    .await;

    assert_eq!(limited["error"]["code"], -32000);
    let message = limited["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("rate_limited"),
        "must reject before leaking target_exists or source metadata: {message}"
    );
    assert!(!message.contains("target_exists"));
}

#[tokio::test]
async fn move_file_checks_ownership_before_tree_or_content_probe() {
    let (state, _tmp) = test_state().await;
    let (owner, _owner_raw) = create_user_with_token(&state, "mcp-move-owner").await;
    let (_other, other_raw) = create_user_with_token(&state, "mcp-move-other").await;
    let vault = state.vaults.create(&owner.user_id, "main").await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    let head = git
        .commit_changes(
            &vault.id,
            None,
            &[
                FileChange::Upsert {
                    path: "source.md".into(),
                    file: StoredFile::Text {
                        bytes: b"source".to_vec(),
                    },
                },
                FileChange::Upsert {
                    path: "target.md".into(),
                    file: StoredFile::Text {
                        bytes: b"target".to_vec(),
                    },
                },
            ],
            "seed move preflight owner",
        )
        .await
        .unwrap();

    let body = post_tool(
        state,
        &other_raw,
        "move_file",
        json!({
            "vault_id": vault.id,
            "parent_commit": head,
            "from": "source.md",
            "to": "target.md"
        }),
    )
    .await;

    assert_eq!(body["error"]["code"], -32000);
    let message = body["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("not_found") || message.contains("vault not found"),
        "must reject before leaking target_exists or source metadata: {message}"
    );
    assert!(!message.contains("target_exists"));
    assert!(!message.contains("unsupported_binary_move"));
}

#[test]
fn batch_and_move_tools_are_marked_destructive() {
    let tools = tools::tool_definitions();
    for name in ["write_files", "move_file"] {
        let tool = tools
            .iter()
            .find(|tool| tool.name == name)
            .unwrap_or_else(|| panic!("missing tool definition for {name}"));
        let annotations = tool.annotations.as_ref().unwrap();
        assert_eq!(annotations.destructive_hint, Some(true), "{name}");
    }
}

#[tokio::test]
async fn write_and_delete_record_mcp_activity_actions() {
    let (state, _tmp) = test_state().await;
    let (user, raw) = create_user_with_token(&state, "mcp-activity").await;
    let vault = state.vaults.create(&user.user_id, "main").await.unwrap();
    let head = seed_text(&state, &user, &vault.id, None, "note.md", "old").await;

    let write_body = post_tool(
        state.clone(),
        &raw,
        "write_file",
        json!({
            "vault_id": vault.id,
            "path": "note.md",
            "content": "new content",
            "parent_commit": head
        }),
    )
    .await;
    let write_commit = structured(&write_body)["commit"].as_str().unwrap();

    let delete_body = post_tool(
        state.clone(),
        &raw,
        "delete_file",
        json!({
            "vault_id": vault.id,
            "path": "note.md",
            "parent_commit": write_commit
        }),
    )
    .await;
    let delete_commit = structured(&delete_body)["commit"].as_str().unwrap();

    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT action, commit_hash, details
         FROM sync_activity
         WHERE vault_id = ? AND action LIKE 'mcp_%'
         ORDER BY id",
    )
    .bind(&vault.id)
    .fetch_all(&state.pool)
    .await
    .unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, "mcp_write");
    assert_eq!(rows[0].1, write_commit);
    assert!(rows[0].2.contains("\"path\":\"note.md\""));
    assert!(rows[0].2.contains("\"size_bytes\":11"));
    assert_eq!(rows[1].0, "mcp_delete");
    assert_eq!(rows[1].1, delete_commit);
    assert!(rows[1].2.contains("\"path\":\"note.md\""));
}
