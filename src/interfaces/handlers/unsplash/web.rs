use axum::{Json, extract::State};
use serde_json::{Value, json};

use crate::app::error::WebError;
use crate::app::state::AppState;

pub async fn index(State(_state): State<AppState>) -> Result<Json<Value>, WebError> {
    Ok(Json(json!({
        "url": "https://example.com/placeholder.jpg",
        "photographer": "placeholder",
        "created_at": "1970-01-01 00:00:00"
    })))
}

#[cfg(test)]
mod tests {
    use crate::test::{start_app, test_client};

    #[tokio::test]
    async fn unsplash_returns_json() {
        let addr = start_app().await;
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
        assert!(body.contains("\"url\""));
        assert!(body.contains("\"photographer\""));
        assert!(body.contains("\"created_at\""));
    }
}
