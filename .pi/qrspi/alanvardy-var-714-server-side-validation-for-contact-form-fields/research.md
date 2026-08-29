# Research Findings

Branch: `alanvardy-var-714-server-side-validation-for-contact-form-fields`

## Q1: Request flow for a POST to `/contact`

### Findings

**Router + middleware layers**
- `main.rs:44-45` — production wiring: `interfaces::routes::routes().layer(app::log::trace_layer())` then `app::rate_limit::with_global_limit(router, rate_limit_per_ms, rate_limit_burst)`. So every request passes through the tracing `TraceLayer` (`src/app/log.rs:62`), then the global governor limiter.
- `routes.rs:33-38` — `POST /contact` is registered on a dedicated sub-router via `tiered_routes(...)` with `CONTACT_TIER_PER_MS = 1_000` (1/sec) and `CONTACT_TIER_BURST = 2` (`rate_limit.rs:77-78`), then merged into the main router. The POST route points to `handlers::contact::web::create` (`routes.rs:34-36`); GET `/contact` → `handlers::contact::web::index` (`routes.rs:26`).
- Rate limit layers are applied per-router: global in `main.rs:45`, contact tier in `routes.rs:33-38`; both attach `GovernorLayer::new(cfg).error_handler(rate_limit_error_response)` (`rate_limit.rs:113`, `rate_limit.rs:129`).

**Body extraction & deserialization**
- Handler signature: `create(State(state): State<AppState>, Form(form): Form<ContactForm>) -> Result<Html<String>, WebError>` (`contact/web.rs:34-37`). The `Form` extractor deserializes `application/x-www-form-urlencoded` into `ContactForm` defined at `src/app/contact.rs:4-13`: four fields — `name: String`, `email: String`, `message: String`, `_website: Option<String>` (honeypot). No validation exists on any field beyond the deserialization itself — plain `String`s, no `#[serde]` constraints.
- Extractor failure (missing field, wrong type) never reaches the handler: axum's built-in `Form` rejection handles it (see Q2).

**Checks before sending**
- Honeypot check only: `if form._website.is_some_and(|w| !w.trim().is_empty()) { return render(&state, true).await; }` (`contact/web.rs:40-41`) — a non-empty `_website` renders the thank-you page and sends nothing ("silently accept"). Comment at `contact/web.rs:38-39` documents that serde_urlencoded maps present-but-empty to `Some("")`, so only non-empty counts as bot.

**Email construction & delivery**
- Subject/text built inline: `format!("New contact message from {} <{}>", form.name, form.email)` and `Name:..\nEmail:..\n\n{message}` (`contact/web.rs:44-47`). Constants `FROM_EMAIL = "Contact Form <noreply@vardy.cc>"`, `TO_EMAIL = "alan@vardy.cc"` (`contact/web.rs:9-10`).
- `contact::send(&state, FROM_EMAIL, TO_EMAIL, &subject, &text).await?` (`contact/web.rs:49`) → `src/app/contact.rs:26-34` passes `state.http`, `state.resend_base_url`, `state.env.resend_api_key` to `resend::send_contact_email` (`src/infra/resend.rs:21-62`).
- Resend call details (`resend.rs:27-37`): `POST {base_url}/emails` with `bearer_auth(api_key)`, JSON payload `SendEmailRequest { from, to: [to; 1], subject, text }` (`resend.rs:6-11`). Non-2xx status → `Err(ResendError(...))` with the upstream body truncated to 500 chars (`resend.rs:38-50`); transport errors → `ResendError("resend request failed: ...")` (`resend.rs:33-35`). A 2xx succeeds without parsing the JSON body (`resend.rs:52-60`, comment: parsing would add a false-failure path).

**Responses per code path**
- Honeypot filled: 200 + `contact.html` rendered with `submitted=true` (`contact/web.rs:41`); no email sent.
- Valid form → email 2xx: 200 + thank-you render (`contact/web.rs:51`).
- Email failure: `?` converts `ResendError` → `WebError::External` → 502 `"bad gateway"` (`error.rs:56-59`; `From` at `error.rs:36-39`).
- Missing/invalid form fields: never reaches handler — axum `Form` rejection (default 400/415/422, see Q2).
- Rate limited at global or contact tier: 429 `"too many requests"` + `retry-after` (Q2).

