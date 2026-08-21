# Research Findings

## Q1: How is AppState defined, constructed, and threaded through the router and handlers?

### Findings
- `AppState` is a two-field-free struct with one field, `templates: minijinja::Environment<'static>`, `#[derive(Clone)]` (`src/app/state.rs:1-4`). No `new()`/`Default` — both call sites use struct literals.
- Template environment factory: `app::templates::init()` builds an empty environment with `path_loader("templates")` and HTML auto-escape for `.html` names (`src/app/templates.rs:1-11`).
- Production construction: `src/main.rs:6-8` builds `AppState { templates: app::templates::init() }`, binds `0.0.0.0:3000` (`src/main.rs:9`), serves `routes().with_state(state).into_make_service_with_connect_info::<SocketAddr>()` (`src/main.rs:11-16`). `connect_info` is provided but no handler consumes it.
- Router is generic: `pub fn routes() -> Router<AppState>` (`src/interfaces/routes.rs:6`); one route `/` → `get(handlers::home::web::index)` (`src/interfaces/routes.rs:9`).
- Handler extraction: `pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError>` (`src/interfaces/handlers/home/web.rs:7-14`); axum clones state per request (hence `Clone`).
- Independent test construction: `src/test/mod.rs:5-18` `start_app()` builds its own `AppState` (`src/test/mod.rs:6`), binds `127.0.0.1:0`, serves `routes().with_state(state)` with plain `into_make_service()` (`src/test/mod.rs:16`), returns the bound `SocketAddr`.
- Both sites share `app::templates::init()` (`src/main.rs:7`, `src/test/mod.rs:7`); only listener address and make-service differ.
- Test module is compiled only under cfg(test): `#[cfg(test)] mod test;` (`src/main.rs:21-22`).

## Q2: How was SQLite configured in ../api's git history before Postgres?

### Findings (from git history of `/Users/vardy/dev/api`; last SQLite commit is `4fb273f^`, boundary commit `4fb273f` "feat: switch from SQLite to PostgreSQL")
- Crates/features at `4fb273f^` `Cargo.toml:22`: `sqlx = { version = "0.9.0", features = ["sqlite", "runtime-tokio", "chrono"] }`. No `tls` or `migrate` features in the SQLite era.
- Connection creation (`4fb273f^:src/infra/db.rs`, path history: `src/db.rs` → `src/app/db.rs` (`41c2aee`) → `src/infra/db.rs` (`912ba5e`)):
  ```rust
  pub async fn init(database_url: &str) -> SqlitePool {
      let options = SqliteConnectOptions::from_str(database_url).unwrap()
          .create_if_missing(true).foreign_keys(true);
      SqlitePoolOptions::new().max_connections(5).connect_with(options).await.unwrap()
  }
  ```
  Only `create_if_missing(true)` and `foreign_keys(true)` were set. No PRAGMA or journal-mode (WAL) configuration ever existed in code.
- Sharing: pool created once in `main` from `DATABASE_URL` env (`src/app/env.rs:39`; default `DATABASE_URL=sqlite:test.db` in `.env_template`), stored as `pub db: SqlitePool` in `AppState` (`#[derive(Clone)]`; `SqlitePool` is Arc-backed so clones share one pool), accessed by handlers via `State(state)` and `&state.db`.
- Migrations evolved through three phases:
  1. In-code DDL: `create_table` with `CREATE TABLE IF NOT EXISTS` (`44fa2df` "add sqlite", dropped in `9f789b8`).
  2. `migrations/` directory (from `73a8f09`/`9f789b8`): 11 SQL files with SQLite syntax (`INTEGER PRIMARY KEY AUTOINCREMENT`, `TEXT`, `REFERENCES users(id)`), applied by `sqlx-cli`. SQLite-era Dockerfile installed `sqlx-cli --no-default-features --features sqlite` and ran `sqlx database create` + `sqlx migrate run` in both build and runtime stages (migrations at image build time).
  3. Tests: `#[sqlx::test]` with a `pool: SqlitePool` argument (e.g., `src/app/sessions.rs:124`), which auto-applies the crate-root `migrations/` to a per-test temp database.
- Query conventions: after `b655cc3` "validate queries", compile-time-checked macros were the norm — `query!`, `query_as!(T, "... RETURNING ...")`, `query_scalar!` with `?` placeholders; type overrides in SQL like `category as "category!: String"`, `updated_at as "updated_at: DateTime<Utc>"` (e.g., `src/app/files.rs:263`). Non-macro `sqlx::query(...)` survived only in tests/fixtures.
- Offline metadata: `.sqlx/*.json` cache via `cargo sqlx prepare` (`cc30e4b`) so CI compiles without a live DB.

## Q3: How does error handling work (WebError, IntoResponse)?

### Findings
- `WebError` is a two-variant enum: `Template(minijinja::Error)` and `NotFound`, with `#[allow(dead_code)]` because `NotFound` is only constructed from unit tests (`src/app/error.rs:6-12`).
- Single `From` impl: `From<minijinja::Error> for WebError` (`src/app/error.rs:14-18`) — this is what lets `?` work in handlers for template errors.
- `IntoResponse` mapping (`src/app/error.rs:20-31`): `NotFound` → 404 `"not found"`; `Template(err)` → `eprintln!` to stderr + 500 `"internal server error"`. No details leak to the client.
- Handler usage: `Result<Html<String>, WebError>` with `?` on `get_template(...)` and `render(...)` (`src/interfaces/handlers/home/web.rs:7-14`). No middleware, fallback route, or error layer exists (`src/interfaces/routes.rs:1-9`).
- Unit tests pin the status mapping (`src/app/error.rs:33-48`).
- Pattern a new fallible operation follows today: return `Result<T, WebError>` from the handler; `minijinja::Error` auto-converts via the existing `From`; any other error type has **no** existing `From` impl or variant — only `NotFound` is hand-constructable, and it is dead-code-gated outside tests.

