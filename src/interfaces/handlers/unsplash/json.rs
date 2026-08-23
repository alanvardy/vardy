use crate::app::error::WebError;
use crate::app::picture;
use crate::app::state::AppState;
use crate::domain::picture::Picture;
use axum::{Json, extract::State};

pub async fn index(State(state): State<AppState>) -> Result<Json<Picture>, WebError> {
    Ok(Json(picture::current(&state).await?))
}

#[cfg(test)]
mod tests {
    use sqlx::{Row, SqlitePool};

    use super::*;
    use crate::test::{start_app_with, start_unsplash_stub, test_client};

    /// The shared test harness seeds a fresh cached picture so page renders
    /// never hit the network; these tests need specific cache states, so they
    /// clear the table first.
    async fn clear_pictures(db: &SqlitePool) {
        sqlx::query("DELETE FROM unsplash_pictures")
            .execute(db)
            .await
            .expect("clear pictures");
    }

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
        clear_pictures(&db).await;
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
        clear_pictures(&db).await;

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
        clear_pictures(&db).await;
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
        clear_pictures(&db).await;
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
        let (addr, db) = start_app_with(&stub.base_url).await;
        clear_pictures(&db).await;

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
        clear_pictures(&db).await;
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

    #[tokio::test]
    async fn unsplash_tier_trips_while_global_budget_stays_open() {
        use axum::http::StatusCode;
        use std::sync::atomic::Ordering;

        let stub = start_unsplash_stub(axum::http::StatusCode::OK).await;
        let (addr, _pool) =
            crate::test::start_app_with_rate_limits(&stub.base_url, 1, 1_000_000).await; // global effectively disabled
        clear_pictures(&_pool).await;
        let client = test_client();
        // UNSPLASH_TIER_BURST = 5; fire 20 concurrent GETs: mix of 200 and 429
        let handles: Vec<_> = (0..20)
            .map(|_| {
                let client = client.clone();
                let url = format!("http://{addr}/unsplash");
                tokio::spawn(async move { client.get(url).send().await.expect("request failed") })
            })
            .collect();
        let mut ok = 0;
        let mut limited = 0;
        for handle in handles {
            let res = handle.await.expect("join");
            match res.status() {
                StatusCode::OK => ok += 1,
                StatusCode::TOO_MANY_REQUESTS => {
                    limited += 1;
                    assert!(res.headers().get("retry-after").is_some());
                    assert_eq!(res.text().await.unwrap(), "too many requests");
                }
                status => panic!("unexpected status {status}"),
            }
        }
        assert!(ok >= 1, "at least one request should succeed");
        assert!(limited >= 5, "tier should trip well before global budget");
        // The tier throttles before every request reaches the upstream stub.
        assert!(
            stub.call_count.load(Ordering::SeqCst) < 20,
            "stub should see fewer calls than requests"
        );
    }
}
