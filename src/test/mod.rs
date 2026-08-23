mod arkitect;

use axum::{Json, Router, http::StatusCode, response::IntoResponse, routing::get};
use serde_json::json;
use sqlx::SqlitePool;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::app::env::Env;

/// Bind a random port, spawn the app, return the bound address.
pub async fn start_app() -> SocketAddr {
    start_app_with("https://api.unsplash.com").await.0
}

/// Like [`start_app`], but with an overridable Unsplash base URL; returns the
/// bound address and the database pool so tests can seed rows.
pub async fn start_app_with(unsplash_base_url: &str) -> (SocketAddr, SqlitePool) {
    serve_app(unsplash_base_url, 1, 1_000_000).await
}

/// Tight-limit harness for 429 integration tests: the global limiter runs with
/// the given budget instead of the effectively-disabled default.
pub async fn start_app_with_rate_limits(
    unsplash_base_url: &str,
    per_ms: u64,
    burst: u32,
) -> (SocketAddr, SqlitePool) {
    serve_app(unsplash_base_url, per_ms, burst).await
}

async fn serve_app(unsplash_base_url: &str, per_ms: u64, burst: u32) -> (SocketAddr, SqlitePool) {
    let env = Env {
        unsplash_api_key: "test-key".into(),
        database_url: "sqlite::memory:".into(),
        sentry_dsn: "test-dsn".into(),
        enable_sentry: false,
        rate_limit_per_ms: per_ms,
        rate_limit_burst: burst,
    };
    let db = crate::app::db::init(&env.database_url).await;
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("migrate");
    let state = crate::app::state::AppState {
        templates: crate::app::templates::init(),
        metrics: Arc::new(crate::infra::metrics::AppMetrics::new().expect("metrics")),
        http: reqwest::Client::new(),
        env: Arc::new(env),
        unsplash_base_url: unsplash_base_url.into(),
        db: db.clone(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let router: Router = crate::app::rate_limit::with_global_limit(
        crate::interfaces::routes::routes(),
        per_ms,
        burst,
    )
    .with_state(state);
    tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("server");
    });
    (addr, db)
}

/// Like `start_app`, but also serves the metrics router; returns (app_addr, metrics_addr).
pub async fn start_app_with_metrics() -> (SocketAddr, SocketAddr) {
    let env = Env {
        unsplash_api_key: "test-key".into(),
        database_url: "sqlite::memory:".into(),
        sentry_dsn: "test-dsn".into(),
        enable_sentry: false,
        rate_limit_per_ms: 1,
        rate_limit_burst: 1_000_000,
    };
    let state = crate::app::state::AppState {
        templates: crate::app::templates::init(),
        db: crate::app::db::init(&env.database_url).await,
        env: Arc::new(env),
        metrics: Arc::new(crate::infra::metrics::AppMetrics::new().expect("metrics")),
        http: reqwest::Client::new(),
        unsplash_base_url: "https://api.unsplash.com".into(),
    };
    let app_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let app_addr = app_listener.local_addr().expect("local addr");
    let metrics_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let metrics_addr = metrics_listener.local_addr().expect("local addr");
    let router: Router = crate::interfaces::routes::routes().with_state(state.clone());
    let metrics_router = crate::interfaces::routes::metrics_router(state.metrics.clone());
    tokio::spawn(async move {
        axum::serve(
            app_listener,
            router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("server");
    });
    tokio::spawn(async move {
        axum::serve(metrics_listener, metrics_router.into_make_service())
            .await
            .expect("server");
    });
    (app_addr, metrics_addr)
}

pub fn test_client() -> reqwest::Client {
    reqwest::Client::new()
}

pub struct UnsplashStub {
    pub base_url: String,
    pub call_count: Arc<AtomicUsize>,
}

/// Spawn a local stub of `GET /photos/random`. Returns canned JSON for any
/// success `status`; the status code is returned verbatim so tests can
/// simulate upstream failures (e.g. 500).
pub async fn start_unsplash_stub(status: StatusCode) -> UnsplashStub {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&call_count);
    let app = Router::new().route(
        "/photos/random",
        get(move || {
            let count = Arc::clone(&count);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                if status.is_success() {
                    Json(json!({
                        "urls": {"regular": "https://images.example.com/photo.jpg"},
                        "user": {"name": "Stub Photographer"}
                    }))
                    .into_response()
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
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
    UnsplashStub {
        base_url: format!("http://{addr}"),
        call_count,
    }
}

#[tokio::test]
async fn page_hits_show_up_in_metrics() {
    let (app_addr, metrics_addr) = start_app_with_metrics().await;
    let client = test_client();
    client
        .get(format!("http://{app_addr}/"))
        .send()
        .await
        .unwrap();
    client
        .get(format!("http://{app_addr}/"))
        .send()
        .await
        .unwrap();

    let res = client
        .get(format!("http://{metrics_addr}/metrics"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);
    let body = res.text().await.unwrap();
    assert!(body.contains("page_views_total"));
    assert!(body.contains(r#"page="home""#));
}
