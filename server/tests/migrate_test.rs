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

#[tokio::test]
async fn v1_uses_single_baseline_migration() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("t.db");
    let p = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&p).await.unwrap();

    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&p)
        .await
        .unwrap();

    assert_eq!(count, 2);
}

#[tokio::test]
async fn runtime_update_check_migration_seeds_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("t.db");
    let p = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&p).await.unwrap();

    let enabled: String =
        sqlx::query_scalar("SELECT value FROM runtime_config WHERE key = 'update_check.enabled'")
            .fetch_one(&p)
            .await
            .unwrap();
    let interval: String = sqlx::query_scalar(
        "SELECT value FROM runtime_config WHERE key = 'update_check.interval_seconds'",
    )
    .fetch_one(&p)
    .await
    .unwrap();

    assert_eq!(enabled, "true");
    assert_eq!(interval, "86400");
}

#[tokio::test]
async fn sync_activity_token_fk_sets_null_on_token_delete() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("t.db");
    let p = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&p).await.unwrap();

    let rows = sqlx::query("PRAGMA foreign_key_list(sync_activity)")
        .fetch_all(&p)
        .await
        .unwrap();
    let token_fk = rows
        .into_iter()
        .find(|row| row.get::<String, _>("table") == "tokens")
        .expect("sync_activity token FK");

    assert_eq!(token_fk.get::<String, _>("on_delete"), "SET NULL");
}

#[tokio::test]
async fn idempotency_cache_primary_key_includes_vault_and_route() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("t.db");
    let p = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&p).await.unwrap();

    let rows = sqlx::query("PRAGMA table_info(idempotency_cache)")
        .fetch_all(&p)
        .await
        .unwrap();
    let pk_columns: Vec<String> = rows
        .into_iter()
        .filter(|row| row.get::<i64, _>("pk") > 0)
        .map(|row| row.get::<String, _>("name"))
        .collect();

    assert_eq!(pk_columns, ["user_id", "key", "vault_id", "route"]);
}

#[tokio::test]
async fn baseline_includes_hot_path_indexes() {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("t.db");
    let p = pool::connect(&db_path).await.unwrap();
    pool::migrate_up(&p).await.unwrap();

    for required in [
        "idx_blob_refs_vault",
        "idx_tokens_user",
        "idx_sync_activity_timestamp",
        "idx_users_is_admin",
    ] {
        let exists: Option<String> =
            sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='index' AND name = ?")
                .bind(required)
                .fetch_optional(&p)
                .await
                .unwrap();
        assert_eq!(
            exists.as_deref(),
            Some(required),
            "missing index {required}"
        );
    }
}
