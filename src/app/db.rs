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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    #[sqlx::test]
    async fn placeholder_table_dropped(pool: SqlitePool) {
        let rows = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'placeholder'",
        )
        .fetch_all(&pool)
        .await
        .expect("failed to query sqlite_master");
        assert!(
            rows.is_empty(),
            "placeholder table should have been dropped"
        );

        let row =
            sqlx::query("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'dumps'")
                .fetch_one(&pool)
                .await
                .expect("dumps table should exist after migrations");
        assert_eq!(row.get::<String, _>("name"), "dumps");
    }

    #[tokio::test]
    async fn init_creates_database_file_and_parent_directory() {
        let dir = std::env::temp_dir().join(format!("vardy-db-test-{}", std::process::id()));
        let url = format!("sqlite://{}/sub/db.sqlite", dir.display());
        let _pool = init(&url).await;
        assert!(dir.join("sub/db.sqlite").is_file());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
