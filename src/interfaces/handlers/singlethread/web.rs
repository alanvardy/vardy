use axum::{extract::State, response::Html};
use minijinja::context;

use crate::app::error::WebError;
use crate::app::picture;
use crate::app::state::AppState;

pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    state.metrics.inc_page_view("singlethread");
    // The wallpaper and its photographer credit are decorative fallbacks:
    // render the page without them rather than failing the whole request
    // if Unsplash is unavailable.
    let (wallpaper_url, photographer, photographer_url) = picture::wallpaper_context(&state).await;
    let html = state
        .templates
        .get_template("singlethread.html")?
        .render(context! { wallpaper_url, photographer, photographer_url })?;
    Ok(Html(html))
}

#[cfg(test)]
mod tests {
    use crate::test::{
        seed_wallpaper_no_url, start_app, start_app_with, start_unsplash_stub, test_client,
    };
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
        // no legacy component classes remain on this page (checked at class-name
        // boundaries so utility substrings like "list-disc" don't false-positive)
        assert!(!body.contains("\"st-"));
        assert!(!body.contains(" st-"));
        assert!(!body.contains("section-heading"));
        // server-rendered wallpaper from the seeded cache row; minijinja
        // escapes `/` in attribute context, browsers decode it back
        assert!(body.contains("url('https:&#x2f;&#x2f;example.com&#x2f;wallpaper.jpg')"));
        // credit line appears with linked photographer name
        assert!(body.contains("Photo by"));
        assert!(body.contains("Wallpaper Photographer"));
        assert!(body.contains(r#"href="https:&#x2f;&#x2f;unsplash.com&#x2f;@test""#));
        assert!(body.contains("on Unsplash"));
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
            .get(format!("http://{addr}/singlethread"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.text().await.expect("body");
        assert!(!body.contains("background-image"));
        assert!(!body.contains("Photo by"));
    }

    #[tokio::test]
    async fn index_shows_credit_as_text_when_no_photographer_url() {
        let (addr, db) = start_app_with("https://api.unsplash.com").await;
        sqlx::query("DELETE FROM unsplash_pictures")
            .execute(&db)
            .await
            .expect("clear pictures");
        seed_wallpaper_no_url(&db).await;
        let body = test_client()
            .get(format!("http://{addr}/singlethread"))
            .send()
            .await
            .expect("request failed")
            .text()
            .await
            .expect("body");
        assert!(body.contains("Photo by NoLink Photographer on Unsplash"));
        // The name must NOT be wrapped in a link when photographer_url is empty
        assert!(!body.contains("NoLink Photographer</a>"));
    }
}
