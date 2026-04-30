use pkv_sync_server::db::pool;
use sqlx::Row;

#[tokio::test]
async fn migrate_up_creates_all_tables() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("t.db");
    let p = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&p).await.unwrap();

    let rows: Vec<String> =
        sqlx::query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .fetch_all(&p)
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.get::<String, _>(0))
            .collect();

    for required in [
        "admin_sessions",
        "blob_refs",
        "idempotency_cache",
        "invites",
        "runtime_config",
        "sync_activity",
        "tokens",
        "users",
        "vaults",
    ] {
        assert!(
            rows.iter().any(|t| t == required),
            "missing table {required}; got {rows:?}"
        );
    }
}

#[tokio::test]
async fn migrate_up_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("t.db");
    let p = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&p).await.unwrap();
    pool::migrate_up(&p).await.unwrap();
}
