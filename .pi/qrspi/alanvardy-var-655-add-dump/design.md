# Design Discussion

## Current State
Small axum 0.8 service (`vardy`), sqlx/SQLite + minijinja, verified against working tree:

- Router factory `pub fn routes() -> Router<AppState>` registers exactly three
  routes (`src/interfaces/routes.rs:7-13`): `/`, `/singlethread`, and nested
  `/static` ServeDir. State attached in `main.rs:17`.
- Handler areas are two files each: `handlers/<area>/mod.rs` = `pub mod web;`
  and `web.rs` holding one `pub async fn index(State(state): State<AppState>) ->
  Result<Html<String>, WebError>` (`home/web.rs:7`, `singlethread/web.rs:7`).
  Wiring a new area touches exactly two points: `handlers/mod.rs:1-2` and
  `routes.rs:7-13`. No `pub use` re-exports anywhere.
- `AppState { templates, db }` (`state.rs:2-9`) — `db: SqlitePool` carries
  `#[allow(dead_code)]`; **zero production SQL exists today**.
- Pool factory `app::db::init` (`db.rs:7-26`): WAL, foreign keys,
  `create_if_missing`, max 5 conns. Tests bootstrap an identical AppState with
  `sqlite::memory:` (`test/mod.rs:5-24`) and exercise real HTTP via reqwest.
- One placeholder migration `migrations/0001_placeholder.sql`; migrations are
  applied by `#[sqlx::test]` (auto), Docker build (`Dockerfile:28-29`), and
  manual `sqlx migrate run` (README:9-10). **The binary never migrates.**
- Only extractors/responses in use: `State` + `Html<String>`. No `Path`, no
  JSON anywhere. `WebError` (`error.rs:8-41`) maps `NotFound` → 404 text and
  `Database`/`Template` → 500 text via `IntoResponse`; `From<sqlx::Error>`
  enables bare `?`.
- No serde/serde_json deps; axum default features (incl. `json`) implicitly on.
  Dev-dep reqwest has `json` feature (`Cargo.toml:13`). `SQLX_OFFLINE=true` in
  Docker but no `.sqlx/` directory is committed yet.

## Desired End State
Two new routes on the existing router:
- `POST /dump/<key>` — accepts a JSON body, stores it as a blob row in a new
  `dumps` table keyed by `<key>`, returns success.
- `GET /dump/<key>` — returns all stored dumps for `<key>` as JSON:
  `[{"id": 1, "body": {...}}, ...]`.

Verification:
- Integration tests over real HTTP (existing harness): POST arbitrary JSON →
  2xx; GET returns the stored body under its id; multiple POSTs accumulate;
  GET of unknown key returns `[]` with 200.
- `cargo nextest run`, clippy `-D warnings` green in CI.

## Patterns to Follow
- **Handler area layout**: two-file area `dump/mod.rs` + `dump/web.rs`,
  register `pub mod dump;` in `handlers/mod.rs` and routes in `routes.rs`
  (pattern: `handlers/home/*`, `routes.rs:9-10`).
- **Error propagation**: bare `?` through `From<sqlx::Error>` into `WebError`
  (`error.rs:24-30`); handlers return `Result<_, WebError>`.
- **Test shape**: inline `#[cfg(test)] mod tests` at bottom of `web.rs`, using
  `crate::test::{start_app, test_client}`, real HTTP, status/content-type/body
  assertions (`home/web.rs:15-38`).
- **DB access**: pool extracted via `State(state)`; queries against
  `state.db`.
- **NOT to follow**: plain-text error bodies and `Html` responses are legacy
  HTML-era patterns — dump endpoints return JSON, not rendered templates.
  Don't copy `#[allow(dead_code)]` onto anything newly written.

## Design Decisions
1. **JSON body handling**: add `serde_json` dependency; extract with
   `Json<serde_json::Value>` (Q1=A). Axum's default features already include
   `json`, so only serde_json needs adding. Invalid JSON gets axum's automatic
   422 rejection without custom code. Body stored as `TEXT` via
   `serde_json::to_string`.
2. **GET response shape**: `[{"id": ..., "body": <original JSON>}, ...]`
   (Q2=A). Body round-trips through `serde_json::from_str`.
3. **Unknown key on GET**: return `[]` with 200 (Q3=A). Simplest client
   contract; avoids inventing key-lifecycle semantics.
4. **Migration creation**: new migration created with
   `sqlx migrate add create_dumps` (per repo convention); contains
   `CREATE TABLE IF NOT EXISTS dumps (id INTEGER PRIMARY KEY AUTOINCREMENT,
   key TEXT NOT NULL, body TEXT NOT NULL)` plus an index on `key`.
   Application stays with existing mechanisms (Q4): tests get it via the test
   harness (see 4a), prod via Docker/manual `sqlx migrate run`.
   - 4a. Because the shared HTTP harness uses `sqlite::memory:` and does not
     auto-migrate, `start_app()` will run `sqlx::migrate!("./migrations")`
     against the pool before serving. Production startup stays untouched.
5. **SQL style**: compile-time `query_as!`/`query!` macros (Q5=B) for
   type-safe, checked queries. This **requires committing `.sqlx/` metadata**:
   run `cargo sqlx prepare` with `DATABASE_URL` set and commit the `.sqlx/`
   directory so `SQLX_OFFLINE=true` Docker builds keep compiling. Document in
   README alongside existing notes.
6. **Route syntax**: axum 0.8 path params use `{key}` syntax
   (`.route("/dump/{key}", ...)` + `Path<String>` extractor) — first use in
   codebase, establishes the precedent.

## What We're NOT Doing
- No auth, rate limiting, or size limits on dumped payloads.
- No DELETE/update endpoints; dumps are append-only.
- No pagination or ordering guarantees beyond insertion order.
- No JSON error-response refactor of `WebError` for existing HTML routes
  (plain-text errors stay for them).
- No startup migrations in production `main.rs`.
- No template/static changes; no new dependencies beyond `serde_json`
  (+ `sqlx-cli` already present for prepare).

## Open Risks
- `.sqlx/` metadata must actually be generated and committed before CI/Docker;
  forgetting this breaks offline builds (first-time setup for this repo).
- `Json<Value>` rejection returns axum's default 422 text body — acceptable
  but differs from our JSON success shape; noted rather than solved.
- Large payloads unbounded (SQLite TEXT is fine, memory isn't) — flagged as a
  future concern, out of scope now.
- First `Path`/`Json` usage may surface axum extractor-ordering gotchas (e.g.
  `Path` before `State` is fine; consuming extractors must come last) — low
  risk, well-documented behavior.
