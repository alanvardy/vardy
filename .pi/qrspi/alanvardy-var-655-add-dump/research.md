# Research Findings

Branch: `alanvardy-var-655-add-dump`. Small axum 0.8 service (`vardy`), sqlx/SQLite + minijinja. All claims verified against the working tree.

## Q1: HTTP request flow — router construction, registration, handler organization

### Findings
- Entry point `src/main.rs:4` (`#[tokio::main]`) declares only two top-level modules: `mod app; mod interfaces;` (`src/main.rs:1-2`), plus `#[cfg(test)] mod test;` at `src/main.rs:23`.
- Startup (`src/main.rs:6-12`): reads `DATABASE_URL` env var with default `"sqlite:data/vardy.db"` (`src/main.rs:7-8`); builds `AppState { templates: app::templates::init(), db: app::db::init(&database_url).await }`; binds `tokio::net::TcpListener` on `"0.0.0.0:3000"` (`src/main.rs:13`).
- Serving (`src/main.rs:15-22`): `axum::serve(listener, interfaces::routes::routes().with_state(state).into_make_service_with_connect_info::<std::net::SocketAddr>())`. Router construction is fully delegated to `interfaces::routes::routes()`; state injected via `.with_state`; connect info exposed.
- Router factory: `pub fn routes() -> Router<AppState>` at `src/interfaces/routes.rs:7-13`:
  - `.route("/", get(handlers::home::web::index))` (`routes.rs:9`)
  - `.route("/singlethread", get(handlers::singlethread::web::index))` (`routes.rs:10`)
  - `.nest_service("/static", ServeDir::new("static"))` (`routes.rs:11`, tower-http `ServeDir`)
  - These are the only three registrations in the codebase. Return type is generic `Router<AppState>` so `.with_state()` happens in main.
- Handler module chain (plain `pub mod`, **no `pub use` re-exports anywhere**):
  - `src/interfaces/mod.rs:1-2`: `pub mod handlers; pub mod routes;`
  - `src/interfaces/handlers/mod.rs:1-2`: `pub mod home; pub mod singlethread;`
  - Each area dir has a one-line `mod.rs`: `pub mod web;` (`handlers/home/mod.rs:1`, `handlers/singlethread/mod.rs:1`)
- Handler signature convention (identical in both areas):
  ```rust
  pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError>
  ```
  (`src/interfaces/handlers/home/web.rs:7`, `src/interfaces/handlers/singlethread/web.rs:7`)
- Body pattern: `state.templates.get_template("<area>.html")?.render(context! {})?` wrapped in `Ok(Html(html))` (`home/web.rs:9-12`; `singlethread/web.rs:9-12`). Errors propagate via `?` through `From<minijinja::Error>` into `WebError`.
- Full request path: TCP :3000 → `TcpListener` → `axum::serve` with stateful router → GET dispatch → handler renders minijinja template against `AppState.templates` → `200 text/html` via `Html<String>`, or `Err(WebError)` → `IntoResponse` (`src/app/error.rs:27-41`) for 404/500.

## Q2: SQLite initialization, AppState, pool threading, query style

### Findings
- Dependency: `sqlx = { version = "0.9.0", features = ["sqlite", "runtime-tokio", "chrono", "migrate"] }` (`Cargo.toml:10`).
- Sole pool factory: `pub async fn init(database_url: &str) -> SqlitePool` at `src/app/db.rs:7-26`:
  - `SqliteConnectOptions::from_str(url).create_if_missing(true).foreign_keys(true).journal_mode(SqliteJournalMode::Wal)` (`db.rs:8-12`)
  - Creates parent directory with `std::fs::create_dir_all` before connecting, excluding `:memory:` (`db.rs:14-21`)
  - `SqlitePoolOptions::new().max_connections(5).connect_with(options)` (`db.rs:22-25`)
- `AppState` full definition (`src/app/state.rs:2-9`):
  ```rust
  #[derive(Clone)]
  pub struct AppState {
      pub templates: minijinja::Environment<'static>,
      #[allow(dead_code)]
      pub db: sqlx::SqlitePool,
  }
  ```
  `db` carries `#[allow(dead_code)]` — no production code queries it yet.
