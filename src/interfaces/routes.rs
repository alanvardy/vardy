use axum::{
    Router,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    routing::get,
};
use tower_http::{services::ServeDir, set_header::SetResponseHeader};

use crate::app::error::WebError;
use crate::app::state::AppState;
use crate::interfaces::handlers;

/// Prove data-layer liveness via the app-layer ping. Any failure flows
/// through `WebError::Database` → logged + Sentry-captured 500.
async fn health(State(state): State<AppState>) -> Result<StatusCode, WebError> {
    crate::app::db::ping(&state.db).await?;
    Ok(StatusCode::OK)
}

pub fn routes() -> Router<AppState> {
    // Expensive endpoints each get their own tighter per-IP budget, nested
    // inside the global limiter; budgets do not pool across tiers.
    let dump_tier = crate::app::rate_limit::tiered_routes(
        Router::new().route(
            "/dump/{key}",
            axum::routing::post(handlers::dump::web::create),
        ),
        crate::app::rate_limit::DUMP_TIER_PER_MS,
        crate::app::rate_limit::DUMP_TIER_BURST,
    );
    let unsplash_tier = crate::app::rate_limit::tiered_routes(
        Router::new()
            .route("/unsplash", get(handlers::unsplash::json::index))
            .route("/unsplash/random", get(handlers::unsplash::json::random)),
        crate::app::rate_limit::UNSPLASH_TIER_PER_MS,
        crate::app::rate_limit::UNSPLASH_TIER_BURST,
    );

    Router::new()
        .route("/", get(handlers::home::web::index))
        .route("/singlethread", get(handlers::singlethread::web::index))
        .route("/dump/{key}", get(handlers::dump::web::index)) // global budget only
        .merge(dump_tier)
        .merge(unsplash_tier)
        .route("/health", get(health))
        .nest_service(
            "/static",
            SetResponseHeader::overriding(
                ServeDir::new("static"),
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            ),
        )
}

/// Router for the dedicated metrics port; owns its own state.
pub fn metrics_router(metrics: std::sync::Arc<crate::app::state::AppMetrics>) -> Router {
    Router::new()
        .route("/metrics", get(handlers::metrics::web::metrics_handler))
        .with_state(metrics)
}

#[cfg(test)]
mod tests {
    use crate::test::{start_app, test_client};
    use axum::http::StatusCode;

    #[tokio::test]
    async fn static_icon_is_served() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/static/singlethread-icon.png"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()
                .get("content-type")
                .is_some_and(|v| v.to_str().unwrap().contains("image/png"))
        );
    }

    #[tokio::test]
    async fn metrics_router_serves_metrics_endpoint() {
        use crate::app::state::AppMetrics;
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use std::sync::Arc;
        use tower::ServiceExt;

        let metrics = Arc::new(AppMetrics::new().expect("test metrics"));
        let router = super::metrics_router(metrics);

        let response = router
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        assert!(content_type.contains("text/plain; version=0.0.4"));
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        // Empty registry renders as empty text.
        assert!(String::from_utf8(body.to_vec()).unwrap().is_empty());
    }

    #[tokio::test]
    async fn static_files_have_immutable_cache_control() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/static/singlethread-icon.png"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()
                .get("cache-control")
                .is_some_and(|v| v.to_str().unwrap().contains("max-age=31536000"))
        );
    }

    #[tokio::test]
    async fn under_limit_request_is_not_rate_limited() {
        let (addr, _pool) =
            crate::test::start_app_with_rate_limits("https://api.unsplash.com", 1_000, 2).await;
        let res = test_client()
            .get(format!("http://{addr}/health"))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn over_limit_requests_get_429_with_exact_body_and_retry_after() {
        let (addr, _pool) =
            crate::test::start_app_with_rate_limits("https://api.unsplash.com", 1_000, 2).await;
        let client = test_client();
        // burst 2, refill 1 token/sec: 10 rapid sequential requests must trip it
        let mut saw_429 = false;
        for _ in 0..10 {
            let res = client
                .get(format!("http://{addr}/health"))
                .send()
                .await
                .unwrap();
            match res.status() {
                StatusCode::TOO_MANY_REQUESTS => {
                    saw_429 = true;
                    assert!(res.headers().get("retry-after").is_some());
                    assert_eq!(res.text().await.unwrap(), "too many requests");
                }
                StatusCode::OK => {}
                status => panic!("unexpected status {status}"),
            }
        }
        assert!(
            saw_429,
            "expected at least one 429 within 10 rapid requests"
        );
    }
    #[tokio::test]
    async fn health_returns_200() {
        let (addr, _pool) = crate::test::start_app_with("https://api.unsplash.com").await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/health"))
            .send()
            .await
            .expect("request to /health should succeed");
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.text().await.unwrap(), ""); // bare StatusCode body
    }

    #[tokio::test]
    async fn health_returns_500_when_database_is_dead() {
        let (addr, pool) = crate::test::start_app_with("https://api.unsplash.com").await;
        pool.close().await; // kill the data layer behind the running server
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/health"))
            .send()
            .await
            .expect("request to /health should complete");
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(res.text().await.unwrap(), "internal server error");
    }

    #[tokio::test]
    async fn singlethread_screenshots_are_served_with_immutable_caching() {
        let addr = start_app().await;
        let client = test_client();
        let cases = [
            ("/static/singlethread-shot-main.jpg", "image/jpeg"),
            ("/static/singlethread-shot-settings.jpg", "image/jpeg"),
            ("/static/singlethread-shot-swipe.jpg", "image/jpeg"),
            ("/static/singlethread-watch-list.png", "image/png"),
            ("/static/singlethread-watch-detail.png", "image/png"),
        ];
        for (path, content_type) in cases {
            let res = client
                .get(format!("http://{addr}{path}"))
                .send()
                .await
                .unwrap_or_else(|_| panic!("request failed for {path}"));
            assert_eq!(res.status(), StatusCode::OK, "{path}");
            assert!(
                res.headers()
                    .get("content-type")
                    .is_some_and(|v| v.to_str().unwrap().contains(content_type)),
                "{path}"
            );
            assert!(
                res.headers()
                    .get("cache-control")
                    .is_some_and(|v| v.to_str().unwrap().contains("max-age=31536000")),
                "{path}"
            );
        }
    }

    #[tokio::test]
    async fn static_homepage_image_is_served() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/static/alanvardy.jpg"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()
                .get("content-type")
                .is_some_and(|v| v.to_str().unwrap().contains("image/jpeg"))
        );
    }

    #[tokio::test]
    async fn static_stylesheet_is_served() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/static/site.css"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()
                .get("content-type")
                .is_some_and(|v| v.to_str().unwrap().contains("text/css"))
        );
        assert!(
            res.headers()
                .get("cache-control")
                .is_some_and(|v| v.to_str().unwrap().contains("max-age=31536000"))
        );
    }
}
