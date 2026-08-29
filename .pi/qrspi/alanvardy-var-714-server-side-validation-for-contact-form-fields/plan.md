# Implementation Plan

## Overview

Add server-side validation to the contact form: validate fields before calling Resend, re-render the form with an error banner and preserved input on failure. Built in four horizontal stages — each layer testable in isolation before the next begins.

---

## Stage 1: `WebError::BadRequest` variant

### Changes

#### 1. Add `BadRequest` variant to `WebError`
**File**: `src/app/error.rs`
**Action**: modify

Add the variant after `External(String)`:

```rust
pub enum WebError {
    Template(minijinja::Error),
    Database(sqlx::Error),
    NotFound,
    External(String),
    BadRequest(String),
    TooManyRequests { retry_after_secs: u64 },
}
```

Add the `IntoResponse` arm after the `External` arm, before the `TooManyRequests` comment + arm:

```rust
            // Client fault: log nothing to Sentry.
            WebError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
```

#### 2. Add unit test
**File**: `src/app/error.rs` (inside existing `#[cfg(test)] mod tests`)
**Action**: modify

Add after `external_error_is_502`:

```rust
    #[tokio::test]
    async fn bad_request_is_400_with_body() {
        let res = WebError::BadRequest("invalid input".into()).into_response();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(bytes.as_ref(), b"invalid input");
    }
```

### Verification
#### Automated
- [x] `cargo nextest run` passes

#### Manual
- [ ] N/A — unit test is self-verifying

---

## Stage 2: `ContactForm::validate()`

### Changes

#### 1. Add `validate()` method on `ContactForm`
**File**: `src/app/contact.rs`
**Action**: modify

Insert between the struct definition and the `send` function:

```rust
impl ContactForm {
    /// Validate form fields and return the first human-readable error, if any.
    /// Checks empty/whitespace fields first, then maximum lengths.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Please enter your name.".into());
        }
        if self.email.trim().is_empty() {
            return Err("Please enter your email address.".into());
        }
        if self.message.trim().is_empty() {
            return Err("Please enter a message.".into());
        }
        if self.name.len() > 200 {
            return Err("Name must be 200 characters or fewer.".into());
        }
        if self.email.len() > 254 {
            return Err("Email must be 254 characters or fewer.".into());
        }
        if self.message.len() > 10_000 {
            return Err("Message must be 10,000 characters or fewer.".into());
        }
        Ok(())
    }
}
```

#### 2. Add unit tests
**File**: `src/app/contact.rs` (add `#[cfg(test)] mod tests` block at bottom)
**Action**: modify

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn form(name: &str, email: &str, message: &str) -> ContactForm {
        ContactForm {
            name: name.into(),
            email: email.into(),
            message: message.into(),
            _website: None,
        }
    }

    #[test]
    fn valid_form_passes_validation() {
        assert!(form("Alan", "a@b.cc", "hi").validate().is_ok());
    }

    #[test]
    fn empty_name_rejected() {
        let err = form("", "a@b.cc", "hi").validate().unwrap_err();
        assert!(err.contains("name"));
    }

    #[test]
    fn whitespace_only_name_rejected() {
        let err = form("   ", "a@b.cc", "hi").validate().unwrap_err();
        assert!(err.contains("name"));
    }

    #[test]
    fn empty_email_rejected() {
        let err = form("Alan", "", "hi").validate().unwrap_err();
        assert!(err.contains("email"));
    }

    #[test]
    fn empty_message_rejected() {
        let err = form("Alan", "a@b.cc", "").validate().unwrap_err();
        assert!(err.contains("message"));
    }

    #[test]
    fn name_too_long_rejected() {
        let long = "a".repeat(201);
        assert!(form(&long, "a@b.cc", "hi").validate().is_err());
    }

    #[test]
    fn name_at_boundary_accepted() {
        let max = "a".repeat(200);
        assert!(form(&max, "a@b.cc", "hi").validate().is_ok());
    }

    #[test]
    fn email_too_long_rejected() {
        let long = "a".repeat(255);
        assert!(form("Alan", &long, "hi").validate().is_err());
    }

    #[test]
    fn message_too_long_rejected() {
        let long = "a".repeat(10_001);
        assert!(form("Alan", "a@b.cc", &long).validate().is_err());
    }

    #[test]
    fn message_at_boundary_accepted() {
        let max = "a".repeat(10_000);
        assert!(form("Alan", "a@b.cc", &max).validate().is_ok());
    }
}
```

### Verification
#### Automated
- [x] `cargo nextest run` passes (new unit tests green; existing tests unchanged)

#### Manual
- [ ] N/A — unit tests are self-verifying

---

## Stage 3: Handler — validate before Resend, re-render on failure

### Changes

#### 1. Extend `render()` signature and add validation check in `create()`
**File**: `src/interfaces/handlers/contact/web.rs`
**Action**: modify

Replace the existing `render()` function (lines 17–27) and `create()` handler (lines 34–51) with:

```rust
/// Shared render helper: GET (form), POST validation failure (form + error),
/// and POST success (thank-you) all render the same template with a
/// `submitted` flag selecting the branch.
async fn render(
    state: &AppState,
    submitted: bool,
    error: Option<&str>,
    name: &str,
    email: &str,
    message: &str,
) -> Result<Html<String>, WebError> {
    // The wallpaper and its photographer credit are decorative fallbacks:
    // render the page without them rather than failing the whole request
    // if Unsplash is unavailable.
    let (wallpaper_url, photographer, photographer_url) = picture::wallpaper_context(state).await;
    let html = state
        .templates
        .get_template("contact.html")?
        .render(context! {
            wallpaper_url, photographer, photographer_url,
            submitted,
            error,
            name,
            email,
            message,
            active_page => "contact",
        })?;
    Ok(Html(html))
}

pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    state.metrics.inc_page_view("contact");
    render(&state, false, None, "", "", "").await
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<ContactForm>,
) -> Result<Html<String>, WebError> {
    // Honeypot: serde_urlencoded maps a present-but-empty field to
    // `Some("")`, so only a non-empty value means a bot filled it.
    if form._website.is_some_and(|w| !w.trim().is_empty()) {
        return render(&state, true, None, "", "", "").await; // silently accept, send nothing
    }

    // Validate before touching Resend — bad input re-renders the form.
    if let Err(msg) = form.validate() {
        return render(&state, false, Some(&msg), &form.name, &form.email, &form.message).await;
    }

    let subject = format!("New contact message from {} <{}>", form.name, form.email);
    let text = format!(
        "Name: {}\nEmail: {}\n\n{}",
        form.name, form.email, form.message
    );
    contact::send(&state, FROM_EMAIL, TO_EMAIL, &subject, &text).await?;

    render(&state, true, None, "", "", "").await
}
```

Note: `index()` changes from `render(&state, false)` to `render(&state, false, None, "", "", "")` — same behavior.

#### 2. Add integration tests
**File**: `src/interfaces/handlers/contact/web.rs` (inside existing `#[cfg(test)] mod tests`, after `post_too_many_requests_returns_429`)
**Action**: modify

```rust
    #[tokio::test]
    async fn post_empty_name_returns_200_and_skips_email() {
        let stub = start_resend_stub(StatusCode::OK).await;
        let (addr, _) = start_app_with_resend(&stub.base_url).await;
        let res = test_client()
            .post(format!("http://{addr}/contact"))
            .form(&[("name", ""), ("email", "a@b.cc"), ("message", "hi")])
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(stub.call_count.load(Ordering::SeqCst), 0);
        let body = res.text().await.unwrap();
        assert!(body.contains("<form"));
    }

    #[tokio::test]
    async fn post_empty_email_returns_200_and_skips_email() {
        let stub = start_resend_stub(StatusCode::OK).await;
        let (addr, _) = start_app_with_resend(&stub.base_url).await;
        let res = test_client()
            .post(format!("http://{addr}/contact"))
            .form(&[("name", "Alan"), ("email", ""), ("message", "hi")])
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(stub.call_count.load(Ordering::SeqCst), 0);
        assert!(res.text().await.unwrap().contains("<form"));
    }

    #[tokio::test]
    async fn post_empty_message_returns_200_and_skips_email() {
        let stub = start_resend_stub(StatusCode::OK).await;
        let (addr, _) = start_app_with_resend(&stub.base_url).await;
        let res = test_client()
            .post(format!("http://{addr}/contact"))
            .form(&[("name", "Alan"), ("email", "a@b.cc"), ("message", "")])
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(stub.call_count.load(Ordering::SeqCst), 0);
        assert!(res.text().await.unwrap().contains("<form"));
    }

    #[tokio::test]
    async fn post_over_length_name_returns_200_and_skips_email() {
        let stub = start_resend_stub(StatusCode::OK).await;
        let (addr, _) = start_app_with_resend(&stub.base_url).await;
        let long_name = "a".repeat(201);
        let res = test_client()
            .post(format!("http://{addr}/contact"))
            .form(&[
                ("name", &long_name),
                ("email", "a@b.cc"),
                ("message", "hi"),
            ])
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(stub.call_count.load(Ordering::SeqCst), 0);
        assert!(res.text().await.unwrap().contains("<form"));
    }

    #[tokio::test]
    async fn post_missing_name_key_returns_422() {
        let stub = start_resend_stub(StatusCode::OK).await;
        let (addr, _) = start_app_with_resend(&stub.base_url).await;
        let res = test_client()
            .post(format!("http://{addr}/contact"))
            .form(&[("email", "a@b.cc"), ("message", "hi")])
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
```