## Q2: Error representation and HTTP mapping

### Findings

- **`WebError` enum** — `src/app/error.rs:10-16`: `Template(minijinja::Error)`, `Database(sqlx::Error)`, `NotFound`, `External(String)`, `TooManyRequests { retry_after_secs: u64 }`. `NotFound` is `#[allow(dead_code)]`, only constructed in unit tests (`error.rs:6-8`).
- **`From` impls** (`error.rs:18-40`): `minijinja::Error` → `Template` (:18-22); `sqlx::Error` → `Database` (:24-28); `UnsplashError` → `External(err.0)` (:30-33); `ResendError` → `External(err.0)` (:36-39). All handlers return `Result<_, WebError>` and use `?`.
- **`IntoResponse`** (`error.rs:42-68`): `NotFound` → 404 `"not found"` (:45); `Database` → 500 `"internal server error"` + `tracing::error!` + `sentry::capture_error` (:46-50); `Template` → 500 same (:51-55); `External` → 502 `"bad gateway"` + `tracing::error!`, **no Sentry** (:56-59); `TooManyRequests` → 429 `"too many requests"` + header `retry-after` (:61-66, tuple-with-headers response). Comment at `error.rs:60` groups 429 with 502 as "Client fault, like `External`: log nothing to Sentry."
- **Client-fault surface through the shared type**: exactly one production 4xx variant — `TooManyRequests` (429). `NotFound` (404) exists but is test-only. No 400-family variant exists in `WebError`.
- **Built-in axum rejections bypass `WebError`** — verified: no `FromRequest`/`FromRequestParts` impls, no `fallback`, no `HandleErrorLayer`, no `WithRejection` anywhere in `src/`. Only `TraceLayer` (`main.rs:44`) and `GovernorLayer`s (`rate_limit.rs:113,129`) wrap routers. So extractor failures use axum 0.8.9 defaults (from vendored `axum-0.8.9/src/extract/rejection.rs`): `JsonSyntaxError` → 400 "Failed to parse the request body as JSON" (:29-31); `JsonDataError` → 422 (:17-19); `MissingJsonContentType` → 415 (:39-41); `InvalidFormContentType` → 415 (:65-67); `FailedToDeserializeForm` → 400 "Failed to deserialize form" (:74-76); **`FailedToDeserializeFormBody` → 422 "Failed to deserialize form body" (:82-84)**. A bad contact POST therefore returns axum's default plain-text 422/400, not HTML, not `WebError`.
- **Rate limiter routing into the chokepoint** — `rate_limit_error_response` (`rate_limit.rs:43-70`): `GovernorError::TooManyRequests { wait_time, headers }` → `WebError::TooManyRequests { retry_after_secs: wait_time }.into_response()` (:46-49) then merges governor headers (e.g. `x-ratelimit-*` from `.use_headers()`, `rate_limit.rs:104`) (:50-55). Any other `GovernorError` (e.g. `UnableToExtractKey`) → locally built 500 tuple (:61-68) with comment "Unreachable with our extractor" (:59-60) — a second, parallel 500 path outside `WebError`.
- Key extractor: `FlyClientIpKeyExtractor` (`rate_limit.rs:19-39`) prefers `fly-client-ip` header, falls back to `ConnectInfo<SocketAddr>`, deliberately ignores `X-Forwarded-For` (spoofable, per comment :7-10).

## Q3: Template rendering and form-state patterns

### Findings

- **minijinja environment** — `src/app/templates.rs:5-24` `init()`: `Environment::new()` (:6), `path_loader("templates")` (:8), auto-escape callback matching on `.html` suffix (:9-13), `asset_url` global function (:14-19, used at `templates/layout.html:7`). Environment stored on `AppState.templates` (`state.rs:11`), constructed once at `main.rs:49`.
- **No `render()` helper lives in `templates.rs`** — the shared render helper for contact is handler-private: `contact/web.rs:17-27`:
  ```rust
  async fn render(state: &AppState, submitted: bool) -> Result<Html<String>, WebError> {
      let (wallpaper_url, photographer, photographer_url) = picture::wallpaper_context(state).await;
      let html = state.templates.get_template("contact.html")?
          .render(context! { wallpaper_url, photographer, photographer_url, submitted, active_page => "contact" })?;
      Ok(Html(html))
  }
  ```
  Wallpaper variables are decorative fallbacks — the doc comment (`web.rs:20-23`) says the page renders without them if Unsplash is unavailable.
