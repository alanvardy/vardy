# Implementation Plan

## Overview

Add a `GET /contact` page with an HTML form (name, email, message, honeypot)
and a `POST /contact` endpoint that validates the honeypot and sends the
message to `alan@vardy.cc` via the Resend API, protected by a dedicated
rate-limit tier. All errors flow through the existing `WebError` chokepoint.

---

## Phase 1: Contact Form Page

Renders `GET /contact` as a full HTML page in the shared layout with a form
containing name, email, message, and hidden honeypot fields. No form handling
yet — `POST /contact` is added in Phase 2.

### Changes

#### 1. New handler domain `contact`
**File**: `src/interfaces/handlers/mod.rs`
**Action**: modify

```rust
pub mod contact;
pub mod dump;
pub mod home;
pub mod metrics;
pub mod singlethread;
pub mod unsplash;
```

**File**: `src/interfaces/handlers/contact/mod.rs`
**Action**: create

```rust
pub mod web;
```

#### 2. GET handler
**File**: `src/interfaces/handlers/contact/web.rs`
**Action**: create

Follows the `home::web::index` pattern exactly: inc page-view metric, fetch
decorative wallpaper context, render template.

```rust
use axum::{extract::State, response::Html};
use minijinja::context;

use crate::app::error::WebError;
use crate::app::picture;
use crate::app::state::AppState;

/// Shared render helper so GET (form) and POST (thank-you) render the same
/// template with a `submitted` flag selecting the branch.
async fn render(state: &AppState, submitted: bool) -> Result<Html<String>, WebError> {
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
```

#### 3. Contact template
**File**: `templates/contact.html`
**Action**: create

Extends `layout.html`, supplies the three contract fields via the handler's
`context!{}` (not in the template), and renders the form. `submitted` is
falsy/absent for GET so the form shows; Phase 2 sets it to `true` for the
thank-you branch. The honeypot is hidden with inline CSS (no Tailwind class,
so it cannot trip the CSS-drift check or depend on the utility set).

```html
{% extends "layout.html" %}
{% block title %}Contact{% endblock %}
{% block heading %}Contact{% endblock %}
{% block content %}
{% if submitted %}
  <p class="text-xl text-accent">Thank you for your message — I'll get back to you soon.</p>
{% else %}
  <form action="/contact" method="post" class="flex flex-col gap-4">
    <div class="flex flex-col gap-2">
      <label for="name" class="text-muted">Name</label>
      <input id="name" name="name" type="text" required
             class="w-full rounded border border-border bg-surface text-text p-2">
    </div>
    <div class="flex flex-col gap-2">
      <label for="email" class="text-muted">Email</label>
      <input id="email" name="email" type="email" required
             class="w-full rounded border border-border bg-surface text-text p-2">
    </div>
    <div class="flex flex-col gap-2">
      <label for="message" class="text-muted">Message</label>
      <textarea id="message" name="message" rows="6" required
                class="w-full rounded border border-border bg-surface text-text p-2"></textarea>
    </div>
    {# Honeypot: bots fill this, humans never see it (CSS-hidden, not `type="hidden"`) #}
    <input type="text" name="_website" value="" tabindex="-1" autocomplete="off"
           aria-hidden="true"
           style="position:absolute;left:-9999px;width:1px;height:1px;overflow:hidden">
    <button type="submit"
            class="rounded bg-accent text-bg font-semibold px-4 py-2 self-start">
      Send message
    </button>
  </form>
{% endif %}
{% endblock %}
```

> The form element classes (`w-full`, `rounded`, `border-border`,
> `bg-surface`, `text-text`, `text-bg`, …) are new to the utility set. The
> Tailwind build in `test.sh` will generate them; commit the regenerated
> `static/site.css`.

#### 4. GET route
**File**: `src/interfaces/routes.rs`
**Action**: modify

Add to the base router (global budget only, matching `home::web::index`):

```rust
Router::new()
    .route("/", get(handlers::home::web::index))
    .route("/singlethread", get(handlers::singlethread::web::index))
    .route("/contact", get(handlers::contact::web::index))
    .route("/dump/{key}", get(handlers::dump::web::index)) // global budget only
    // ...
```

#### 5. Navigation link
**File**: `templates/layout.html`
**Action**: modify

