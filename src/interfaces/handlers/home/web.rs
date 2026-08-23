use axum::{extract::State, response::Html};
use minijinja::context;

use crate::app::error::WebError;
use crate::app::state::AppState;

pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    state.metrics.inc_page_view("home");
    let html = state
        .templates
        .get_template("home.html")?
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
        let body = res.text().await.unwrap();
        assert!(body.contains("<title>Home</title>"));
        assert!(body.contains("Hi!"));
        assert!(body.contains("My name is Alan Vardy"));
        assert!(body.contains("AI, backend Rust services and Swift applications"));
        assert!(body.contains("high-output individual contributor"));
        assert!(body.contains("You are invited to"));
        assert!(body.contains(r#"href="https://github.com/alanvardy""#));
        assert!(body.contains(r#"href="https://www.linkedin.com/in/alanvardy/""#));
        assert!(body.contains(r#"<img class="portrait" src="/static/alanvardy.jpg?v="#));
        assert!(body.contains(r#"<img class="wave" src="/static/wave.svg?v="#));
        assert!(body.contains(r#"src="/static/github.svg?v="#));
        assert!(body.contains(r#"src="/static/linkedin.svg?v="#));
        // nav chrome unchanged
        assert!(body.contains(r#"<a href="/">Home</a>"#));
        assert!(body.contains(r#"<a href="/singlethread">SingleThread</a>"#));
        assert!(body.contains("/static/site.css?v="));
        assert!(!body.contains("<style>"));
    }
}
