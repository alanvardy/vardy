use axum::{
    extract::{Form, State},
    response::Html,
};
use minijinja::context;

use crate::app::contact::{self, ContactForm};
use crate::app::error::WebError;
use crate::app::picture;
use crate::app::state::AppState;

const FROM_EMAIL: &str = "Contact Form <noreply@vardy.cc>";
const TO_EMAIL: &str = "alan@vardy.cc";

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

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<ContactForm>,
) -> Result<Html<String>, WebError> {
    // Honeypot: serde_urlencoded maps a present-but-empty field to
    // `Some("")`, so only a non-empty value means a bot filled it.
    if form._website.is_some_and(|w| !w.trim().is_empty()) {
        return render(&state, true).await; // silently accept, send nothing
    }

    let subject = format!("New contact message from {} <{}>", form.name, form.email);
    let text = format!(
        "Name: {}\nEmail: {}\n\n{}",
        form.name, form.email, form.message
    );
    contact::send(&state, FROM_EMAIL, TO_EMAIL, &subject, &text).await?;

    render(&state, true).await
}

#[cfg(test)]
mod tests {
    use crate::test::{
        start_app, start_app_with_resend, start_app_with_resend_and_rate_limits, start_resend_stub,
        test_client,
    };
    use axum::http::StatusCode;
    use std::sync::atomic::Ordering;

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
        assert!(body.contains(r#"<a href="/contact" class="active">Contact</a>"#));
    }

    #[tokio::test]
    async fn post_valid_form_sends_email() {
        let stub = start_resend_stub(StatusCode::OK).await;
        let (addr, _) = start_app_with_resend(&stub.base_url).await;
        let res = test_client()
            .post(format!("http://{addr}/contact"))
            .form(&[("name", "Alan"), ("email", "a@b.cc"), ("message", "hi")])
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(stub.call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn post_honeypot_filled_skips_email() {
        let stub = start_resend_stub(StatusCode::OK).await;
        let (addr, _) = start_app_with_resend(&stub.base_url).await;
        let res = test_client()
            .post(format!("http://{addr}/contact"))
            .form(&[
                ("name", "Bot"),
                ("email", "b@b.cc"),
                ("message", "spam"),
                ("_website", "http://spam"),
            ])
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(stub.call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn post_resend_failure_returns_502() {
        let stub = start_resend_stub(StatusCode::INTERNAL_SERVER_ERROR).await;
        let (addr, _) = start_app_with_resend(&stub.base_url).await;
        let res = test_client()
            .post(format!("http://{addr}/contact"))
            .form(&[("name", "Alan"), ("email", "a@b.cc"), ("message", "hi")])
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(res.text().await.unwrap(), "bad gateway");
    }

    #[tokio::test]
    async fn post_too_many_requests_returns_429() {
        let stub = start_resend_stub(StatusCode::OK).await;
        let (addr, _) = start_app_with_resend_and_rate_limits(&stub.base_url, 1, 1_000_000).await;
        let client = test_client();
        // CONTACT_TIER_BURST = 2; 10 rapid POSTs must trip the tier (global stays open)
        let mut saw_429 = false;
        for _ in 0..10 {
            let res = client
                .post(format!("http://{addr}/contact"))
                .form(&[("name", "Alan"), ("email", "a@b.cc"), ("message", "hi")])
                .send()
                .await
                .expect("request failed");
            match res.status() {
                StatusCode::TOO_MANY_REQUESTS => {
                    saw_429 = true;
                    assert!(res.headers().get("retry-after").is_some());
                    assert_eq!(res.text().await.unwrap(), "too many requests");
                }
                StatusCode::OK => {}
                s => panic!("unexpected status {s}"),
            }
        }
        assert!(saw_429, "expected at least one 429 within 10 rapid POSTs");
    }
}
