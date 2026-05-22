use pkv_sync_server::auth::{password, token};
use pkv_sync_server::db::pool;
use pkv_sync_server::db::repos::{NewToken, NewUser, TokenRepo, UserRepo};
use pkv_sync_server::service::AppState;

async fn test_state() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let pool = pool::connect(&tmp.path().join("metadata.db"))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let state = AppState::new(pool, tmp.path().to_path_buf(), "test".into(), false)
        .await
        .unwrap();
    (state, tmp)
}

async fn create_token(state: &AppState) -> (String, String) {
    let user = state
        .users
        .create(NewUser {
            username: "active".into(),
            password_hash: password::hash("passw0rd!!").unwrap(),
            is_admin: false,
        })
        .await
        .unwrap();
    let raw = token::generate();
    let row = state
        .tokens
        .create(NewToken {
            user_id: &user.id,
            token_hash: &token::hash(&raw),
            device_id: "dev-renewal",
            device_name: "renewal",
        })
        .await
        .unwrap();
    (row.id, raw)
}

#[tokio::test]
async fn touch_used_extends_expires_at() {
    let (state, _tmp) = test_state().await;
    let (token_id, raw) = create_token(&state).await;
    let (initial_row, _) = state
        .tokens
        .find_by_hash(&token::hash(&raw))
        .await
        .unwrap()
        .unwrap();
    let later = initial_row.expires_at - 60;

    state.tokens.touch_used(&token_id, later).await.unwrap();

    let (reloaded, _) = state
        .tokens
        .find_by_hash(&token::hash(&raw))
        .await
        .unwrap()
        .unwrap();
    let expected = later + token::TOKEN_TTL_SECONDS;
    assert_eq!(reloaded.expires_at, expected);
}

#[tokio::test]
async fn touch_used_never_shortens_expires_at() {
    let (state, _tmp) = test_state().await;
    let (token_id, raw) = create_token(&state).await;
    let (initial_row, _) = state
        .tokens
        .find_by_hash(&token::hash(&raw))
        .await
        .unwrap()
        .unwrap();

    state
        .tokens
        .touch_used(&token_id, initial_row.created_at - 60)
        .await
        .unwrap();

    let (reloaded, _) = state
        .tokens
        .find_by_hash(&token::hash(&raw))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reloaded.expires_at, initial_row.expires_at);
}