```html
<nav>
    <a href="/">Home</a>
    <a href="/singlethread">SingleThread</a>
    <a href="/contact">Contact</a>
</nav>
```

#### 6. Route docs
**File**: `ROUTES.md`
**Action**: modify

Insert a self-contained `### GET /contact` block (between the
`### GET /singlethread` section and its closing `---`, or after it):

```markdown
### GET /contact

Renders the contact form (name, email, message, and a CSS-hidden honeypot
field) with a random Unsplash wallpaper and photographer credit that degrade
to hidden when the Unsplash fetch fails.

- Response: `200 OK` — `text/html` (minijinja `templates/contact.html`)
- Errors: `500` via `WebError` (template render failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.

---
```

#### 7. GET handler test
**File**: `src/interfaces/handlers/contact/web.rs` (inline `#[cfg(test)] mod tests`)
**Action**: modify (add at bottom)

```rust
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
        assert!(res.headers().get("content-type")
            .is_some_and(|v| v.to_str().unwrap().contains("text/html")));
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
```

### Verification

#### Automated
- [x] `./scripts/test.sh` passes (format, sqlx prepare, check, CSS build + drift, clippy, nextest)
- [x] `cargo nextest run` — `get_contact_returns_200_with_form` passes
- [x] Existing `home` and `singlethread` nav assertions still pass (adding the link does not break them)

#### Manual
- [ ] Run the server; `GET http://localhost:3000/contact` returns 200 HTML with the `<form>` and the four fields (`name`, `email`, `message`, `_website`)
- [ ] `/` and `/singlethread` each show the new "Contact" nav link
- [ ] `static/site.css` shows no git diff (drift check green) after the CSS build

---

## Phase 2: Form Submission → Email

Accepts `POST /contact`, validates the honeypot, sends via Resend, returns a
thank-you page, and adds a dedicated rate-limit tier. Introduces
`RESEND_API_KEY` and the `infra::resend` module (mirrors `infra::unsplash`).

### Changes

#### 1. Infra: Resend client
**File**: `src/infra/resend.rs`
**Action**: create

Follows `infra::unsplash.rs` exactly: one `pub` error type, one `pub` function,
three failure classes collapsed into the error.

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct SendEmailRequest {
    from: String,
    to: [String; 1],
    subject: String,
    text: String,
}

#[derive(Deserialize)]
struct SendEmailResponse {
    #[allow(dead_code)] // parsed only to prove the API accepted the message
    id: String,
}

/// Failure talking to the Resend API; translated into
/// `WebError::External` (HTTP 502) at the app layer.
#[derive(Debug)]
pub struct ResendError(pub String);