- **`templates/contact.html` context**: receives `wallpaper_url`, `photographer`, `photographer_url`, `submitted: bool`, `active_page => "contact"`. Extends `layout.html` (`contact.html:1`), which requires the three wallpaper vars per its documented contract (`templates/layout.html:1-4`) and uses `active_page` for nav highlight (:20-24).
- **State handling**: single `{% if submitted %}` branch at `contact.html:19-20` (thank-you paragraph) vs `{% else %}` form (:21-40). Form fields: `name` (:24), `email` (:27), `message` textarea (:30), honeypot `_website` with static `value=""` (:36), submit button (:38-39). `{% endif %}` at :41.
- **No value preservation / no per-field error state anywhere**: grep shows the only `value=` in all templates is the honeypot's static `value=""` (`contact.html:36`); no `errors`/`error` variables exist in any template; the only `<form>` in the whole `templates/` dir is `contact.html:22`.
- **No handler re-renders a form with preserved input or error state.** Survey of all handlers:
  - `contact/web.rs:34-51` (POST /contact) is the **only** HTML form-submission handler. It re-renders the same template with `submitted=true` (fresh state; submitted values are dropped after `ContactForm` deserialization). Non-HTML fallbacks: 502 (`error.rs:56-59`), 429 (`error.rs:61-66`), and default axum rejections (Q2).
  - `dump/web.rs:18-26` POST returns `StatusCode::CREATED` with no body (JSON API, no template).
  - `home/web.rs`, `singlethread/web.rs:73`, `metrics/web.rs`, `unsplash/json.rs` are GET-only or return JSON.
  - GET /contact (`contact/web.rs:29-31`) calls `render(&state, false)` and increments `metrics.inc_page_view("contact")`; POST does not increment page views.

## Q4: Request body constraints and interfaces-layer dependency rules

### Findings

**Body-size limit**
- **No explicit body-limit configuration anywhere in `src/`**: no `DefaultBodyLimit`, no `RequestBodyLimitLayer` (tower-http), no `with_limited_body`, no `Bytes`/custom extractor. Verified by grep across `src/`.
- Router setup: only layers are `TraceLayer` (`main.rs:44`), global `GovernorLayer` (`main.rs:45`; `rate_limit.rs:113`), tier `GovernorLayer`s (`routes.rs:23-39`; `rate_limit.rs:129`), and `SetResponseHeader`/`ServeDir` for `/static` (`routes.rs:46-53`). None are body-limit related.
- Only Form extraction is `Form<ContactForm>` at `contact/web.rs:36`; dump POST uses `Json<serde_json::Value>` (`dump/web.rs:21`).
- **Effective limit = axum 0.8 default 2 MiB (2,097,152 bytes)**, from vendored `axum-core-0.5.6/src/ext_traits/request.rs:319` (`const DEFAULT_LIMIT: usize = 2_097_152; // 2 mb`), applied via `Limited::new(b, DEFAULT_LIMIT)` when no limit override is present (:325-326). axum docs (`axum-core-0.5.6/src/extract/default_body_limit.rs`) state this applies to `Bytes` and extractors using it internally such as `String`, `Json`, and `Form`. Over-limit bodies produce a length-limit rejection (413-family, `FailedToBufferBody`/`LengthLimitError`-based); untested in this repo.

