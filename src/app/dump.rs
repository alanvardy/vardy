use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Serialize, Deserialize)]
pub struct DumpEntry {
    pub id: i64,
    pub body: serde_json::Value,
}

pub async fn list(pool: &SqlitePool, key: &str) -> sqlx::Result<Vec<DumpEntry>> {
    let entries = sqlx::query_as!(
        DumpEntry,
        r#"SELECT id, body AS "body: serde_json::Value" FROM dumps WHERE key = ? ORDER BY id"#,
        key
    )
    .fetch_all(pool)
    .await?;
    Ok(entries)
}

pub async fn create(pool: &SqlitePool, key: &str, body: &str) -> sqlx::Result<()> {
    sqlx::query!("INSERT INTO dumps (key, body) VALUES (?, ?)", key, body)
        .execute(pool)
        .await?;
    Ok(())
}
