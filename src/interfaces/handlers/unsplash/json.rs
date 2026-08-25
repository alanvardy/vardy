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
    use sqlx::SqlitePool;

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
        assert!(body.contains("https://unsplash.com/@stub"));

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
        // Fresh rows come from a legacy INSERT with no photographer_url.
        assert!(body.contains(r#""photographer_url":"""#));

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
        // The stale row refetches from the stub, which seeds photographer_url.
        assert!(body.contains("https://unsplash.com/@stub"));

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
    async fn malformed_upstream_json_missing_user_links_is_502() {
        use axum::response::IntoResponse;
        use axum::{Json, Router, routing::get};
        use serde_json::json;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Stub returns JSON without `user.links`, which fails the strict parse.
        let call_count = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&call_count);
        let app = Router::new().route(
            "/photos/random",
            get(move || {
                let count = Arc::clone(&count);
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Json(json!({
                        "urls": {"regular": "https://images.example.com/photo.jpg"},
                        "user": {"name": "Stub Photographer"}
                    }))
                    .into_response()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            axum::serve(listener, app.into_make_service())
                .await
                .expect("server");
        });
        let base_url = format!("http://{addr}");

        let (app_addr, db) = start_app_with(&base_url).await;
        clear_pictures(&db).await;

        let res = test_client()
            .get(format!("http://{app_addr}/unsplash"))
            .send()
            .await
            .expect("request");
        assert_eq!(res.status(), 502);
        let body = res.text().await.expect("body");
        assert_eq!(body, "bad gateway");
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
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
        assert!(first_body.contains("https://unsplash.com/@stub"));

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
