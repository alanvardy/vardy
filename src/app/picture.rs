use serde::Serialize;
use sqlx::SqlitePool;

/// A picture served by the `/unsplash` endpoint, persisted in the
/// `unsplash_pictures` table.
#[derive(Serialize, sqlx::FromRow)]
pub struct Picture {
    pub url: String,
    pub photographer: String,
    pub created_at: String,
}

pub async fn latest(pool: &SqlitePool) -> sqlx::Result<Option<Picture>> {
    let picture = sqlx::query_as::<_, Picture>(
        "SELECT url, photographer, created_at FROM unsplash_pictures ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(picture)
}

pub async fn create(pool: &SqlitePool, picture: &Picture) -> sqlx::Result<Picture> {
    let inserted = sqlx::query_as::<_, Picture>(
        "INSERT INTO unsplash_pictures (url, photographer) VALUES (?, ?) \
         RETURNING url, photographer, created_at",
    )
    .bind(&picture.url)
    .bind(&picture.photographer)
    .fetch_one(pool)
    .await?;
    Ok(inserted)
}