- Pool threading: created once in `main` (`main.rs:8-11`) → attached via `.with_state(state)` (`main.rs:17`) → extracted per-request as `State(state): State<AppState>` in handlers (`home/web.rs:7`, `singlethread/web.rs:7`). Handlers currently consume only `state.templates`.
- Test bootstrap mirrors production: `AppState` built with `db: crate::app::db::init("sqlite::memory:")` and same `routes().with_state(state)` (`src/test/mod.rs:7-14`).
- Query style: the **only** SQL in the repo is runtime-checked non-macro `sqlx::query("...")` in one test (`src/app/db.rs:37-40`, querying `sqlite_master`). No `query!`/`query_as!` macros, no `query_as`, and zero production SQL anywhere yet.

## Q3: Migrations

### Findings
- Exactly one migration: `migrations/0001_placeholder.sql:1-3` — `CREATE TABLE IF NOT EXISTS placeholder (id INTEGER PRIMARY KEY AUTOINCREMENT);`
- **Production `main.rs` does NOT run migrations.** No `sqlx::migrate!()`, `Migrator`, or migrate call exists in `src/`. `app::db::init` only creates file/dir and connects (`src/app/db.rs:7-26`).
- Migrations are applied outside the binary:
  1. **Tests**: `#[sqlx::test]` auto-applies crate-root `migrations/` to its per-test DB — see `migrations_applied` test asserting the `placeholder` table exists (`src/app/db.rs:35-43`).
  2. **Docker build**: `sqlx database create` then `sqlx migrate run` during image build (`Dockerfile:28-29`; migrations copied at `Dockerfile:22`; `sqlx-cli --features sqlite` installed at `Dockerfile:9`).
  3. **Manual local dev**: `sqlx migrate run` per `README.md:9-10`.
- The integration test harness (`src/test/mod.rs:8`, `sqlite::memory:`) does **not** apply migrations either — only `#[sqlx::test]`-annotated tests get them.
- Build-time settings: `migrate` feature enabled (`Cargo.toml:10`); `SQLX_OFFLINE=true` set in Docker build (`Dockerfile:15`) but no `.sqlx/` metadata directory is committed (harmless today since no compile-time macros are used; policy documented in `README.md:13-15`). `.env_template:1` shows `DATABASE_URL=sqlite:data/vardy.db`; no `.env` committed.

## Q4: Extraction/response patterns and WebError

### Findings
- Extractors in use: **only `State`** (`home/web.rs:7`, `singlethread/web.rs:7`). No `Path<T>`, no `Json<T>`, no `Form<T>` anywhere in `src/`.
- Responses in use: `Result<Html<String>, WebError>` rendering minijinja templates. No JSON responses exist.
- Dependencies relevant to extraction/response (`Cargo.toml:5-13`):
  - `axum = "0.8.9"` declared **without** `default-features = false`, so default features (including `json`, `form`, `query`) are implicitly enabled even though unused.
  - **No `serde` or `serde_json` direct dependencies** exist; grep for `serde` across all `.rs` files returns zero matches. Only serde-related entry is the dev-dep `reqwest = { version = "0.13", features = ["json"] }` (`Cargo.toml:13`).
  - `tower-http = { version = "0.6", features = ["fs"] }` for `ServeDir`.
- Template environment: `minijinja::path_loader("templates")` with HTML auto-escape for names ending `.html`, else none (`src/app/templates.rs:3-9`).
- `WebError` full picture (`src/app/error.rs`):
  - Enum (`error.rs:8-14`, with `#[allow(dead_code)]` on the enum): `Template(minijinja::Error)`, `Database(sqlx::Error)`, `NotFound` (NotFound constructed only from tests).
  - Conversions: `From<minijinja::Error>` (`error.rs:16-22`), `From<sqlx::Error>` (`error.rs:24-30`) — enables bare `?` in handlers.
  - `IntoResponse` (`error.rs:32-41`): `NotFound` → `(StatusCode::NOT_FOUND, "not found")`; `Database(err)` and `Template(err)` → `eprintln!` log + `(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")`. All bodies are plain text; no JSON error payloads.
  - Unit tests cover 404/500 mappings and both `From` impls (`error.rs:43-70`).

## Q5: Test structure

### Findings
- No top-level `tests/` integration directory. All tests are inline `#[cfg(test)]` modules plus a shared harness:
  - Harness `src/test/mod.rs`, wired via `#[cfg(test)] mod test;` (`src/main.rs:23`).
    - `start_app()` (`test/mod.rs:5-20`): builds real `AppState` with `sqlite::memory:` pool, binds `127.0.0.1:0` (random port), spawns `axum::serve(listener, routes().with_state(state).into_make_service())` via `tokio::spawn`, returns `SocketAddr`.
    - `test_client()` (`test/mod.rs:22-24`): returns `reqwest::Client::new()`.
  - Tests exercise the app over **real HTTP** — no `tower::ServiceExt::oneshot` anywhere in `src/`.
  - Note: harness uses plain `.into_make_service()` while production uses `.into_make_service_with_connect_info::<SocketAddr>()` (`main.rs:19` vs `test/mod.rs:16`).
