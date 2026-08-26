/// Sanctioned re-export: the Unsplash fetch is implemented in `infra`
/// but `interfaces` must reach it only through `app`.
pub use crate::infra::unsplash::fetch_random;

use crate::app::error::WebError;
use crate::app::state::AppState;
use crate::domain::picture::Picture;
use sqlx::SqlitePool;

/// Extract the wallpaper URL, photographer name, and photographer URL
/// from the cached picture for template rendering. Returns empty defaults
/// on any failure so the page render never fails due to Unsplash
/// unavailability — the template guards suppress the wallpaper background
/// and credit line when these values are empty.
pub async fn wallpaper_context(state: &AppState) -> (String, String, String) {
    current(state)
        .await
        .ok()
        .map(|p| (p.url, p.photographer, p.photographer_url))
        .unwrap_or_default()
}

/// Latest picture from the database cache, refreshed from Unsplash when
/// older than the staleness window (delegates to [`Picture::is_stale`]).
pub async fn current(state: &AppState) -> Result<Picture, WebError> {
    if let Some(picture) = latest(&state.db).await?
        && !&picture.is_stale()
    {
        return Ok(picture);
    }
    fetch_and_insert(state).await
}

pub async fn latest(pool: &SqlitePool) -> sqlx::Result<Option<Picture>> {
    let picture = sqlx::query_as::<_, Picture>(
        "SELECT url, photographer, photographer_url, created_at FROM unsplash_pictures ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(picture)
}

pub async fn create(pool: &SqlitePool, picture: &Picture) -> sqlx::Result<Picture> {
    let inserted = sqlx::query_as::<_, Picture>(
        "INSERT INTO unsplash_pictures (url, photographer, photographer_url) VALUES (?, ?, ?) \
         RETURNING url, photographer, photographer_url, created_at",
    )
    .bind(&picture.url)
    .bind(&picture.photographer)
    .bind(&picture.photographer_url)
    .fetch_one(pool)
    .await?;
    Ok(inserted)
}

/// Minimum number of cached rows before `/unsplash/random` stops
/// fetching from upstream and selects locally instead.
const RANDOM_CACHE_MIN_ROWS: i64 = 5;

async fn fetch_and_insert(state: &AppState) -> Result<Picture, WebError> {
    let picture = fetch_random(
        &state.http,
        &state.unsplash_base_url,
        &state.env.unsplash_api_key,
    )
    .await?;
    Ok(create(&state.db, &picture).await?)
}

async fn count(pool: &SqlitePool) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM unsplash_pictures")
        .fetch_one(pool)
        .await
}

async fn random_select(pool: &SqlitePool) -> sqlx::Result<Picture> {
    sqlx::query_as::<_, Picture>(
        "SELECT url, photographer, photographer_url, created_at FROM unsplash_pictures ORDER BY RANDOM() LIMIT 1",
    )
    .fetch_one(pool)
    .await
}

/// Return a random cached picture, refilling from Unsplash when fewer
/// than [`RANDOM_CACHE_MIN_ROWS`] rows are available. No staleness
/// timeout — the row-count threshold alone controls refill.
pub async fn random(state: &AppState) -> Result<Picture, WebError> {
    if count(&state.db).await? < RANDOM_CACHE_MIN_ROWS {
        return fetch_and_insert(state).await;
    }
    Ok(random_select(&state.db).await?)
}

#[cfg(test)]
mod tests {
    use sqlx::{Row, SqlitePool};

    use super::*;

