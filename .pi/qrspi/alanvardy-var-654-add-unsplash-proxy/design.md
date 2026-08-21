# Design Discussion — VAR-654 Unsplash Proxy

## Current State

- Minimal axum app: `Router<AppState>` built in `src/interfaces/routes.rs:7-13`
  (`/`, `/singlethread`, `/health`, `/static`), state attached in
  `src/main.rs:19-21`.
- `AppState { templates, db }` (`src/app/state.rs:2-9`) — the `db` pool is
  plumbed but still `#[allow(dead_code)]`; **this ticket is the first
  data-backed handler and sets the query-organization precedent**.
- Handlers return `Result<Html<String>, WebError>` with empty contexts
  (`src/interfaces/handlers/home/web.rs:6-13`). `WebError` has only
  `Template`/`Database`/`NotFound` variants (`src/app/error.rs:9-13`) with
  `From<minijinja::Error>` / `From<sqlx::Error>` impls.
- No outbound HTTP client or serde/JSON anywhere in prod deps
  (`Cargo.toml:5-12`); `reqwest` exists as dev-dep only (`Cargo.toml:14`).
- Only env var is `DATABASE_URL`, read inline with a hardcoded default
  (`src/main.rs:5-6`). `.env_template` contains one line.
- One placeholder migration; migrations applied at Docker build time only,
  auto-applied in tests via `#[sqlx::test]` (`src/app/db.rs:39-48`). Nothing
  runs them at app start.
- Reference patterns live in the sibling repo `../api`: secrets via a
  dedicated `Env` struct that panics on missing vars
  (`../api/src/app/env.rs:8-45`, `get_string_env` at `env.rs:117-122`);
  secrets reach handlers as fields on `AppState`
  (`../api/src/app/state.rs:9-16`); outbound calls take `&Client` params and
  map parse failures to an error variant (`../api/src/app/apple_auth.rs:28-53`).

## Desired End State

- `GET /unsplash` returns JSON `{ "url": "...", "photographer": "...",
  "created_at": "..." }` for the current stored picture.
- On each request: query newest row from `unsplash_pictures`; if none exists
  or it is older than 6 hours, call the Unsplash API
  (`https://api.unsplash.com/photos/random?query=nature` with
  `Authorization: Client-ID <key>`), insert a new row, return it. Otherwise
  return the cached row.
- `UNSPLASH_API_KEY` loaded at startup via a new env module; startup panics
  if missing.
- New migration creates `unsplash_pictures` (id, url, photographer,
  created_at). Local devs must run `sqlx migrate run` once; Docker picks it
  up automatically at image build.

**Verification**: integration tests following the existing handler-test
pattern — status code + body assertions against a spawned test app; DB tests
via `#[sqlx::test]` asserting table existence and freshness logic.

## Patterns to Follow

- **Handler layout**: feature folder with `mod.rs` re-exporting `web.rs`
  (`src/interfaces/handlers/home/mod.rs:1`); register route in
  `src/interfaces/routes.rs:7-13`. New: `handlers/unsplash/{mod.rs,web.rs}`.
- **Error handling**: handlers return `Result<T, WebError>` relying on `?`
  and `From` conversions (`src/app/error.rs:15-23`). Add an `External`
  (or `BadGateway`, matching `../api/src/app/apple_auth.rs`) variant for
  upstream Unsplash failures → 502, with `eprintln!` logging like existing
  variants (`src/app/error.rs:25-41`).
- **Secrets**: mirror `../api/src/app/env.rs` — small `Env::init()` called
  first thing in `main.rs` (as in `../api/src/main.rs:23`), panicking on
  empty/missing vars; store key on `AppState`. Update `.env_template` and
  set via `fly secrets set` (workflow documented at `../api/src/app/env.rs:1-3`).
- **Outbound HTTP**: promote `reqwest = { version = "0.13", features =
  ["json"] }` to prod deps and add `serde` + `serde_json` — exactly the
  `../api` stack (`../api/Cargo.toml:20-21,32`). Pass `reqwest::Client` as a
  parameter to the fetch function, constructed by the caller
  (`../api/src/app/users.rs:160-167`).
- **DB access**: keep queries inline in the handler module for now (no query
  layer exists yet — this file establishes the convention; revisit if it
  grows beyond ~3 queries).
- **Testing**: spawn-app integration tests via `start_app()` /
  `test_client()` (`src/test/mod.rs:7-28`), assertions on status +
  body substring (`src/interfaces/routes.rs:18-31`); `#[sqlx::test]` for
  migration/table checks (`src/app/db.rs:39-48`).

### Patterns NOT to follow

- Do **not** copy the hardcoded-default style of `DATABASE_URL`
  (`src/main.rs:5-6`) — secrets must fail fast, per the `../api` pattern.
- Do **not** cache in-process with `OnceLock<Mutex<...>>`
  (`../api/src/apple_auth.rs:20-40`) — we have SQLite now; the DB *is* the
  cache and survives restarts.
- Do **not** add runtime migrations in this ticket — Docker build-time +
  `#[sqlx::test]` coverage suffices for one table.

## Design Decisions

1. **Storage: metadata only** — rows hold the Unsplash URL + credit, never
   image bytes. Browser fetches from Unsplash CDN; keeps SQLite tiny and
   avoids fly.io ephemeral-disk issues. (Rejected: blobs, writing to
   `static/`.)
2. **Response format: JSON** — plain JSON body, no template. First JSON
   endpoint in the repo; needs serde_json in prod deps but no minijinja
   involvement. Content-type asserted in tests.
3. **Refresh model: lazy on request** — handler checks row age (> 6h) and
   refreshes synchronously when stale. No background task; worst case one
   caller pays ~a few hundred ms. Acceptable for a personal site.
4. **Config: `Env` struct pattern from `../api`** — new `src/app/env.rs`,
   panic-fast on missing `UNSPLASH_API_KEY`, value stored on `AppState`.
5. **Dependencies: reqwest + serde/serde_json promoted to prod** — matches
   `../api` versions/features exactly; no lighter client considered.
6. **Upstream failure handling**: new `WebError` variant mapping to HTTP 502;
   do not serve stale rows as fallback (simplicity over availability for
   this use case).

## What We're NOT Doing

- No background refresh task, scheduler, or cron.
- No downloading/caching of image binaries locally.
- No rate limiting, retries, or circuit breaking around the Unsplash call.
- No runtime migration execution change.
- No config crate/dotenv loader beyond the minimal `env.rs` module.
- No changes to existing routes, templates, or static assets (nav link to
  `/unsplash` in `layout.html` is out of scope unless requested).

## Open Risks

- **Unsplash API contract**: exact response shape (`urls.regular`,
  `user.name`) assumed from API docs; verify against a live call during
  implementation. Tests will need a mock/stub strategy since CI can't hold a
  real API key — likely inject a base URL into AppState or gate live tests.
- **Clock semantics**: `created_at` staleness comparison relies on SQLite
  datetime functions vs Rust-side time — pick one side consistently to avoid
  timezone drift.
- **First JSON endpoint**: if axum's `Json` extractor/response ergonomics
  clash with the `Result<_, WebError>` pattern, the error type may need a
  small extension (e.g. `IntoResponse` for `(StatusCode, Json)`).