- Existing HTTP tests (all follow identical shape): `routes.rs:14-35` (static icon), `home/web.rs:15-38`, `singlethread/web.rs:15-37`:
  - `let addr = start_app().await; let client = test_client();`
  - `client.get(format!("http://{addr}/")).send().await.expect("request failed")`
  - Assert status: `assert_eq!(res.status(), StatusCode::OK)`
  - Assert header: `res.headers().get("content-type").is_some_and(|v| v.to_str().unwrap().contains("text/html"))` (or `"image/png"`)
  - Assert body: `res.text().await.unwrap()` + `body.contains(...)` string checks
- Non-HTTP tests: error mapping unit tests (`error.rs:43-70`), template render tests (`templates.rs`), `#[sqlx::test] migrations_applied` (`db.rs:35-43`), `#[tokio::test]` verifying `db::init` creates file/dir (`db.rs:46-55`).
- reqwest capabilities: dev-dep `reqwest = { version = "0.13", features = ["json"] }` (`Cargo.toml:13`) — `.json(&T)` request bodies and `.json::<T>()` response deserialization are **available** but unused by current tests (all use `.get()` + `.text()`).
- CI: `cargo nextest run --profile ci` on PRs, `cargo llvm-cov nextest` on main, clippy `-D warnings` (`.github/workflows/ci.yml:62,67,126`).

## Q6: Naming / layout / module-wiring conventions

### Findings
- Top level: private `mod app; mod interfaces;` (`main.rs:1-2`); layers addressed by full paths, no re-export aliases.
- A handler area `<area>` consists of exactly two files:
  - `src/interfaces/handlers/<area>/mod.rs` containing only `pub mod web;`
  - `src/interfaces/handlers/<area>/web.rs` containing the handler(s)
- Registering a new area requires touching exactly two wiring points:
  1. Add `pub mod <area>;` to `src/interfaces/handlers/mod.rs:1-2`
  2. Add `.route("/<path>", get(handlers::<area>::web::<fn>))` in `src/interfaces/routes.rs:7-13`
- Handler function conventions: single `pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError>`; imports ordered `axum::{extract::State, response::Html}`, `minijinja::context`, then `crate::app::error::WebError`, `crate::app::state::AppState` (`home/web.rs:1-5`); renders `"<area>.html"` with empty context.
- Inline `#[cfg(test)] mod tests` at the bottom of each `web.rs` and `routes.rs`, using `crate::test::{start_app, test_client}` helpers and `#[tokio::test] async fn index_serves_ok_html()`.
- Templates live flat in repo-root `templates/` (`home.html`, `layout.html`, `singlethread.html`), loaded by name via `path_loader` and extendable from `layout.html`. Static assets in repo-root `static/`, served at `/static`.

## Cross-Cutting Observations
- Production (`main.rs:15-22`) and test harness (`test/mod.rs:5-20`) construct the identical `AppState` + `routes().with_state(...)`; only differences are bind address, `connect_info` wrapper, and DB URL.
- Dead-code precedents: `#[allow(dead_code)]` on `AppState.db` (`state.rs:5-7`) and on `WebError` enum (`error.rs:7`) — patterns for fields/variants not yet used by production code.
- The service currently has zero production SQL and zero JSON handling; every existing pattern is server-rendered HTML with plain-text errors.
- axum default features are implicitly on (no `default-features = false`), so `Json` extractor/response types are available without Cargo changes — but `serde`/`serde_json` would be needed for concrete payload types.
- Migrations are applied everywhere *except* the binary itself (tests via `#[sqlx::test]`, Docker build, manual CLI).

## Open Areas
- No precedent exists for: path parameters (`Path`), JSON request/response bodies, POST/PUT/DELETE routes, form submission, or multi-file handler areas (both existing areas have a single `index` handler in `web.rs`).
- Whether new endpoints should run migrations at startup cannot be answered from the code — current startup never does; behavior must follow Docker/manual CLI conventions described above.
- Only two handler-area samples exist; conventions for areas needing more than one handler function or sub-route are not observable.