    #[sqlx::test]
    async fn unsplash_pictures_table_exists(pool: SqlitePool) {
        let row = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'unsplash_pictures'",
        )
        .fetch_one(&pool)
        .await
        .expect("unsplash_pictures table should exist after migrations");
        assert_eq!(row.get::<String, _>("name"), "unsplash_pictures");
    }

    #[sqlx::test]
    async fn insert_picture_returns_row_with_created_at(pool: SqlitePool) {
        let picture = Picture {
            url: "https://example.com/x.jpg".to_string(),
            photographer: "Someone".to_string(),
            photographer_url: "https://unsplash.com/@someone".to_string(),
            created_at: String::new(),
        };
        let inserted = create(&pool, &picture)
            .await
            .expect("insert should succeed");
        assert!(!inserted.created_at.is_empty());

        let latest = latest(&pool)
            .await
            .expect("query should succeed")
            .expect("row should exist");
        assert_eq!(latest.url, "https://example.com/x.jpg");
        assert_eq!(latest.photographer, "Someone");
        assert_eq!(latest.photographer_url, "https://unsplash.com/@someone");
        assert_eq!(latest.created_at, inserted.created_at);
    }

    #[sqlx::test]
    async fn count_returns_zero_on_empty(pool: SqlitePool) {
        let c = count(&pool).await.expect("count");
        assert_eq!(c, 0);
    }

    #[sqlx::test]
    async fn count_returns_seeded_row_count(pool: SqlitePool) {
        // Seed 3 rows
        for i in 0..3 {
            create(
                &pool,
                &Picture {
                    url: format!("https://example.com/{i}.jpg"),
                    photographer: format!("Photographer {i}"),
                    photographer_url: format!("https://unsplash.com/@p{i}"),
                    created_at: String::new(),
                },
            )
            .await
            .expect("insert");
        }
        let c = count(&pool).await.expect("count");
        assert_eq!(c, 3);
    }

    #[sqlx::test]
    async fn random_select_returns_a_valid_picture(pool: SqlitePool) {
        // Seed a few rows so random_select has something to pick
        for i in 0..2 {
            create(
                &pool,
                &Picture {
                    url: format!("https://example.com/{i}.jpg"),
                    photographer: format!("Photographer {i}"),
                    photographer_url: format!("https://unsplash.com/@p{i}"),
                    created_at: String::new(),
                },
            )
            .await
            .expect("insert");
        }
        let pic = random_select(&pool).await.expect("random_select");
        assert!(!pic.url.is_empty());
        assert!(!pic.photographer.is_empty());
        assert!(!pic.photographer_url.is_empty());
        assert!(!pic.created_at.is_empty());
    }

    #[tokio::test]
    async fn random_below_threshold_fetches_and_inserts() {
        use crate::app::env::Env;
        use crate::test::start_unsplash_stub;
        use std::sync::Arc;

        let stub = start_unsplash_stub(axum::http::StatusCode::OK).await;
        let pool = SqlitePool::connect("sqlite::memory:").await.expect("pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrate");
        // table is empty after migration (no seed_wallpaper)

        let state = AppState {
            db: pool.clone(),
            http: reqwest::Client::new(),
            unsplash_base_url: stub.base_url.into(),
            resend_base_url: "https://api.resend.com".into(),
            env: Arc::new(Env {
                unsplash_api_key: "test-key".into(),
                resend_api_key: "test-key".into(),
                database_url: "sqlite::memory:".into(),
                sentry_dsn: String::new(),
                enable_sentry: false,
                rate_limit_per_ms: 1,
                rate_limit_burst: 1_000_000,
            }),
            templates: crate::app::templates::init(),
            metrics: Arc::new(crate::infra::metrics::AppMetrics::new().expect("metrics")),
        };

        let picture = random(&state).await.expect("random should succeed");
        assert!(picture.url.contains("images.example.com"));
        assert_eq!(picture.photographer, "Stub Photographer");

        let c = count(&pool).await.expect("count");
        assert_eq!(c, 1);
        assert_eq!(stub.call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn random_at_threshold_selects_without_upstream() {
        use crate::app::env::Env;
        use crate::test::start_unsplash_stub;
        use std::sync::Arc;

        let stub = start_unsplash_stub(axum::http::StatusCode::OK).await;
        let pool = SqlitePool::connect("sqlite::memory:").await.expect("pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrate");

        // Seed exactly 5 rows
        for i in 0..5 {
            create(
                &pool,
                &Picture {
                    url: format!("https://example.com/{i}.jpg"),
                    photographer: format!("Photographer {i}"),
                    photographer_url: format!("https://unsplash.com/@p{i}"),
                    created_at: String::new(),
                },
            )
            .await
            .expect("insert");
        }
        let initial_count = count(&pool).await.expect("count");
        assert_eq!(initial_count, 5);

        let state = AppState {
            db: pool.clone(),
            http: reqwest::Client::new(),
            unsplash_base_url: stub.base_url.into(),
            resend_base_url: "https://api.resend.com".into(),
            env: Arc::new(Env {
                unsplash_api_key: "test-key".into(),
                resend_api_key: "test-key".into(),
                database_url: "sqlite::memory:".into(),
                sentry_dsn: String::new(),
                enable_sentry: false,
                rate_limit_per_ms: 1,
                rate_limit_burst: 1_000_000,
            }),
            templates: crate::app::templates::init(),
            metrics: Arc::new(crate::infra::metrics::AppMetrics::new().expect("metrics")),
        };

        let picture = random(&state).await.expect("random should succeed");
        assert!(!picture.url.is_empty());
        assert!(!picture.photographer.is_empty());

        // Row count unchanged
        let final_count = count(&pool).await.expect("count");
        assert_eq!(final_count, 5);
        // No upstream call
        assert_eq!(stub.call_count.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn random_upstream_failure_returns_error() {
        use crate::app::env::Env;
        use crate::test::start_unsplash_stub;
        use std::sync::Arc;

        let stub = start_unsplash_stub(axum::http::StatusCode::INTERNAL_SERVER_ERROR).await;
        let pool = SqlitePool::connect("sqlite::memory:").await.expect("pool");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrate");
        // empty table → will try upstream

        let state = AppState {
            db: pool.clone(),
            http: reqwest::Client::new(),
            unsplash_base_url: stub.base_url.into(),
            resend_base_url: "https://api.resend.com".into(),
            env: Arc::new(Env {
                unsplash_api_key: "test-key".into(),
                resend_api_key: "test-key".into(),
                database_url: "sqlite::memory:".into(),
                sentry_dsn: String::new(),
                enable_sentry: false,
                rate_limit_per_ms: 1,
                rate_limit_burst: 1_000_000,
            }),
            templates: crate::app::templates::init(),
            metrics: Arc::new(crate::infra::metrics::AppMetrics::new().expect("metrics")),
        };

        let result = random(&state).await;
        assert!(result.is_err());
        match result.err().unwrap() {
            WebError::External(_) => {} // expected
            other => panic!("expected WebError::External, got {other:?}"),
        }
    }
}
