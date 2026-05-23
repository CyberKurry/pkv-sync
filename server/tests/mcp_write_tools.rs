use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use pkv_sync_server::auth::{password, token, AuthenticatedUser};
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo, VaultRepo};
use pkv_sync_server::mcp::transport_http;
use pkv_sync_server::service::sync::{self, PushChange, PushReq};
use pkv_sync_server::service::AppState;
use pkv_sync_server::storage::git::{Git2VaultStore, GitVaultStore, StoredFile};
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