### Verification
#### Automated
- [x] `./scripts/test.sh` passes (full gate: fmt, sqlx, check, CSS drift, clippy, nextest, no TODOs)
- [x] `post_valid_form_sends_email` continues to pass (regression — validates existing behavior unchanged)

#### Manual
- [ ] N/A — integration tests are self-verifying

---

## Stage 4: Template — error banner + preserved input

### Changes

#### 1. Add error banner and `value=` attributes to form
**File**: `templates/contact.html`
**Action**: modify

In the `{% else %}` branch (form), insert the error banner block immediately before `<form action="/contact" method="post" ...>`, and add `value=` attributes on the visible fields.

The `{% else %}` block (lines 21–40) becomes:

```html
        {% else %}
            {% if error %}
            <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded mb-4" role="alert">
                {{ error }}
            </div>
            {% endif %}
            <form action="/contact" method="post" class="flex flex-col gap-4">
                <div class="flex flex-col gap-2">
                    <label for="name" class="form-label">Name</label>
                    <input id="name" name="name" type="text" required class="form-input" value="{{ name }}">
                </div>
                <div class="flex flex-col gap-2">
                    <label for="email" class="form-label">Email</label>
                    <input id="email" name="email" type="email" required class="form-input" value="{{ email }}">
                </div>
                <div class="flex flex-col gap-2">
                    <label for="message" class="form-label">Message</label>
                    <textarea id="message" name="message" rows="6" required class="form-input">{{ message }}</textarea>
                </div>
                {# Honeypot: bots fill this, humans never see it (CSS-hidden, not type="hidden") #}
                <input type="text" name="_website" value="" tabindex="-1" autocomplete="off"
                       aria-hidden="true"
                       style="position:absolute;left:-9999px;width:1px;height:1px;overflow:hidden">
                <button type="submit"
                        class="btn self-start rounded-full inline-flex items-center justify-center gap-2 cursor-pointer
                               transition-all duration-200 hover:-translate-y-0.5 hover:shadow-md
                               active:translate-y-0 active:shadow-sm
                               focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2
                               focus-visible:outline-accent-strong">
                    Send message
                </button>
            </form>
        {% endif %}
```

Note: when `name`/`email`/`message` are empty strings (GET path, honeypot path, successful POST), `value=""` or an empty textarea body renders identically to the current no-`value` behavior.

#### 2. Extend validation-failure tests with template assertions
**File**: `src/interfaces/handlers/contact/web.rs` (modify the tests added in Stage 3)
**Action**: modify

In each of the four `*_returns_200_and_skips_email` tests, add assertions for:
- Error banner CSS class present in body
- Specific error text present
- Preserved field values present

Updated tests (replace the Stage 3 versions):

