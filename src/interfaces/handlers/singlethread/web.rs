use axum::{extract::State, response::Html};
use minijinja::context;

use crate::app::error::WebError;
use crate::app::state::AppState;

pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    state.metrics.inc_page_view("singlethread");
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
        assert!(body.contains("Your brain does one thing at a time")); // hero tagline
        assert!(body.contains("One at a time.")); // first bullet lead-in
        assert!(body.contains("Why it helps"));
        assert!(body.contains("Everything you need, nothing you don't"));
        assert!(body.contains("Thoughtful by design"));
        assert!(body.contains("Built for quiet productivity"));
        assert!(body.contains("Your reminders. One at a time. In order. At your pace."));
        assert!(body.contains(r#"<img src="/static/singlethread-shot-main.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/singlethread-shot-settings.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/singlethread-shot-swipe.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/singlethread-watch-list.png?v="#));
        assert!(body.contains(r#"<img src="/static/singlethread-watch-detail.png?v="#));
        assert!(body.contains(r#"<a href="/">Home</a>"#));
        assert!(body.contains(r#"<a href="/singlethread">SingleThread</a>"#));
    }
}
