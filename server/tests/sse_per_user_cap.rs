use pkv_sync_server::db::pool;
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

#[tokio::test]
async fn one_user_acquires_per_user_limit() {
    let (state, _tmp) = test_state().await;
    state.set_sse_per_user_limit_for_tests(16);

    let guards = (0..16)
        .map(|_| state.try_acquire_sse_subscriber("user_a"))
        .collect::<Option<Vec<_>>>()
        .expect("first 16 subscribers should be accepted");

    assert!(state.try_acquire_sse_subscriber("user_a").is_none());
    drop(guards);
}

#[tokio::test]
async fn cap_is_per_user_not_global() {
    let (state, _tmp) = test_state().await;
    state.set_sse_per_user_limit_for_tests(16);

    let guards_a = (0..16)
        .map(|_| state.try_acquire_sse_subscriber("user_a"))
        .collect::<Option<Vec<_>>>()
        .expect("user A should be able to fill their own cap");

    let guards_b = (0..16)
        .map(|_| state.try_acquire_sse_subscriber("user_b"))
        .collect::<Option<Vec<_>>>()
        .expect("user B should have an independent cap");

    drop(guards_b);
    drop(guards_a);
}

#[tokio::test]
async fn releasing_subscriber_frees_per_user_slot() {
    let (state, _tmp) = test_state().await;
    state.set_sse_per_user_limit_for_tests(16);

    let mut guards = (0..16)
        .map(|_| state.try_acquire_sse_subscriber("user_a"))
        .collect::<Option<Vec<_>>>()
        .expect("first 16 subscribers should be accepted");
    assert!(state.try_acquire_sse_subscriber("user_a").is_none());

    guards.pop();

    let replacement = state.try_acquire_sse_subscriber("user_a");
    assert!(replacement.is_some());
    drop(replacement);
    drop(guards);
}
