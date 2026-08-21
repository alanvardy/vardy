use axum::response::Html;

pub async fn index() -> Html<String> {
    Html("Hello, world!".to_string())
}

#[cfg(test)]
mod tests {
    use crate::test::{start_app, test_client};
    use axum::http::StatusCode;

    #[tokio::test]
    async fn index_serves_ok_html() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()
                .get("content-type")
                .is_some_and(|v| v.to_str().unwrap().contains("text/html"))
        );
        assert!(res.text().await.unwrap().contains("Hello, world!"));
    }
}
