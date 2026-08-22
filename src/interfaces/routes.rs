use axum::{
    Router,
    http::{HeaderValue, StatusCode, header},
    routing::get,
};
use tower_http::{services::ServeDir, set_header::SetResponseHeader};

use crate::app::state::AppState;
use crate::interfaces::handlers;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(handlers::home::web::index))
        .route("/singlethread", get(handlers::singlethread::web::index))
        .route("/unsplash", get(handlers::unsplash::json::index))
        .route(
            "/dump/{key}",
            get(handlers::dump::web::index).post(handlers::dump::web::create),
        )
        .route("/health", get(|| async { StatusCode::OK }))
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
    async fn health_returns_200() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/health"))
            .send()
            .await
            .expect("request to /health should succeed");
        assert_eq!(res.status(), StatusCode::OK);
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