**Architecture guard (`src/test/arkitect.rs`)**
- Allowed deps whitelist for `vardy::interfaces` (arkitect.rs:28-41): `axum`, `crate::app`, `crate::test`, `minijinja`, `serde_json`, `std`, `tower_http`, `vardy::app`, `vardy::domain`, `vardy::infra`, `vardy::test`, `sqlx` (last one "just for tests").
- Rule: `.rules_for_module("vardy::interfaces").it_may_depend_on(&interfaces_deps).and_it(Box::new(MustNotDependOnExceptTestsBuilder { forbidden: vec!["sqlx", "reqwest"] }))` (arkitect.rs:44-51).
- Net effect: interfaces may depend only on the whitelist; **`sqlx` and `reqwest` are forbidden outside `#[cfg(test)]` modules** (the custom builder filters deps outside test modules — `deps_outside_test_modules` collector at arkitect.rs ~127-187, AST visitor records `inside_test`). `reqwest` is additionally absent from the whitelist. `serde` is also excluded from the whitelist (corroborated by comment in `singlethread/web.rs:67` that the layer marshals via `serde_json`).
- Guard runs as a `#[cfg(test)]` test (`test_architectural_rules`, arkitect.rs:22-56) via `Arkitect::ensure_that(project).complies_with(rules)`; fails the build listing violations.
- Note: `sqlx` in the whitelist + forbidden-in-production is resolved by the whitelist applying to test-only usage (e.g. `#[sqlx::test]` / `SqlitePool` in test harness usage).

## Q5: How form-POST handlers are tested

### Findings

**Test harness (`src/test/mod.rs`)**
- `start_app()` → `SocketAddr`, delegates to `start_app_with("https://api.unsplash.com").await.0` (:20-23). Variants, all funneling through private `serve_app` (:56-107): `start_app_with` (:26-29, returns `(SocketAddr, SqlitePool)`), `start_app_with_rate_limits` (:32-37), `start_app_with_resend` (:42-45), `start_app_with_resend_and_rate_limits` (:48-53), `start_app_with_metrics` (:108-159, extra metrics port).
- `serve_app` (:56-107): hardcoded `Env` with `resend_api_key: "test-key"`, `sqlite::memory:`, `enable_sentry: false` (:63-70); runs `sqlx::migrate!` (:71-75) and `seed_wallpaper` (:76); builds `AppState` (:78-85); binds `127.0.0.1:0` (:86-88); wraps routes with `with_global_limit(..., per_ms, burst)` (:90-95); spawns `axum::serve` (:96-105).
- `test_client()` → plain `reqwest::Client::new()` (:161-165), no special config.
- Seed helpers: `seed_wallpaper` (:167-177), `seed_wallpaper_no_url` (:179-186), `UnsplashStub` (:189-233).
- **Resend stub** — `ResendStub { base_url, call_count }` (:236-238); `start_resend_stub(status: StatusCode)` (:243-272) spawns a stub `POST /emails` returning canned `{"id":"email_test"}` on success or the given status verbatim. **Records only `call_count` (`AtomicUsize`) — the stub handler takes no request arguments (:249-252) and never reads the request body.** The outgoing email's subject/text/from/to are unobservable in tests (the body content of the Resend call is discarded; only its count and the HTTP response are asserted).

**Contact tests (`src/interfaces/handlers/contact/web.rs`, `#[cfg(test)] mod tests` at :55)**
- `get_contact_returns_200_with_form` (:64-89): GET, asserts 200, `text/html`, and presence of the four named inputs, `action="/contact"`, copy, nav chrome.
- `post_valid_form_sends_email` (:92-105): POST with `.form(&[("name","Alan"),("email","a@b.cc"),("message","hi")])` against `start_app_with_resend(&stub.base_url)`; asserts 200, `stub.call_count == 1`, body contains "Thank you — I'll get back to you soon."
- `post_honeypot_filled_skips_email` (:108-124): same + `("_website","http://spam")`; asserts 200 and `call_count == 0`.
- `post_resend_failure_returns_502` (:127-138): stub spawned with `INTERNAL_SERVER_ERROR`; asserts `BAD_GATEWAY` and exact body `"bad gateway"`.
- `post_too_many_requests_returns_429` (:141-157): loose harness limits `1, 1_000_000`; 10 rapid POSTs; asserts at least one 429 with `retry-after` and body `"too many requests"`, others must be 200. Comment cites `CONTACT_TIER_BURST = 2` (`rate_limit.rs:78`).
- Tests use reqwest `.form(...)` producing `application/x-www-form-urlencoded`, matching axum's `Form` extractor (`web.rs:36`). **No test posts a malformed/missing-field form** — the axum-default 400/422 rejection path on `/contact` is untested.

