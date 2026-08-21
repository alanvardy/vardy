# Research Findings

## Q1: Route registration and handler organization

### Findings
- Single `routes()` builder: `src/interfaces/routes.rs:7-13`. Routes: `/` →
  `handlers::home::web::index`, `/singlethread` →
  `handlers::singlethread::web::index`, `/health` → inline closure returning
  `StatusCode::OK`, `/static` → `tower_http::services::ServeDir::new("static")`.
- `Router<AppState>` is built stateless in `routes()`; state attached via
  `.with_state(state)` in `src/main.rs:19-21`, served with
  `into_make_service_with_connect_info::<SocketAddr>()` (`src/main.rs:22`).
- Module wiring: `src/interfaces/mod.rs:1-2` declares `pub mod handlers; pub
  mod routes;`. `src/interfaces/handlers/mod.rs:1-2` declares `pub mod home;
  pub mod singlethread;`. Each feature folder has `mod.rs` (re-exporting
  `web`) + `web.rs` (the actual handler) — e.g. `src/interfaces/handlers/home/mod.rs:1`.
- Handler shape (both handlers identical):
  `pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError>`
  (`src/interfaces/handlers/home/web.rs:6-13`, `src/interfaces/handlers/singlethread/web.rs:6-13`).
  Extract state via axum `State` extractor, render template, return `Html`.
- Error type: `WebError` enum (`src/app/error.rs:9-13`) with `Template`,
  `Database`, `NotFound` variants; `From` impls for `minijinja::Error` and
  `sqlx::Error` (`src/app/error.rs:15-23`); `IntoResponse` maps
  NotFound→404, Database/Template→500 with `eprintln!` logging
  (`src/app/error.rs:25-41`).
- App state: `AppState { templates: minijinja::Environment<'static>, db:
  sqlx::SqlitePool }` (`src/app/state.rs:2-9`); `Clone`; `db` currently
  `#[allow(dead_code)]` with a comment noting it is "unused until the first
  handler query lands".

## Q2: SQLite database access

### Findings
- Pool creation: `app::db::init(database_url)` (`src/app/db.rs:6-30`) —
  `SqliteConnectOptions::from_str` with `create_if_missing(true)`,
  `foreign_keys(true)`, `journal_mode(Wal)`; creates parent directories for
  file DBs (skips `:memory:`); `SqlitePoolOptions` with `max_connections(5)`;
  panics via `.expect` on failure.
- Query style: no application queries exist yet. Only test queries using
  `sqlx::query(...).fetch_one(&pool)` (`src/app/db.rs:41-46`). There is no
  query module/organization pattern established.
- Migrations live in `migrations/` — currently only `migrations/0001_placeholder.sql`
  (creates a `placeholder` table).
- **Runtime**: migrations are NOT applied at runtime. `src/main.rs` and
  `db::init` never call `sqlx::migrate!` or `Migrator`. In Docker they are
  applied at build time: `Dockerfile` runs `sqlx database create` and
  `sqlx migrate run` during image build (Dockerfile, runtime stage), and
  copies `migrations/`, `templates/`, `static/` into the image.
- **Tests**: `#[sqlx::test]` auto-applies migrations from `./migrations` —
  used in `src/app/db.rs:39-48` (`migrations_applied` asserts the
  `placeholder` table exists). The `migrate` feature is enabled in sqlx
  (`Cargo.toml:9`).

## Q3: Environment variables and secrets

### Findings
- Only env var read: `DATABASE_URL` in `src/main.rs:5-6`, with a hardcoded
  default `"sqlite:data/vardy.db"`. Read directly via `std::env::var`; no
  config struct, no dotenv crate, no config module.
- `.env_template` at repo root contains only `DATABASE_URL=sqlite:data/vardy.db`.
- Docker sets `ENV DATABASE_URL=sqlite:data/vardy.db` (Dockerfile, runtime
  stage); `SQLX_OFFLINE=true` set for build.
- No other secrets/config exist. No pattern for passing config beyond the
  `AppState` struct (`src/app/state.rs:2-9`) — anything handler-visible
  would follow that precedent (fields on `AppState`, cloned per request).
- Port is hardcoded `0.0.0.0:3000` (`src/main.rs:15`), not configurable.

## Q4: HTTP client dependencies and JSON patterns

### Findings
- **Production dependencies have no HTTP client** (`Cargo.toml:5-12`): axum,
  minijinja, sqlx, tokio, tower-http only.
- **`reqwest` exists only as a dev-dependency**: `reqwest = { version =
  "0.13", features = ["json"] }` (`Cargo.toml:14`), used by test helpers
  (`src/test/mod.rs:26-28`) and route/handler tests.
- **serde is not a dependency at all** (neither `serde` nor `serde_json` in
  `Cargo.toml`). No JSON serialization/deserialization exists anywhere in
  the codebase; no derive macros, no JSON response handlers.
- The only outbound-request pattern in the repo is the test client:
  `reqwest::Client::new()` (`src/test/mod.rs:26-28`) making GETs against a
  locally spawned server.

## Q5: HTML rendering and static assets

### Findings
- Template env built once at startup: `app::templates::init()`
  (`src/app/templates.rs:1-11`) — `minijinja::path_loader("templates")`,
  auto-escape `Html` for `.html` names, `None` otherwise. Stored in
  `AppState.templates` (`src/app/state.rs:3`).
- Templates: `templates/layout.html` (base with `title`, `heading`,
  `content` blocks, nav links to `/` and `/singlethread`, stylesheet
  `/static/site.css`), `templates/home.html` and
  `templates/singlethread.html` both `{% extends "layout.html" %}`.
