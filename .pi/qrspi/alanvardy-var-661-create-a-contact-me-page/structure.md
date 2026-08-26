# Structure Outline

## Approach

Two vertical slices: the contact page (static form + nav) and the submission
pipeline (honeypot → Resend email → rate-limit tier). Each crosses all
layers and is independently testable. The rate-limit tier is grouped with
Phase 2 because it only guards the POST endpoint.

---

## Phase 1: Contact Form Page

Renders `GET /contact` as a full HTML page inside the shared layout,
with a form containing name, email, message, and hidden honeypot fields.
No form handling — the form's `action` points at `/contact` but the POST
handler doesn't exist yet.

**Files**:
- `templates/contact.html` — new
- `src/interfaces/handlers/contact/web.rs` — new
- `src/interfaces/handlers/contact/mod.rs` — new
- `src/interfaces/handlers/mod.rs` — modified
- `src/interfaces/routes.rs` — modified
- `templates/layout.html` — modified
- `ROUTES.md` — modified

**Key changes**:
- `pub mod contact;` — new handler domain in `handlers/mod.rs`
- `pub mod web;` — re-export in `contact/mod.rs`
- `pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError>` — new GET handler
- `context!{ wallpaper_url, photographer, photographer_url }` + `{% block content %}` — template contract identical to home/singlethread
- `<form action="/contact" method="post">` with fields `name`, `email`, `message`, `_website` — template form body
- `<a href="/contact">Contact</a>` — new nav link in `layout.html`
- `GET /contact` route in `routes.rs` — follows `handlers::home::web::index` pattern (global budget only, no tier)

**Verify**: `./scripts/test.sh` passes; `GET http://localhost:3000/contact` returns 200 HTML with `<form>`
containing name/email/message/\_website fields and the "Contact" nav link appears on `/` and `/singlethread`.

---

## Phase 2: Form Submission → Email

Accepts `POST /contact` with the form body, validates the honeypot, sends
the message through the Resend API, and returns a thank-you page. Adds a
dedicated rate-limit tier so contact submissions don't compete with the
global budget. This phase introduces the `RESEND_API_KEY` env key and a new
`infra::resend` module following the Unsplash pattern.

**Files**:
- `src/interfaces/handlers/contact/web.rs` — modified (add POST handler)
- `src/interfaces/routes.rs` — modified (add contact tier, POST route)
- `src/app/rate_limit.rs` — modified (tier constants)
- `src/app/env.rs` — modified (new `resend_api_key` field)
- `src/app/state.rs` — modified (new `resend_base_url` field)
- `src/infra/resend.rs` — new
- `src/infra/mod.rs` — modified (add `resend` module)
- `src/main.rs` — modified (const `RESEND_BASE_URL`, build state field)
- `.env_template` — modified (add `RESEND_API_KEY`)
- `src/test/mod.rs` — modified (Env constructions + Resend stub)
- `ROUTES.md` — modified (add POST route docs)
- `src/app/error.rs` — potentially modified (add `From<ResendError>`, if not using `UnsplashError`)

**Key changes**:

```rust
// contact/web.rs — form struct (serde::Deserialize)
#[derive(Deserialize)]
struct ContactForm {
    name: String,
    email: String,
    message: String,
    _website: Option<String>, // honeypot
}

// contact/web.rs — POST handler signature
pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<ContactForm>,
) -> Result<Html<String>, WebError>

// contact/web.rs — honeypot check pattern
if form._website.is_some() {
    // return 200 silently; don't send email
}

// infra/resend.rs — error type (follows UnsplashError pattern)
#[derive(Debug)]
pub struct ResendError(pub String);

// infra/resend.rs — send function signature
pub async fn send_contact_email(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    from_name: &str,  // hardcoded, e.g. "Contact Form <noreply@vardy.cc>"
    to_email: &str,    // derived/configured, e.g. "alan@vardy.cc"
    subject: &str,
    body: &str,
) -> Result<(), ResendError>

// env.rs — new field + init
pub resend_api_key: String,
let resend_api_key = get_string_env("RESEND_API_KEY");

// state.rs — new field (optional: only if we want stub-overridable base URL)
pub resend_base_url: Arc<str>,

// rate_limit.rs — new tier constants
pub const CONTACT_TIER_PER_MS: u64 = 1_000;
pub const CONTACT_TIER_BURST: u32 = 2;

// routes.rs — contact tier router (pattern: dump/unsplash tier)
let contact_tier = crate::app::rate_limit::tiered_routes(
    Router::new().route("/contact", post(handlers::contact::web::create)),
    CONTACT_TIER_PER_MS,
    CONTACT_TIER_BURST,
);
// Merge into base router alongside dump_tier, unsplash_tier
```

**Error mapping for Resend failures**:
Resend transport/status/parse errors all collapse into the existing
`WebError::External` variant, same as Unsplash. Either reuse `UnsplashError`
→ `WebError::External` by giving `ResendError` the same `pub String`
shape and adding a `From<ResendError>` impl, or unify both into a shared
`ExternalError(String)` in `infra/` and use a single `From` impl. Prefer
the simpler path: add `From<infra::resend::ResendError> for WebError`.

**Arkitect allow-list**: `infra/resend.rs` depends on `reqwest` and `serde`
(already in `infra_deps`). If the handler directly constructs Resend
types, either re-export through `app/state.rs` (pattern: `pub use
crate::infra::resend::send_contact_email;`) or ensure the arch test allows
`vardy::infra` from interfaces — it already does.

**Test harness ripple**: Every hand-built `Env` literal (`test/mod.rs`
`serve_app`, `start_app_with_metrics`; `picture.rs` tests) adds
`resend_api_key: "test-key".into()` plus `resend_base_url: RESEND_BASE_URL.into()`.
The Resend stub (`start_resend_stub`) follows `start_unsplash_stub`:
spawns an axum router handling `POST /emails`, returns canned `{"id":"email_test"}`,
counts calls via `Arc<AtomicUsize>`.

**Integration tests to add** (in `contact/web.rs` `#[cfg(test)]`):

| Test | What it proves |
|------|---------------|
| `get_contact_returns_200` | Page exists (from Phase 1, confirmed) |
| `post_valid_form_sends_email` | Happy path: Resend called, 200 returned |
| `post_honeypot_filled_skips_email` | Honeypot filled → Resend stub not called, page still returns 200 |
| `post_resend_failure_returns_502` | Resend stub returns 500 → handler returns 502 |
| `post_too_many_requests_returns_429` | Burst 2, rapid submissions → 429 with `retry-after` |

**Verify**: `./scripts/test.sh` passes; all five integration tests pass.
Manual: `POST http://localhost:3000/contact` with valid form data (and
`RESEND_API_KEY` set in `.env`) delivers a real email to the configured
inbox.

---

## Testing Checkpoints

| After Phase | What must be true |
|-------------|-------------------|
| Phase 1 | `GET /contact` → 200 HTML with form; `/contact` in nav on all pages; `ROUTES.md` documents GET route |
| Phase 2 | All Phase 1 assertions still hold; POST with valid fields hits Resend stub (or real endpoint) exactly once; honeypot-filled POST returns 200 without calling Resend; Resend 500 → 502; bursting POST returns 429; tier budgets do not affect GET `/contact` |