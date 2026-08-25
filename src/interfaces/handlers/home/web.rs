use axum::{extract::State, response::Html};
use minijinja::context;

use crate::app::error::WebError;
use crate::app::picture;
use crate::app::state::AppState;

pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    state.metrics.inc_page_view("home");
    // The wallpaper is decorative: render the page without it rather than
    // failing the whole request if Unsplash is unavailable.
    let (wallpaper_url, photographer, photographer_url) = picture::current(&state)
        .await
        .ok()
        .map(|p| (p.url, p.photographer, p.photographer_url))
        .unwrap_or_default();
    let html = state
        .templates
        .get_template("home.html")?
        .render(context! { wallpaper_url, photographer, photographer_url })?;
    Ok(Html(html))
}

#[cfg(test)]
mod tests {
    use crate::test::{start_app, start_app_with, start_unsplash_stub, test_client};
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
        // all images versioned
        assert!(body.contains(r#"src="/static/wave.svg?v="#));
        assert!(body.contains(r#"src="/static/alanvardy.jpg?v="#));
        assert!(body.contains(r#"src="/static/github.svg?v="#));
        assert!(body.contains(r#"src="/static/linkedin.svg?v="#));
        // no legacy component classes remain on this page
        assert!(!body.contains("home-columns"));
        assert!(!body.contains("invite-list"));
        // nav chrome unchanged
        assert!(body.contains(r#"<a href="/">Home</a>"#));
        assert!(body.contains(r#"<a href="/singlethread">SingleThread</a>"#));
        assert!(body.contains("/static/site.css?v="));
        assert!(!body.contains("<style>"));
        // credit line appears with linked photographer name
        assert!(body.contains("Photo by"));
        assert!(body.contains("Wallpaper Photographer"));
        assert!(body.contains(r#"href="https:&#x2f;&#x2f;unsplash.com&#x2f;@test""#));
        assert!(body.contains("on Unsplash"));
    }

    #[tokio::test]
    async fn index_renders_wallpaper_from_cache() {
        let addr = start_app().await;
        let body = test_client()
            .get(format!("http://{addr}/"))
            .send()
            .await
            .expect("request failed")
            .text()
            .await
            .expect("body");
        // minijinja escapes `/` in attribute context; browsers decode it back
        assert!(body.contains("url('https:&#x2f;&#x2f;example.com&#x2f;wallpaper.jpg')"));
    }

    #[tokio::test]
    async fn index_still_renders_when_wallpaper_fetch_fails() {
        let stub = start_unsplash_stub(axum::http::StatusCode::INTERNAL_SERVER_ERROR).await;
        let (addr, db) = start_app_with(&stub.base_url).await;
        sqlx::query("DELETE FROM unsplash_pictures")
            .execute(&db)
            .await
            .expect("clear pictures");

        let res = test_client()
            .get(format!("http://{addr}/"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.text().await.expect("body");
        assert!(!body.contains("background-image"));
    }
}