- Context construction: empty contexts today — `render(context! {})`
  (`src/interfaces/handlers/home/web.rs:9-12`). All page copy is hardcoded
  in the templates.
- Static assets: served by `ServeDir::new("static")` mounted at `/static`
  (`src/interfaces/routes.rs:12`); files in `static/` include
  `alanvardy.jpg`, `wave.svg`, `github.svg`, `linkedin.svg`,
  `singlethread-icon.png`, `site.css`.
- Templates reference images with absolute paths, e.g.
  `<img class="portrait" src="/static/alanvardy.jpg">` (`templates/home.html`),
  `<img src="/static/singlethread-icon.png">` (`templates/singlethread.html`).
- Docker copies `templates/` and `static/` relative to `/app` workdir
  (Dockerfile runtime stage) — loaders use relative paths, matching
  `WORKDIR /app`.

## Q6: Testing conventions

### Findings
- Test helpers in `src/test/mod.rs` (included via `#[cfg(test)] mod test;`
  in `src/main.rs:24` — note: module declared as `test`, directory is
  `src/test/`):
  - `start_app()` (`src/test/mod.rs:7-22`): builds `AppState` with
    `templates::init()` and `db::init("sqlite::memory:")`, binds
    `127.0.0.1:0`, spawns `axum::serve` on a tokio task, returns `SocketAddr`.
  - `test_client()` (`src/test/mod.rs:26-28`): fresh `reqwest::Client`.
- Integration test pattern: `let addr = start_app().await; let client =
  test_client(); client.get(format!("http://{addr}/...")).send()` — used in
  `src/interfaces/routes.rs:18-31` (health + 3 static-asset tests),
  `src/interfaces/handlers/home/web.rs:15-40`,
  `src/interfaces/handlers/singlethread/web.rs:15-37`.
- Assertions are on status code, `content-type` header, and body substring
  (`body.contains(...)`) — including exact HTML fragments and asset paths.
- DB tests: `#[sqlx::test]` for migration verification (`src/app/db.rs:39-48`);
  plain `#[tokio::test]` for file-creation behavior (`src/app/db.rs:50-58`).
  Unit tests for error mapping live in `src/app/error.rs:44-68` and template
  auto-escape in `src/app/templates.rs:14-40`.
- Tests live inline in `#[cfg(test)] mod tests` within each source file; no
  separate `tests/` directory.

## Cross-Cutting Observations

- Layering: `main.rs` (bootstrap) → `app/` (state, db, error, templates) →
  `interfaces/` (routes, handlers). Handlers depend only on `AppState`,
  `WebError`, and minijinja.
- Every handler currently renders a template with an empty context; the
  `db` pool is plumbed but unused — the first data-backed handler will set
  the query-organization precedent.
- Error handling is uniform: handlers return `Result<_, WebError>` and rely
  on `From` conversions (`?`).
- Deployment: fly.io (`fly.toml`, health check on `GET /health`), Docker
  build applies migrations at image build, not app start.
- CI in `.github/workflows/ci.yml` and `ci-secure.yml`; lint script at
  `scripts/lint_string.sh`.

## Sibling Project Reference: `../api` (spark API)

The sibling repo `/Users/vardy/dev/api` is a larger axum service by the same
author and is the reference for how API keys/secrets are fetched and used.

### Secrets loading pattern — `../api/src/app/env.rs`
- Dedicated `Env` struct with `Env::init().await` (`env.rs:8-45`); module
  doc states the workflow: "Set them for production with `fly secrets set
  KEY=VALUE`. Set them locally in `.env`" (`env.rs:1-3`).
- `.env_template` header lists the full checklist: new entries must be added
  to `.env`, `.env_template`, the fly.io dashboard, and 1Password.
- Helpers: `get_string_env(key)` panics if the var is missing or empty
  (`env.rs:117-122`); `get_bool_env` validates `true`/`false`
  (`env.rs:124-130`). Third-party creds are namespaced per service
  (e.g. `STORAGE_AWS_*`, `SES_AWS_*`) with "no fallback chain"
  (`env.rs:53-58`).
- `main.rs` calls `Env::init().await` before anything else
  (`../api/src/main.rs:23`) and threads values into `AppState`.
- Secrets reach handlers as `Arc<str>` fields on `AppState` (e.g.
  `jwt_secret: Arc<str>`, `apple_client_id: Arc<str>` —
  `../api/src/app/state.rs:9-16`), not the raw `Env` struct.

### Outbound HTTP + JSON pattern — `../api`
- `reqwest = { version = "0.13", features = ["json"] }` is a **production**
  dependency (`../api/Cargo.toml:32`), alongside `serde`/`serde_json`
  (`Cargo.toml:20-21`).
- Outbound calls take `client: &Client` as a parameter; caller constructs
  `Client::new()` (`../api/src/app/users.rs:160-167`). Example call:
  `client.get(APPLE_JWKS_URL).send().await?` then
  `response.json::<JwkSet>()` mapped to `AppError::BadGateway` on parse
  failure (`../api/src/app/apple_auth.rs:28-53`).
- External payloads are cached in-memory with a TTL via
  `OnceLock<Mutex<Option<(Instant, T)>>>` (`apple_auth.rs:20-40`).

## Open Areas

- The vardy repo itself has no outbound HTTP client or serde/JSON usage —
  the precedent lives in `../api` (see section above).
- No runtime migration execution — if migrations must run at app start,
  there is no existing pattern (only `#[sqlx::test]` and Docker build step).
- No query module convention, no repository/DAO layer, no caching layer.
- No rate limiting, secrets manager, or API-key storage mechanism exists.