/// Send a plain-text contact email through the Resend API.
/// Non-2xx status or parse failure maps to `WebError::External` (HTTP 502)
/// via `From<ResendError>`.
pub async fn send_contact_email(
    client: &Client,
    base_url: &str,
    api_key: &str,
    from: &str,
    to: &str,
    subject: &str,
    text: &str,
) -> Result<(), ResendError> {
    let response = client
        .post(format!("{base_url}/emails"))
        .bearer_auth(api_key)
        .json(&SendEmailRequest {
            from: from.to_owned(),
            to: [to.to_owned()],
            subject: subject.to_owned(),
            text: text.to_owned(),
        })
        .send()
        .await
        .map_err(|e| ResendError(format!("resend request failed: {e}")))?;

    if !response.status().is_success() {
        return Err(ResendError(format!(
            "resend returned status {}",
            response.status()
        )));
    }

    response
        .json::<SendEmailResponse>()
        .await
        .map_err(|e| ResendError(format!("resend response parse failed: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_resend_email_response() {
        let parsed: SendEmailResponse =
            serde_json::from_value(serde_json::json!({ "id": "abc123" })).unwrap();
        assert_eq!(parsed.id, "abc123");
    }
}
```

**File**: `src/infra/mod.rs`
**Action**: modify

```rust
pub mod metrics;
pub mod resend;
pub mod sentry;
pub mod unsplash;
```

#### 2. App: form type + send orchestration
**File**: `src/app/contact.rs`
**Action**: create

> **Deviation from structure.md**: the structure places `ContactForm` in the
> handler, but `interfaces` may not `use serde` (arkitect allow-list at
> `src/test/arkitect.rs` includes `serde_json` but not `serde`). Putting the
> `Deserialize` derive here matches the existing `DumpEntry` precedent
> (`src/app/dump.rs`) and avoids weakening the arch test. The handler reaches
> it via `crate::app::contact`.

```rust
use serde::Deserialize;

use crate::app::error::WebError;
use crate::app::state::AppState;
use crate::infra::resend;

#[derive(Deserialize)]
pub struct ContactForm {
    pub name: String,
    pub email: String,
    pub message: String,
    /// Honeypot: `None`/empty for humans, non-empty means a bot filled it.
    pub _website: Option<String>,
}

/// Send a contact email using the shared HTTP client and Resend config
/// carried on `AppState`.
pub async fn send(
    state: &AppState,
    from: &str,
    to: &str,
    subject: &str,
    text: &str,
) -> Result<(), WebError> {
    resend::send_contact_email(
        &state.http,
        &state.resend_base_url,
        &state.env.resend_api_key,
        from,
        to,
        subject,
        text,
    )
    .await?;
    Ok(())
}
```

#### 3. Error mapping
**File**: `src/app/error.rs`
**Action**: modify

Add alongside the existing `From<UnsplashError>`:

```rust
impl From<crate::infra::resend::ResendError> for WebError {
    fn from(err: crate::infra::resend::ResendError) -> Self {
        WebError::External(err.0)
    }
}
```

Add a test in the same file's `#[cfg(test)] mod tests`:

```rust
#[test]
fn resend_error_is_502() {
    let res = WebError::from(crate::infra::resend::ResendError("boom".into()))
        .into_response();
    assert_eq!(res.status(), StatusCode::BAD_GATEWAY);
}
```

#### 4. Env key
**File**: `src/app/env.rs`
**Action**: modify

Add field, init call, and struct-literal entry:

```rust
pub struct Env {
    pub unsplash_api_key: String,
    pub resend_api_key: String,
    pub database_url: String,
    // ...
}

impl Env {
    pub fn init() -> Env {
        let unsplash_api_key = get_string_env("UNSPLASH_API_KEY");
        let resend_api_key = get_string_env("RESEND_API_KEY");
        // ...
        Env {
            unsplash_api_key,
            resend_api_key,
            // ...
        }
    }
}
```

#### 5. State field
**File**: `src/app/state.rs`
**Action**: modify

```rust
/// Overridable so tests can point at a local stub server.
pub resend_base_url: Arc<str>,
```

#### 6. `main.rs` const + state
**File**: `src/main.rs`
**Action**: modify

```rust
const UNSPLASH_BASE_URL: &str = "https://api.unsplash.com";
const RESEND_BASE_URL: &str = "https://api.resend.com";

// in AppState literal:
        unsplash_base_url: UNSPLASH_BASE_URL.into(),
        resend_base_url: RESEND_BASE_URL.into(),
```

#### 7. Rate-limit tier constants
**File**: `src/app/rate_limit.rs`
**Action**: modify

```rust
pub const DUMP_TIER_PER_MS: u64 = 1_000;
pub const DUMP_TIER_BURST: u32 = 3;
pub const UNSPLASH_TIER_PER_MS: u64 = 200;
pub const UNSPLASH_TIER_BURST: u32 = 5;
pub const CONTACT_TIER_PER_MS: u64 = 1_000; // 1 submission/sec sustained
pub const CONTACT_TIER_BURST: u32 = 2;      // allow one retry
```

#### 8. POST handler
**File**: `src/interfaces/handlers/contact/web.rs`
**Action**: modify

```rust
use axum::extract::{Form, State};
use crate::app::contact::{self, ContactForm};

const FROM_EMAIL: &str = "Contact Form <noreply@vardy.cc>";
const TO_EMAIL: &str = "alan@vardy.cc";

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
    let text = format!("Name: {}\nEmail: {}\n\n{}", form.name, form.email, form.message);
    contact::send(&state, FROM_EMAIL, TO_EMAIL, &subject, &text).await?;

    render(&state, true).await
}
```

> **Correction to structure.md**: the structure's snippet `if
> form._website.is_some()` is wrong — `serde_urlencoded 0.7.1` returns
> `Some("")` for an empty submitted field, so every human submission would
> be skipped. Use `is_some_and(|w| !w.trim().is_empty())`.

#### 9. POST route + contact tier
**File**: `src/interfaces/routes.rs`
**Action**: modify

```rust
let contact_tier = crate::app::rate_limit::tiered_routes(
    Router::new().route("/contact", axum::routing::post(handlers::contact::web::create)),
    crate::app::rate_limit::CONTACT_TIER_PER_MS,
    crate::app::rate_limit::CONTACT_TIER_BURST,
);

Router::new()
    .route("/", get(handlers::home::web::index))
    .route("/singlethread", get(handlers::singlethread::web::index))
    .route("/contact", get(handlers::contact::web::index)) // global budget only
    .route("/dump/{key}", get(handlers::dump::web::index))
    .merge(dump_tier)
    .merge(unsplash_tier)
    .merge(contact_tier)
    // ...
```

#### 10. Env template
**File**: `.env_template`
**Action**: modify

```
RESEND_API_KEY=XXXX
```

(Add next to the other keys; the file's header comment already lists the four
places — `.env`, `.env_template`, fly.io dashboard, 1Password. `RESEND_API_KEY`
must also be added to `.env` locally and `fly secrets set RESEND_API_KEY=…`
out-of-band; neither `.env` nor Fly secrets are in-repo.)

#### 11. Test harness: Env/state + Resend stub
**File**: `src/test/mod.rs`
**Action**: modify

- Extend `use axum::{... routing::{get, post}};`.
- Add `resend_api_key: "test-key".into()` to **both** `Env { … }` literals
  (`serve_app` ~line 34, `start_app_with_metrics` ~line 79).
- Add `resend_base_url: RESEND_BASE_URL.into()` (or the literal
  `"https://api.resend.com".into()`) to **both** `AppState { … }` literals.
- Change `serve_app` to accept the Resend base URL and thread it through:

```rust
const RESEND_BASE_URL: &str = "https://api.resend.com";

async fn serve_app(
    unsplash_base_url: &str,
    resend_base_url: &str,
    per_ms: u64,
    burst: u32,
) -> (SocketAddr, SqlitePool) {
    // ... env gets resend_api_key: "test-key".into()
    // ... state gets resend_base_url: resend_base_url.into()
}

pub async fn start_app_with_resend(resend_base_url: &str) -> (SocketAddr, SqlitePool) {
    serve_app("https://api.unsplash.com", resend_base_url, 1, 1_000_000).await
}

pub async fn start_app_with_resend_and_rate_limits(
    resend_base_url: &str,
    per_ms: u64,
    burst: u32,
) -> (SocketAddr, SqlitePool) {
    serve_app("https://api.unsplash.com", resend_base_url, per_ms, burst).await
}
```

- Update the existing wrappers to pass `RESEND_BASE_URL`:
  `start_app_with` → `serve_app(unsplash_base_url, RESEND_BASE_URL, 1, 1_000_000)`;
  `start_app_with_rate_limits` → `serve_app(unsplash_base_url, RESEND_BASE_URL, per_ms, burst)`.
- Add the stub (mirrors `start_unsplash_stub`):

```rust
pub struct ResendStub {
    pub base_url: String,
    pub call_count: Arc<AtomicUsize>,
}

/// Spawn a local stub of `POST /emails`, returning canned `{"id":"email_test"}`
/// on success or the given status verbatim to simulate upstream failure.
pub async fn start_resend_stub(status: StatusCode) -> ResendStub {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&call_count);
    let app = Router::new().route(
        "/emails",
        post(move || {
            let count = Arc::clone(&count);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                if status.is_success() {
                    Json(json!({ "id": "email_test" })).into_response()
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.expect("server");
    });
    ResendStub { base_url: format!("http://{addr}"), call_count }
}
```

#### 12. Compile ripple: `picture.rs` test literals
**File**: `src/app/picture.rs`
**Action**: modify

The three `#[tokio::test]` cases (`random_below_threshold_fetches_and_inserts`,
`random_at_threshold_selects_without_upstream`, `random_upstream_failure_returns_error`)
each build an `Env { … }` and an `AppState { … }` literal (~lines 197/200,
252/255, 293/296). Add to each `Env` literal:

```rust
resend_api_key: "test-key".into(),
```

and to each `AppState` literal:

```rust
resend_base_url: "https://api.resend.com".into(),
```

#### 13. Route docs (POST)
**File**: `ROUTES.md`
**Action**: modify

Add a self-contained `### POST /contact` block:

```markdown
### POST /contact

Accepts the contact form, skips email when the honeypot is filled (returns the
thank-you page silently), otherwise sends the message to the configured inbox
via the Resend API and returns the thank-you page.

- Request body: `application/x-www-form-urlencoded` (`name`, `email`, `message`,
  `_website` honeypot)
- Response: `200 OK` — `text/html` thank-you page
- Errors: `502` via `WebError` (Resend API failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.
- Rate limit: also subject to a stricter dedicated tier (see
  `CONTACT_TIER_*` in `src/app/rate_limit.rs`) nested inside the global budget.

---
```

#### 14. Integration tests
**File**: `src/interfaces/handlers/contact/web.rs` (inline `#[cfg(test)] mod tests`)
**Action**: modify (extend)

```rust
use crate::test::{
    start_app_with_resend, start_app_with_resend_and_rate_limits, start_resend_stub, test_client,
};
use std::sync::atomic::Ordering;

#[tokio::test]
async fn post_valid_form_sends_email() {
    let stub = start_resend_stub(StatusCode::OK).await;
    let (addr, _) = start_app_with_resend(&stub.base_url).await;
    let res = test_client()
        .post(format!("http://{addr}/contact"))
        .form(&[("name", "Alan"), ("email", "a@b.cc"), ("message", "hi")])
        .send().await.expect("request failed");
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(stub.call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn post_honeypot_filled_skips_email() {
    let stub = start_resend_stub(StatusCode::OK).await;
    let (addr, _) = start_app_with_resend(&stub.base_url).await;
    let res = test_client()
        .post(format!("http://{addr}/contact"))
        .form(&[("name", "Bot"), ("email", "b@b.cc"), ("message", "spam"), ("_website", "http://spam")])
        .send().await.expect("request failed");
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
        .send().await.expect("request failed");
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
            .send().await.expect("request failed");
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
```

Note: the 429 test uses `start_app_with_resend_and_rate_limits` so the burst
requests that pass the tier hit the stub instead of a real network call. Use
`per_ms = 1, burst = 1_000_000` for the global limiter (effectively disabled),
matching `dump/web.rs`'s tier test.

### Verification

#### Automated
- [x] `./scripts/test.sh` passes end-to-end (format, sqlx prepare, check, CSS build + drift, clippy, nextest)
- [x] `cargo nextest run` — all five contact tests pass: `get_contact_returns_200_with_form`, `post_valid_form_sends_email`, `post_honeypot_filled_skips_email`, `post_resend_failure_returns_502`, `post_too_many_requests_returns_429`
- [x] `cargo test --lib test_architectural_rules` passes (new `app::contact` module is `app`-layer; no `serde`/`reqwest` leak into `interfaces`)

#### Manual
- [ ] With `RESEND_API_KEY` set in `.env` and the server running, `POST /contact` with valid form data delivers a real email to the configured inbox
- [ ] Honeypot-filled `POST /contact` returns 200 and sends no email
- [ ] `GET /contact` remains 200 and is unaffected by the POST tier budget

---

## Deviations from structure.md (summary)

1. **`src/app/contact.rs` is new** (not in structure's file list). `ContactForm`
   lives here, not in the handler, because the arkitect allow-list for
   `interfaces` does not permit `serde`. This matches the `DumpEntry`
   precedent (`src/app/dump.rs`) and avoids modifying `src/test/arkitect.rs`.
   The handler imports it via `crate::app::contact`. Consequence: `state.rs`
   only gains the `resend_base_url` field — the
   `pub use ... send_contact_email` re-export the structure suggested is
   unnecessary because `app::contact::send` wraps the call.

2. **Honeypot check corrected** to `is_some_and(|w| !w.trim().is_empty())`.
   `serde_urlencoded 0.7.1` deserializes a present-but-empty field to
   `Some("")`, so `is_some()` would block every human submission.

3. **`src/app/picture.rs` is an added ripple file** (not in structure's file
   list): its three test `Env`/`AppState` literals need the two new fields to
   compile.

4. `RESEND_BASE_URL` is a `const` in `main.rs` (per structure); the test
   harness repeats the literal `"https://api.resend.com"` the same way it
   already repeats the Unsplash URL.