**POST precedent (`src/interfaces/handlers/dump/web.rs`)**
- `create` extracts `Path<String>` + `Json<serde_json::Value>` (:18-26). Tests at :28-171: `dump_post_tier_trips_while_global_budget_stays_open` (:34-69, 15 spawned POSTs, counts CREATED vs 429, cites `DUMP_TIER_BURST = 3`); `dump_get_is_not_tier_limited` (:72-85); `get_unknown_key_returns_empty_list` (:88-104); `post_stores_and_get_returns_it` (:107-129, POST then GET, deserializes JSON body, field-by-field compare); `multiple_posts_accumulate` (:132-157); `post_invalid_json_rejected` (:160-170, raw body `"{not json"` with json content-type → asserts `BAD_REQUEST`, cites axum 0.8 `JsonSyntaxError`).
- Precedent pattern: `.json(&serde_json::json!(...))` for POST bodies, status + deserialized-body round-trip assertions, stress-loop for rate tiers that asserts both sides of the boundary.

**Gate (`scripts/test.sh`, 22 lines)** — all `&&`-chained, aborts on failure:
1. `set -a; source .env; set +a` (:3 — requires `.env` with `DATABASE_URL`)
2. `cargo fmt --all` (:6)
3. `cargo sqlx prepare -- --tests` (:8 — refreshes sqlx offline metadata)
4. `cargo check --all-targets` (:10)
5. `./scripts/build-css.sh && git diff --exit-code -- static/site.css` (:12-13 — CSS drift check)
6. `cargo clippy --all-targets --all-features --locked -- -D warnings` (:15)
7. `cargo nextest run` (:17 — nextest, not plain cargo test)
8. `! rg -i -s -g '*.rs' 'FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO\s|todo\s' src` (:20 — inverted rg for forgotten TODOs)
9. `echo "🎉  SUCCESS"` (:22)

## Cross-Cutting Observations

- **Single error chokepoint**: every handler returns `Result<_, WebError>` and every error response (handler errors, governor 429s) flows through `IntoResponse for WebError` (`error.rs:42-68`); extractor rejections are the sole bypass, using axum defaults. This matches the home-directory policy of a central error-response chokepoint.
- **Layered middleware**: global governor (all routes) → tier governor (per route group `routes.rs:23-39`) → handler. Contact POST sits behind two rate limiters plus tracing.
- **Render pattern**: rendering is done inline in handlers via `state.templates.get_template(...)?.render(context! {...})` — no shared engine-level helper; the contact `render()` helper is handler-local and passes a boolean `submitted` flag. This is the only form-state mechanism in the app.
- **Honeypot-only validation**: the contact handler's only input check is the `_website` honeypot; `name`/`email`/`message` are unvalidated `String`s (client-side `required`/type attrs in `contact.html:24-30` are the only enforcement).
- **Tier constants live in code** (`rate_limit.rs:73-78`), referenced by route wiring and test comments.
- **Interfaces layer is thin and constrained**: whitelist deps only; DB access happens via `crate::app`; outgoing calls via `crate::infra`.

## Open Areas

- Exact status/body text of axum `Form` rejection on `/contact` is inferred from vendored axum 0.8.9 source (paths under `~/.cargo/registry/src/.../axum-0.8.9/`), not from any repo test — no test exercises missing-field or malformed-field POSTs to `/contact`.
- Behavior of a >2 MiB POST body on `/contact` (413 path) is untested in the repo; limit is the axum default since no `DefaultBodyLimit` config exists.
- The outgoing Resend request body (subject/text/from/to) is not asserted anywhere in tests — only call count and HTTP responses are observable.
- `NotFound` (404) `WebError` variant is dead code in production (`error.rs:6-8`) — no production handler constructs it.
- The `MustNotDependOnExceptTestsBuilder` internals (arkitect.rs:93+) were not reviewed line-by-line; the forbidden list and test-module exception were verified at the call site (arkitect.rs:46-51).