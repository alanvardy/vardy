/// Sanctioned re-export: the Unsplash fetch is implemented in `infra`
/// but `interfaces` must reach it only through `app`.
pub use crate::infra::unsplash::fetch_random;

use crate::app::error::WebError;
use crate::app::state::AppState;
use crate::domain::picture::Picture;
use sqlx::SqlitePool;

/// Latest picture from the database cache, refreshed from Unsplash when
/// older than [`MAX_AGE_HOURS`] hours.
pub async fn current(state: &AppState) -> Result<Picture, WebError> {
    if let Some(picture) = latest(&state.db).await?
        && !&picture.is_stale()
    {
        return Ok(picture);
    }
    let picture = fetch_random(
        &state.http,
        &state.unsplash_base_url,
        &state.env.unsplash_api_key,
    )
    .await?;
    Ok(create(&state.db, &picture).await?)
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
