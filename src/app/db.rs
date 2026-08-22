use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use std::str::FromStr;

pub async fn init(database_url: &str) -> SqlitePool {
    let options = SqliteConnectOptions::from_str(database_url)
        .expect("invalid DATABASE_URL")
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);

    // `create_if_missing` creates the database file but not its parent
    // directory, so a fresh checkout would fail to boot without this.
    let filename = options.get_filename();
    if filename != std::path::Path::new(":memory:")
        && let Some(parent) = filename.parent()
    {
        std::fs::create_dir_all(parent).expect("failed to create database directory");
    }

    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .expect("failed to connect to database")
}

/// Apply embedded migrations. Idempotent: `_sqlx_migrations` bookkeeping
/// makes repeat calls no-ops, so every boot converges to the current schema.
pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn init_creates_database_file_and_parent_directory() {
        let dir = std::env::temp_dir().join(format!("vardy-db-test-{}", std::process::id()));
        let url = format!("sqlite://{}/sub/db.sqlite", dir.display());
        let _pool = init(&url).await;
        assert!(dir.join("sub/db.sqlite").is_file());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[tokio::test]
    async fn migrate_creates_schema_on_empty_database() {
        let dir = std::env::temp_dir().join(format!("vardy-migrate-test-{}", std::process::id()));
        let url = format!("sqlite:{}/db.sqlite", dir.display());
        let pool = init(&url).await;
        migrate(&pool).await.expect("migration should succeed");

        // Happy path: a table from each meaningful migration exists.
        for table in ["placeholder", "dumps", "unsplash_pictures"] {
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .expect("query");
            assert_eq!(count, 1, "table {table} should exist");
        }
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[tokio::test]
    async fn migrate_is_idempotent() {
        let dir =
            std::env::temp_dir().join(format!("vardy-migrate-idem-test-{}", std::process::id()));
        let url = format!("sqlite:{}/db.sqlite", dir.display());
        let pool = init(&url).await;
        migrate(&pool).await.expect("first migrate");
        migrate(&pool).await.expect("second migrate is a no-op");
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
