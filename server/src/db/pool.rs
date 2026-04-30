use crate::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{ConnectOptions, SqlitePool};
use std::path::Path;
#[cfg(test)]
use std::str::FromStr;

/// Connect to (and create if needed) the SQLite database at the given path.
///
/// Applies production-grade pragmas: WAL journal, NORMAL synchronous, foreign keys ON.
pub async fn connect(db_path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| crate::Error::Io(parent.to_path_buf(), e))?;
    }

    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .create_if_missing(true)
        .disable_statement_logging();

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts)
        .await?;

    Ok(pool)
}

/// Run all pending migrations from `server/migrations/`.
pub async fn migrate_up(pool: &SqlitePool) -> Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

/// Connect to an in-memory SQLite (for tests).
#[cfg(test)]
pub async fn connect_memory() -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
    Ok(SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connect_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let pool = connect(&db_path).await.unwrap();
        assert!(db_path.exists());
        let row: (i64,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, 1);
    }

    #[tokio::test]
    async fn connect_creates_parent_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("nested").join("test.db");
        let pool = connect(&db_path).await.unwrap();
        assert!(db_path.exists());
        let row: (i64,) = sqlx::query_as("SELECT 7").fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, 7);
    }

    #[tokio::test]
    async fn memory_pool_runs_select() {
        let pool = connect_memory().await.unwrap();
        let row: (i64,) = sqlx::query_as("SELECT 42").fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, 42);
    }
}
