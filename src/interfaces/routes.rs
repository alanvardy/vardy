use axum::{Router, http::StatusCode, routing::get};
use tower_http::services::ServeDir;

use crate::app::state::AppState;
use crate::interfaces::handlers;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(handlers::home::web::index))
        .route("/singlethread", get(handlers::singlethread::web::index))
        .route("/health", get(|| async { StatusCode::OK }))
        .nest_service("/static", ServeDir::new("static"))
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
}
