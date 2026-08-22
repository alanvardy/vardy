use crate::app::error::WebError;
use crate::app::picture::{self, fetch_random};
use crate::app::state::AppState;
use crate::domain::picture::Picture;
use axum::{Json, extract::State};
use chrono::{Duration, Utc};

const MAX_AGE_HOURS: i64 = 6;

pub async fn index(State(state): State<AppState>) -> Result<Json<Picture>, WebError> {
    if let Some(picture) = picture::latest(&state.db).await?
        && !is_stale(&picture)
    {
        return Ok(Json(picture));
    }
    let client = reqwest::Client::new();
    let picture = fetch_random(
        &client,
        &state.unsplash_base_url,
        &state.env.unsplash_api_key,
    )
    .await?;
    let picture = picture::create(&state.db, &picture).await?;
    Ok(Json(picture))
}

fn is_stale(picture: &Picture) -> bool {
    chrono::NaiveDateTime::parse_from_str(&picture.created_at, "%Y-%m-%d %H:%M:%S")
        .map(|created_at| Utc::now().naive_utc() - created_at > Duration::hours(MAX_AGE_HOURS))
        .unwrap_or(true) // unparseable timestamp → treat as stale, force refresh
}

#[cfg(test)]
mod tests {
    use sqlx::{Row, SqlitePool};

    use super::*;
    use crate::test::{start_app_with, start_unsplash_stub, test_client};

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
            created_at: String::new(),
        };
        let inserted = picture::create(&pool, &picture)
            .await
            .expect("insert should succeed");
        assert!(!inserted.created_at.is_empty());

        let latest = picture::latest(&pool)
            .await
            .expect("query should succeed")
            .expect("row should exist");
        assert_eq!(latest.url, "https://example.com/x.jpg");
        assert_eq!(latest.photographer, "Someone");
        assert_eq!(latest.created_at, inserted.created_at);
    }

    #[tokio::test]
    async fn unsplash_serves_seeded_row() {
        let (addr, db) = start_app_with("https://api.unsplash.com").await;
        sqlx::query("INSERT INTO unsplash_pictures (url, photographer) VALUES (?, ?)")
            .bind("https://example.com/seeded.jpg")
            .bind("Seeded Photographer")
            .execute(&db)
            .await
            .expect("seed insert should succeed");

        let client = test_client();
        let res = client
            .get(format!("http://{addr}/unsplash"))
            .send()
            .await
            .expect("request to /unsplash should succeed");
        assert_eq!(res.status(), 200);
        assert!(
            res.headers()
                .get("content-type")
                .is_some_and(|v| v.to_str().unwrap().contains("application/json"))
        );
        let body = res.text().await.expect("body");
        assert!(body.contains("https://example.com/seeded.jpg"));
        assert!(body.contains("Seeded Photographer"));
    }

    #[tokio::test]
    async fn no_row_triggers_fetch_and_insert() {
        let stub = start_unsplash_stub(axum::http::StatusCode::OK).await;
        let (addr, db) = start_app_with(&stub.base_url).await;

        let res = test_client()
            .get(format!("http://{addr}/unsplash"))
            .send()
            .await
            .expect("request to /unsplash should succeed");
        assert_eq!(res.status(), 200);
        let body = res.text().await.expect("body");
        assert!(body.contains("https://images.example.com/photo.jpg"));
        assert!(body.contains("Stub Photographer"));

        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM unsplash_pictures")
            .fetch_one(&db)
            .await
            .expect("count");
        assert_eq!(count, 1);
        assert_eq!(stub.call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn fresh_row_does_not_call_upstream() {
        let stub = start_unsplash_stub(axum::http::StatusCode::OK).await;
        let (addr, db) = start_app_with(&stub.base_url).await;
        sqlx::query(
            "INSERT INTO unsplash_pictures (url, photographer) \
             VALUES ('https://example.com/fresh.jpg', 'Fresh Photographer')",
        )
        .execute(&db)
        .await
        .expect("seed insert");

        let res = test_client()
            .get(format!("http://{addr}/unsplash"))
            .send()
            .await
            .expect("request to /unsplash should succeed");
        assert_eq!(res.status(), 200);
        let body = res.text().await.expect("body");
        assert!(body.contains("https://example.com/fresh.jpg"));
        assert!(body.contains("Fresh Photographer"));

        assert_eq!(stub.call_count.load(std::sync::atomic::Ordering::SeqCst), 0);
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM unsplash_pictures")
            .fetch_one(&db)
            .await
            .expect("count");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn stale_row_triggers_refetch() {
        let stub = start_unsplash_stub(axum::http::StatusCode::OK).await;
        let (addr, db) = start_app_with(&stub.base_url).await;
        sqlx::query(
            "INSERT INTO unsplash_pictures (url, photographer, created_at) \
             VALUES ('https://example.com/stale.jpg', 'Stale Photographer', \
             datetime('now', '-7 hours'))",
        )
        .execute(&db)
        .await
        .expect("seed insert");

        let res = test_client()
            .get(format!("http://{addr}/unsplash"))
            .send()
            .await
            .expect("request to /unsplash should succeed");
        assert_eq!(res.status(), 200);
        let body = res.text().await.expect("body");
        assert!(body.contains("https://images.example.com/photo.jpg"));
        assert!(body.contains("Stub Photographer"));

        assert_eq!(stub.call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM unsplash_pictures")
            .fetch_one(&db)
            .await
            .expect("count");
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn upstream_failure_is_502() {
        let stub = start_unsplash_stub(axum::http::StatusCode::INTERNAL_SERVER_ERROR).await;
        let (addr, _db) = start_app_with(&stub.base_url).await;

        let res = test_client()
            .get(format!("http://{addr}/unsplash"))
            .send()
            .await
            .expect("request to /unsplash should succeed");
        assert_eq!(res.status(), 502);
        let body = res.text().await.expect("body");
        assert_eq!(body, "bad gateway");
    }

    #[tokio::test]
    async fn second_request_within_window_is_cached() {
        let stub = start_unsplash_stub(axum::http::StatusCode::OK).await;
        let (addr, db) = start_app_with(&stub.base_url).await;
        let client = test_client();

        let first = client
            .get(format!("http://{addr}/unsplash"))
            .send()
            .await
            .expect("first request");
        assert_eq!(first.status(), 200);
        let first_body = first.text().await.expect("body");

        let second = client
            .get(format!("http://{addr}/unsplash"))
            .send()
            .await
            .expect("second request");
        assert_eq!(second.status(), 200);
        let second_body = second.text().await.expect("body");

        assert_eq!(first_body, second_body);
        assert_eq!(stub.call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM unsplash_pictures")
            .fetch_one(&db)
            .await
            .expect("count");
        assert_eq!(count, 1);
    }
}
