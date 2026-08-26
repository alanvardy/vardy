# Design Discussion — Contact Me Page

## Current State

The vardy site serves two full HTML pages (`/`, `/singlethread`) and several
JSON endpoints through axum 0.8.9. Each page handler follows a uniform
pattern: inc page-view metric → fetch wallpaper context → render template →
`Ok(Html(html))` (`home/web.rs:7-16`, `singlethread/web.rs`). Templates
extend `layout.html` via minijinja, which requires `wallpaper_url`,
`photographer`, and `photographer_url` in every page's render context
(`templates/layout.html:1-4`). Navigation is hardcoded in
`layout.html:21-24`.

There is **no** form-handling endpoint anywhere in the codebase
(`research.md` Q2). The only `POST` is `Json<serde_json::Value>` at
`dump/web.rs:18-25`. Axum's built-in `Form` extractor is available but
unused. There is no email integration, no CAPTCHA, and no outbound HTTP
beyond the Unsplash `fetch_random` call (`infra/unsplash.rs`).

Rate limiting is a two-layer stack: a global `GovernorLayer` per-IP
(`rate_limit.rs:118-125`) + optional per-endpoint tiers with separate
budgets (`rate_limit.rs:130-141`). Two tiers exist today: dump (`1_000`ms
permit / burst `3`) and unsplash (`200`ms / burst `5`)
(`rate_limit.rs:84-87`).

## Desired End State

A `GET /contact` page renders an HTML form (name, email, message). `POST
/contact` accepts the form, validates via honeypot, and sends the message to
a personal inbox through the Resend API. Both handler and HTTP-call errors
flow through the existing `WebError` chokepoint.

Verification checklist:
- `GET /contact` returns 200 with the form rendered inside the shared layout
- Honeypot-filled submissions return 200 **without** sending email
- Valid submissions deliver email to the inbox
- Resend API failures return 502 via `WebError::External`
- The contact POST has its own rate-limit tier (not the global budget)
- `/contact` appears in the site nav
- Integration tests cover: happy path, honeypot trap, Resend stub failure,
  rate-limit trip

## Patterns to Follow

| Pattern | File:Line | Usage |
|---------|-----------|-------|
| Full-page handler: `State`, metric inc, `wallpaper_context`, template render | `home/web.rs:7-16` | `GET /contact` handler |
| Error chokepoint via `WebError` | `error.rs:39-65` | All error paths |
| Outbound HTTP: `state.http.get(url).header(...).send().await` | `infra/unsplash.rs` | Resend API call |
| External failure → `WebError::External` via `From` | `error.rs:26-29` | Resend → 502 |
| Per-endpoint rate-limit tier via `tiered_routes` | `rate_limit.rs:130-141`, `routes.rs:20-35` | `POST /contact` |
| Tier constants as `pub const` in `rate_limit.rs` | `rate_limit.rs:84-87` | Contact tier budget |
| Env key lifecycle: struct field → `init()` → `.env_template` → `.env` → `1Password` → Fly secrets | `env.rs:4-31`, `.env_template` | `RESEND_API_KEY` |
| Test harness: `start_app_with` + `test_client` | `test/mod.rs:19`, `mod.rs:129` | Contact integration tests |
| HTTP stub: custom axum router with `Arc<AtomicUsize>` call counter | `test/mod.rs:157-190` | Resend stub |
| Template extends `layout.html`, supplies contract fields | `home.html`, `singlethread.html` | `contact.html` |
| Navigation hardcoded in `layout.html` | `layout.html:21-24` | Add `/contact` link |
| Handler modules: `contact/web.rs` declared in `handlers/mod.rs` | `handlers/mod.rs:1-5` | New `contact` domain |
| Decorative fallback: `wallpaper_context` swallows errors | `picture.rs:23-27` | Share with contact page |
| Page-view metric via `state.metrics.inc_page_view` | `home/web.rs:8` | Call for contact GET |
| Immutable static cache via `asset_url` + `Cache-Control` header | `routes.rs:41-48`, `assets.rs:45-50` | Already applies |

### Patterns to Avoid
- **Don't** use `Json` extractor for HTML form bodies — use `axum::Form<T>`
  (available natively in axum 0.8, no extra dep needed).
- **Don't** store submissions in the DB (no schema change, no migration).
- **Don't** introduce a validator crate or new validation framework — keep
  it to serde `Deserialize` + manual honeypot check in the handler.
- **Don't** create a new error variant — reuse `WebError::External` for
  Resend failures, same as Unsplash.

## Design Decisions

1. **Bot protection: honeypot field** — A CSS-hidden `<input
   name="_website">` that real users never see. If filled, silently return
   200 without sending email. Zero friction for real users, no third-party
   dependency, no JS requirement. Not a hard security boundary, but
   sufficient for a personal contact form.

2. **Email provider: Resend** — Simple REST API (`POST
   https://api.resend.com/emails`, bearer token auth, JSON body). Follows
   the same `reqwest::Client` + `Authorization` header pattern as Unsplash
   (`infra/unsplash.rs`). No additional Rust crate needed.

3. **Storage: email only** — Send the email and forget. No
   `contact_submissions` table, no migration. If email delivery fails,
   `WebError::External` returns 502 and the visitor can retry. Keeps the
   implementation minimal.

4. **Rate-limit tier: dedicated contact tier** — `CONTACT_TIER_PER_MS=1_000`
   (one submission per second), `CONTACT_TIER_BURST=2` (allow a retry).
   Prevents one spammer from starving the rest of the site. Follows the
   existing `tiered_routes` pattern.

5. **Form handling: named struct with `serde::Deserialize`** —
   ```rust
   #[derive(Deserialize)]
   struct ContactForm {
       name: String,
       email: String,
       message: String,
       _website: Option<String>, // honeypot
   }
   ```
   Uses `axum::extract::Form<ContactForm>`. No new dependencies.

## What We're NOT Doing

- No DB storage for submissions (no migration, no model, no `contact_submissions` table)
- No CAPTCHA/third-party bot service
- No client-side JavaScript (honeypot is pure CSS)
- No email templates or HTML email — plain text or minimal markup inline
- No admin dashboard or submission viewer
- No email retry queue — Resend failure → 502, user retries
- No custom error variant — Resend failures map to the existing `WebError::External`
- No configuration UI — the recipient email is the Resend-verified sender,
  hardcoded or derived from the API key's domain

## Open Risks

- **Deliverability**: The Resend "from" address must be a verified domain.
  This requires DNS setup outside the codebase (SPF/DKIM) before the form
  goes live.
- **Spam volume**: With no storage, there's no signal to tune the rate limit
  against real abuse patterns. If spam volume is higher than expected, the
  tier constants can be tightened post-deploy.
- **Honeypot bypass**: A targeted attacker can trivially skip the honeypot
  field. The rate-limit tier is the real abuse defense; the honeypot is
  defense-in-depth against broad crawlers.
- **Resend API key scoping**: Resend API keys are full-access by default. If
  the key were leaked, an attacker could send arbitrary emails from the
  verified domain. Mitigation: restrict the key to the sending domain in the
  Resend dashboard.