## Q4: How are tests organized and what constraints exist?

### Findings
- Shared helpers in `src/test/mod.rs` (compiled only under `#[cfg(test)]`, declared `src/main.rs:21-22`): `start_app() -> SocketAddr` (random port via `127.0.0.1:0`, `tokio::spawn`ed `axum::serve`, `src/test/mod.rs:5-18`) and `test_client() -> reqwest::Client` (`src/test/mod.rs:20-22`).
- `reqwest = { version = "0.13", features = ["json"] }` is the only dev-dependency (`Cargo.toml:9`).
- Inline `#[cfg(test)]` modules: `src/app/error.rs:32` (2 tests), `src/app/templates.rs:14` (auto-escape), `src/interfaces/handlers/home/web.rs:15` (`#[tokio::test]` integration test using `start_app()` + `test_client()`).
- Parallelism: no `serial_test`, no `--test-threads=1`, no nextest concurrency overrides (`.config/nextest.toml` has only JUnit output config). Tests run fully parallel; existing HTTP tests are parallel-safe because of ephemeral ports. No fixture/seeding/DB-isolation helpers exist.
- CI: PRs run `cargo nextest run --profile ci`; main pushes run `cargo llvm-cov nextest --profile ci --all-features --lcov` uploaded to Codecov (`.github/workflows/ci.yml:63-79`). Coverage thresholds are enforced by Codecov, not locally: `codecov.yml` sets project target 70%, patch target 90%, ignoring `src/main.rs`. No tarpaulin/llvm-cov config files exist.
- Also in CI: `cargo fmt --check`, `clippy --all-targets --all-features --locked -- -D warnings`, and a string-lint gate `scripts/lint_string.sh` failing on `FIXME`/`fixme`/`dbg!` (`ci.yml:82-91`).

## Q5: What do Dockerfile, fly.toml, and CI assume about dependencies and persistent files?

### Findings
- Dockerfile: multi-stage with `lukemathwalker/cargo-chef:latest-rust-1` planner/builder; runtime image is `debian:bookworm-slim` (glibc, dynamically linked; `Dockerfile:16`). Copies `/app/templates` → `/app/templates` and the release binary → `/usr/local/bin` (`Dockerfile:17-18`). `WORKDIR /app`, `ENTRYPOINT /usr/local/bin/vardy` (`Dockerfile:19-20`).
- fly.toml: app `vardy`, `primary_region = 'ord'`; `[env] PORT = '8080'` is dead config (app binds hardcoded `0.0.0.0:3000`, matching `internal_port = 3000`; `src/main.rs:9`). VM: 512mb / 1 cpu (`fly.toml:21-23`). `auto_stop_machines = 'stop'`, `min_machines_running = 1`.
- **No `[mounts]` section** — no Fly volumes; the container filesystem is ephemeral. The only runtime on-disk assets are the baked-in `/app/templates`, loaded via relative path `"templates"` from `WORKDIR /app` (`src/app/templates.rs:3`). Any file written at runtime would be lost on redeploy/restart.
- CI workflows: `ci.yml` (tests, coverage, fmt, clippy `-D warnings`, TODO/FIXME/`dbg!` string lint); `ci-secure.yml` (weekly CodeQL + advisory clippy SARIF); `fly-deploy.yml` (deploy on push to main/master via `flyctl deploy --remote-only`); `rust-version-bump.yml` (weekly toolchain PR, currently pinned 1.97.1 in `rust-toolchain.toml`); Dependabot auto-merge.
- Cargo.toml assumptions: `axum = "0.8.9"` (default features), `tokio = "1.52.3"` with features `["rt-multi-thread", "macros", "net", "io-util"]` — **no `fs` feature**, so `tokio::fs` is unavailable as-is; `minijinja = "2"` (debug). No sqlx/rusqlite/sqlite dependency exists yet in `Cargo.toml` or `Cargo.lock`. Release LTO/codegen settings exist only as CI env vars, not `[profile.*]` sections.

## Cross-Cutting Observations
- State is minimal and clone-per-request; both production (`src/main.rs:6-8`) and tests (`src/test/mod.rs:6`) construct it independently but share the `templates::init()` factory — a second field (e.g., a pool) would need to be added in both places.
- The `../api` SQLite era provides a complete prior end-to-end template: `SqliteConnectOptions` + `SqlitePool` in state, `sqlx-cli` migrations, `#[sqlx::test]` per-test databases, compile-time-checked macros, and `.sqlx/` offline metadata for CI.
- Error handling currently has exactly one `From` conversion (`minijinja::Error`); any new fallible source (e.g., DB errors) has no existing variant or conversion path (`src/app/error.rs:9-18`).
- The deployment has no persistent storage (no Fly mounts) and the runtime image only bakes in `templates/`; a database file would need either a volume, a path convention, or image-baked placement.
- CI gates that new code must satisfy: clippy `-D warnings`, fmt, the FIXME/`dbg!` string lint, and Codecov targets (70% project / 90% patch) — the `NotFound` variant's `#[allow(dead_code)]` comment (`src/app/error.rs:6-8`) shows coverage-driven test conventions already at work.

## Open Areas
- No SQLite driver dependency exists anywhere in this repo yet (`Cargo.toml`, `Cargo.lock`); all SQLite facts come from `../api`'s history.
- Whether the deployed app would get a Fly volume, a baked-in database file, or another persistence strategy is not determined by any config in this repo.
- `tokio` lacks the `fs` feature (`Cargo.toml:7`), so async filesystem operations are not currently available.
