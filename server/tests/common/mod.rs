//! Shared scaffolding for the `mcp_*` integration tests: a file-backed
//! AppState, a user with a bearer token, and an AuthenticatedUser handle.
//! Kept as a plain module next to the tests so each binary picks what it
//! needs with `mod common;`; not every binary uses every helper.
use pkv_sync_server::auth::{password, token, AuthenticatedUser};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
use pkv_sync_server::service::AppState;

/// File-backed state in a fresh temp dir (matches the per-test isolation the
/// mcp_* suites were written against; `connect_memory` would share one
/// in-process database across every test in the binary).
pub async fn test_state() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let p = pool::connect(&tmp.path().join("test.db")).await.unwrap();
    sqlx::migrate!("./migrations").run(&p).await.unwrap();
    let state = AppState::new(p, tmp.path().to_path_buf(), "test".into(), true)
        .await
        .unwrap();
    (state, tmp)
}

/// Create a user plus a fresh bearer token; returns (user_id, raw token).
// Used by some of the `mcp_*` binaries only (each test binary compiles this
// module independently, so unused-in-this-binary would fire dead_code).
#[allow(dead_code)]
pub async fn create_user_with_token(state: &AppState, username: &str) -> (String, String) {
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
            device_id: "mcp-test-device",
            device_name: "mcp-test",
        })
        .await
        .unwrap();
    (user.id, raw)
}

/// Resolve the AuthenticatedUser a request with this bearer token would
/// authenticate as (user + token row looked up from the database).
// Used by some of the `mcp_*` binaries only (each test binary compiles this
// module independently, so unused-in-this-binary would fire dead_code).
#[allow(dead_code)]
pub async fn auth_for(state: &AppState, raw: &str, username: &str) -> AuthenticatedUser {
    let user = state
        .users
        .find_by_username(username)
        .await
        .unwrap()
        .unwrap();
    let (token_row, _username) = state
        .tokens
        .find_by_hash(&token::hash(raw))
        .await
        .unwrap()
        .unwrap();
    AuthenticatedUser {
        user_id: user.id,
        username: user.username,
        is_admin: user.is_admin,
        token_id: token_row.id,
        device_id: token_row.device_id,
    }
}
