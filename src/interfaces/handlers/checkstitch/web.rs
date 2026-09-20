use axum::{extract::State, response::Html};
use minijinja::context;

use crate::app::error::WebError;
use crate::app::picture;
use crate::app::state::AppState;

pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    state.metrics.inc_page_view("checkstitch");
    // The wallpaper and its photographer credit are decorative fallbacks:
    // render the page without them rather than failing the whole request
    // if Unsplash is unavailable.
    let (wallpaper_url, photographer, photographer_url) = picture::wallpaper_context(&state).await;
    let html = state.templates.get_template("checkstitch.html")?.render(
        context! { wallpaper_url, photographer, photographer_url, active_page => "checkstitch" },
    )?;
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
            .get(format!("http://{addr}/checkstitch"))
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
        assert!(body.contains("<title>CheckStitch</title>"));
        assert!(body.contains("<h1>CheckStitch</h1>"));
        assert!(body.contains("Your list is how you think")); // hero tagline
        assert!(body.contains("One checklist in. A list of reminders out.")); // closing CTA
        assert!(body.contains(r#"<a href="/checkstitch" class="active">CheckStitch</a>"#));
        // No App Store badge: CheckStitch has no App Store presence yet.
        assert!(!body.contains("apps.apple.com"));
        assert!(!body.contains("app-store.svg"));
        // server-rendered wallpaper from the seeded cache row; minijinja
        // escapes `/` in attribute context, browsers decode it back
        assert!(body.contains("url('https:&#x2f;&#x2f;example.com&#x2f;wallpaper.jpg')"));
        // credit line appears for the photographer
        assert!(body.contains("Photo by"));
        // responsive: wallpaper and credit are hidden on mobile breakpoints
        assert!(body.contains(r#"class="wallpaper hidden md:block""#));
        assert!(body.contains("hidden md:block"));
        assert!(body.contains("bg-black/50"));
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
            .get(format!("http://{addr}/checkstitch"))
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
            .get(format!("http://{addr}/checkstitch"))
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
