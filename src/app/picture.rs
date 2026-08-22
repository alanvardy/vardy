use crate::domain::picture::Picture;
use sqlx::SqlitePool;

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