```rust
    #[tokio::test]
    async fn post_empty_name_returns_200_and_skips_email() {
        let stub = start_resend_stub(StatusCode::OK).await;
        let (addr, _) = start_app_with_resend(&stub.base_url).await;
        let res = test_client()
            .post(format!("http://{addr}/contact"))
            .form(&[("name", ""), ("email", "a@b.cc"), ("message", "hi")])
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(stub.call_count.load(Ordering::SeqCst), 0);
        let body = res.text().await.unwrap();
        assert!(body.contains("<form"));
        assert!(body.contains("bg-red-100"));
        assert!(body.contains("Please enter your name."));
        assert!(body.contains(r#"value="a@b.cc""#));
        assert!(body.contains("hi</textarea>"));
    }

    #[tokio::test]
    async fn post_empty_email_returns_200_and_skips_email() {
        let stub = start_resend_stub(StatusCode::OK).await;
        let (addr, _) = start_app_with_resend(&stub.base_url).await;
        let res = test_client()
            .post(format!("http://{addr}/contact"))
            .form(&[("name", "Alan"), ("email", ""), ("message", "hi")])
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(stub.call_count.load(Ordering::SeqCst), 0);
        let body = res.text().await.unwrap();
        assert!(body.contains("<form"));
        assert!(body.contains("bg-red-100"));
        assert!(body.contains("Please enter your email address."));
        assert!(body.contains(r#"value="Alan""#));
        assert!(body.contains("hi</textarea>"));
    }

    #[tokio::test]
    async fn post_empty_message_returns_200_and_skips_email() {
        let stub = start_resend_stub(StatusCode::OK).await;
        let (addr, _) = start_app_with_resend(&stub.base_url).await;
        let res = test_client()
            .post(format!("http://{addr}/contact"))
            .form(&[("name", "Alan"), ("email", "a@b.cc"), ("message", "")])
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(stub.call_count.load(Ordering::SeqCst), 0);
        let body = res.text().await.unwrap();
        assert!(body.contains("<form"));
        assert!(body.contains("bg-red-100"));
        assert!(body.contains("Please enter a message."));
        assert!(body.contains(r#"value="Alan""#));
        assert!(body.contains(r#"value="a@b.cc""#));
    }

    #[tokio::test]
    async fn post_over_length_name_returns_200_and_skips_email() {
        let stub = start_resend_stub(StatusCode::OK).await;
        let (addr, _) = start_app_with_resend(&stub.base_url).await;
        let long_name = "a".repeat(201);
        let res = test_client()
            .post(format!("http://{addr}/contact"))
            .form(&[
                ("name", &long_name),
                ("email", "a@b.cc"),
                ("message", "hi"),
            ])
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(stub.call_count.load(Ordering::SeqCst), 0);
        let body = res.text().await.unwrap();
        assert!(body.contains("<form"));
        assert!(body.contains("bg-red-100"));
        assert!(body.contains("Name must be 200 characters or fewer."));
    }
```

`post_missing_name_key_returns_422` remains unchanged from Stage 3 (no template assertions — axum's default rejection body is plain text, not HTML).

### Verification
#### Automated
- [x] `./scripts/test.sh` passes (full gate: fmt, sqlx, check, **CSS drift**, clippy, nextest, no TODOs)
- [x] If CSS drift check fails: run `./scripts/build-css.sh` to regenerate `static/site.css` with the new Tailwind classes (`bg-red-100`, `border-red-400`, `text-red-700`, `px-4`, `py-3`, `rounded`, `mb-4`), then commit the updated CSS

#### Manual
- [ ] N/A — integration tests with template assertions are self-verifying

---

## Testing Checkpoints

| Checkpoint | Command | What must be green |
|---|---|---|
| After Stage 1 | `cargo nextest run` | Unit test for `BadRequest` variant |
| After Stage 2 | `cargo nextest run` | Unit tests for `validate()` |
| After Stage 3 | `./scripts/test.sh` | Full gate: fmt, sqlx, check, CSS drift, clippy, nextest, no TODOs |
| After Stage 4 | `./scripts/test.sh` | Full gate, including CSS drift check |

---

## Layer Dependencies

```
Stage 1 (WebError::BadRequest) ── no deps, pure addition
    │
Stage 2 (ContactForm::validate()) ── no deps on Stage 1, pure logic
    │
Stage 3 (Handler) ─────────────── depends on Stage 1 + 2
    │
Stage 4 (Template) ──────────── depends on Stage 3 (handler passes context vars)
```

Stage 2 is independent of Stage 1; they share no code path. The order follows the codebase layering: `app/error.rs` → `app/contact.rs` → `interfaces/handlers/contact/web.rs` → `templates/`.

---

## Notes

- **`BadRequest` not used in the validation happy path** — the handler calls `render()` directly on validation failure (producing 200 HTML), not `WebError::BadRequest`. The `BadRequest` variant is added for completeness of the error type and future handlers that need a plain 400 text response.
- **No `From` impl for `BadRequest`** — there is no `Result<_, WebError>` propagation path where a `String` needs automatic conversion to `BadRequest`. Design decision 6 (`.map_err(WebError::BadRequest)?`) was re-evaluated: re-rendering with 200 is better UX than a bare 400 text response for form validation failures.
- **Single error message** — `validate()` returns one `String`, not per-field errors. The template's `{% if error %}` block shows it as a banner.
- **No new middleware or extractors** — `Form<ContactForm>` stays; missing-key rejection remains axum's default 422.
- **CSS drift** — the new Tailwind classes must be in `static/site.css`. If `./scripts/test.sh` fails on the CSS drift check, run `./scripts/build-css.sh` and commit the regenerated CSS.