use axum::{extract::State, response::Html};
use minijinja::context;

use crate::app::error::WebError;
use crate::app::picture;
use crate::app::state::AppState;

/// Shared render helper so GET (form) and POST (thank-you) render the same
/// template with a `submitted` flag selecting the branch.
async fn render(state: &AppState, submitted: bool) -> Result<Html<String>, WebError> {
    // The wallpaper and its photographer credit are decorative fallbacks:
    // render the page without them rather than failing the whole request
    // if Unsplash is unavailable.
    let (wallpaper_url, photographer, photographer_url) = picture::wallpaper_context(state).await;
    let html = state
        .templates
        .get_template("contact.html")?
        .render(context! { wallpaper_url, photographer, photographer_url, submitted })?;
    Ok(Html(html))
}

pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    state.metrics.inc_page_view("contact");
    render(&state, false).await
}

#[cfg(test)]
mod tests {
    use crate::test::{start_app, test_client};
    use axum::http::StatusCode;

    #[tokio::test]
    async fn get_contact_returns_200_with_form() {
        let addr = start_app().await;
        let res = test_client()
            .get(format!("http://{addr}/contact"))
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
        assert!(body.contains("<title>Contact</title>"));
        assert!(body.contains(r#"name="name""#));
        assert!(body.contains(r#"name="email""#));
        assert!(body.contains(r#"name="message""#));
        assert!(body.contains(r#"name="_website""#));
        assert!(body.contains(r#"action="/contact""#));
        // nav chrome
        assert!(body.contains(r#"<a href="/">Home</a>"#));
        assert!(body.contains(r#"<a href="/singlethread">SingleThread</a>"#));
        assert!(body.contains(r#"<a href="/contact">Contact</a>"#));
    }
}
