use pkv_sync_server::auth::{password, token};
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo, VaultRepo};
use pkv_sync_server::mcp::transport_stdio::StdioSession;
use pkv_sync_server::service::AppState;
use pkv_sync_server::storage::git::{FileChange, Git2VaultStore, GitVaultStore, StoredFile};
use serde_json::json;

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

async fn create_user_with_token(state: &AppState, username: &str) -> (String, String) {
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
    state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &token::hash(&raw),
            device_id: "stdio-device",
            device_name: "stdio",
        })
        .await
        .unwrap();
    (user.id, raw)
}

#[tokio::test]
async fn stdio_session_authenticates_token_and_exposes_only_requested_vault() {
    let (state, _tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "stdio-owner").await;
    let vault_a = state.vaults.create(&user_id, "alpha").await.unwrap();
    let _vault_b = state.vaults.create(&user_id, "beta").await.unwrap();

    let session = StdioSession::authenticate(state, vault_a.id.clone(), raw)
        .await
        .unwrap();
    let response = session
        .handle_jsonrpc(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "list_vaults",
                "arguments": {}
            }
        }))
        .await;

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert_eq!(
        response["result"]["structuredContent"]["vaults"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        response["result"]["structuredContent"]["vaults"][0]["id"],
        vault_a.id
    );
}

#[tokio::test]
async fn stdio_session_rejects_cross_vault_tool_arguments_without_panicking() {
    let (state, _tmp) = test_state().await;
    let (user_id, raw) = create_user_with_token(&state, "stdio-scoped").await;
    let allowed = state.vaults.create(&user_id, "allowed").await.unwrap();
    let denied = state.vaults.create(&user_id, "denied").await.unwrap();
    let git = Git2VaultStore::new(state.default_vault_root());
    git.commit_changes(
        &denied.id,
        None,
        &[FileChange::Upsert {
            path: "secret.md".into(),
            file: StoredFile::Text {
                bytes: b"secret".to_vec(),
            },
        }],
        "seed",
    )
    .await
    .unwrap();

    let session = StdioSession::authenticate(state, allowed.id, raw)
        .await
        .unwrap();
    let response = session
        .handle_jsonrpc(json!({
            "jsonrpc": "2.0",
            "id": "cross-vault",
            "method": "tools/call",
            "params": {
                "name": "list_files",
                "arguments": {
                    "vault_id": denied.id
                }
            }
        }))
        .await;

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], "cross-vault");
    assert_eq!(response["error"]["code"], -32000);
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("vault not available"));
}

#[tokio::test]
async fn stdio_session_rejects_invalid_token() {
    let (state, _tmp) = test_state().await;
    let (user_id, _raw) = create_user_with_token(&state, "stdio-invalid").await;
    let vault = state.vaults.create(&user_id, "main").await.unwrap();

    let err = StdioSession::authenticate(state, vault.id, "not-a-token".into())
        .await
        .unwrap_err();

    assert!(err.to_string().contains("invalid token"));
}
