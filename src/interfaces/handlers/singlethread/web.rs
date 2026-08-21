use axum::{extract::State, response::Html};
use minijinja::context;

use crate::app::error::WebError;
use crate::app::state::AppState;

pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    let html = state
        .templates
        .get_template("singlethread.html")?
        .render(context! {})?;
    Ok(Html(html))
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
            .get(format!("http://{addr}/singlethread"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()
                .get("content-type")
                .is_some_and(|v| v.to_str().unwrap().contains("text/html"))
        );
        let body = res.text().await.unwrap();
        assert!(body.contains("<title>SingleThread</title>"));
        assert!(body.contains("<h1>SingleThread</h1>"));
        assert!(body.contains("single line of work"));
        assert!(body.contains(r#"<img src="/static/singlethread-icon.png""#));
    }
}
