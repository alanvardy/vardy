use axum::Router;
use sqlx::SqlitePool;
use std::net::SocketAddr;

/// Bind a random port, spawn the app, return the bound address.
pub async fn start_app() -> SocketAddr {
    start_app_with("https://api.unsplash.com").await.0
}

/// Like [`start_app`], but with an overridable Unsplash base URL; returns the
/// bound address and the database pool so tests can seed rows.
pub async fn start_app_with(unsplash_base_url: &str) -> (SocketAddr, SqlitePool) {
    let db = crate::app::db::init("sqlite::memory:").await;
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("migrate");
    let state = crate::app::state::AppState {
        templates: crate::app::templates::init(),
        metrics: std::sync::Arc::new(crate::infra::metrics::AppMetrics::new().expect("metrics")),
        unsplash_api_key: "test-key".into(),
        unsplash_base_url: unsplash_base_url.into(),
        db: db.clone(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let router: Router = crate::interfaces::routes::routes().with_state(state);
    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service())
            .await
            .expect("server");
    });
    (addr, db)
}

/// Like `start_app`, but also serves the metrics router; returns (app_addr, metrics_addr).
pub async fn start_app_with_metrics() -> (SocketAddr, SocketAddr) {
    let state = crate::app::state::AppState {
        templates: crate::app::templates::init(),
        db: crate::app::db::init("sqlite::memory:").await,
        metrics: std::sync::Arc::new(crate::infra::metrics::AppMetrics::new().expect("metrics")),
        unsplash_api_key: "test-key".into(),
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
        axum::serve(app_listener, router.into_make_service())
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
