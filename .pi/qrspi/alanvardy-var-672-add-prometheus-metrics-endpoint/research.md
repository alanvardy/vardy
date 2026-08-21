# Research Findings

## Q1: How are HTTP routes registered and how are handler modules organized under `src/interfaces/`?

### Findings
- Startup flow: `#[tokio::main] async fn main()` at `src/main.rs:4-5` reads `DATABASE_URL` (default `"sqlite:data/vardy.db"`, `src/main.rs:6-7`), builds `AppState { templates, db }` (`src/main.rs:8-11`), binds `0.0.0.0:3000` (`src/main.rs:12`), then serves:
  ```rust
  axum::serve(listener,
      interfaces::routes::routes()
          .with_state(state)
          .into_make_service_with_connect_info::<std::net::SocketAddr>())
  ```
  (`src/main.rs:14-20`).
- Router is built by `pub fn routes() -> Router<AppState>` at `src/interfaces/routes.rs:7-13`, axum builder style:
  - `/` → `handlers::home::web::index` (`routes.rs:9`)
  - `/singlethread` → `handlers::singlethread::web::index` (`routes.rs:10`)
  - `/health` → inline closure `|| async { StatusCode::OK }` — no named handler (`routes.rs:11`)
  - `/static` → `nest_service(..., ServeDir::new("static"))` (`routes.rs:12`)
- Handler organization: `src/interfaces/handlers/mod.rs:1-2` declares one module per feature (`home`, `singlethread`); each feature module has a `web.rs` submodule (`handlers/home/mod.rs:1`, `handlers/singlethread/mod.rs:1`). Pattern: `handlers/<feature>/web.rs`.
- Handler shape: `pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError>` rendering a minijinja template (`src/interfaces/handlers/home/web.rs:7-12`, same in `singlethread/web.rs:7-12`).
- `WebError` enum: `Template(minijinja::Error) | Database(sqlx::Error) | NotFound` with `From` conversions and an `IntoResponse` impl (`src/app/error.rs:9-41`). `NotFound` is `#[allow(dead_code)]` and only produced from tests.
- No `/metrics` endpoint exists anywhere today; `/health` is the closest analog (inline closure, no state access).

## Q2: What middleware/tower layers exist, and where would per-request hooks live?

### Findings
- **No middleware or layers applied anywhere.** Zero `.layer(...)` / `.route_layer(...)` / `middleware::...` calls in `src/`. The router builder chain at `src/interfaces/routes.rs:8-13` and `axum::serve` at `src/main.rs:14-19` have no wrapping.
- Only tower-http usage is `ServeDir` as an endpoint service (imported `src/interfaces/routes.rs:2`), not middleware.
- Direct deps (`Cargo.toml:6-12`): `axum = "0.8.9"` (default features), `tower-http = { version = "0.6", features = ["fs"] }`, `tokio 1.52.3`, `minijinja`, `sqlx`. No direct `tower` or `hyper` dep.
- Cargo.lock resolved versions: `axum 0.8.9` (`Cargo.lock:65`), `tower-http 0.6.11` (`Cargo.lock:2047`), `tower 0.5.3` transitively (`Cargo.lock:2031`), `hyper 1.11.0` (`Cargo.lock:716`), `http 1.5.0` (`Cargo.lock:656`).
- Feature availability: axum's default features include `tracing` (`Cargo.lock:94`); tower-http currently enables only `fs` — `trace` / other features are not enabled but tower-http is already a direct dependency.
- Natural hook locations observed: the single builder chain inside `routes()` (`src/interfaces/routes.rs:7-13`) is the choke point for all routes; both production (`src/main.rs:16`) and tests (`src/test/mod.rs:16`) call `routes()`, so a layer added there covers both. `into_make_service_with_connect_info::<SocketAddr>()` (`src/main.rs:17`) already makes per-connection info available.

## Q3: Existing logging/tracing/error-reporting/metrics instrumentation

### Findings
- **No logging framework, tracing subscriber, metrics library, or error reporter exists.** No `log`, `tracing`, `tracing-subscriber`, `env_logger`, `slog`, `metrics`, `prometheus`, or `opentelemetry` crates in `Cargo.toml` or `Cargo.lock`.
- Entire console-output surface is three statements:
  | Location | Statement |
  |---|---|
  | `src/main.rs:13` | `println!("Hosting on http://localhost:3000");` (once at startup) |
  | `src/app/error.rs:32` | `eprintln!("database error: {err}");` before returning 500 |
  | `src/app/error.rs:36` | `eprintln!("template render error: {err}");` before returning 500 |
- Error flow: handlers return `Result<Html<String>, WebError>` using `?`; all HTTP error rendering happens in `IntoResponse for WebError` (`src/app/error.rs:30-41`). `NotFound` → 404 with no output; Database/Template → stderr line + generic `500 "internal server error"` body.
- Startup failure handling uses panics, not logging: `.expect(...)` calls in `src/app/db.rs:9,20,27`.
- Successful requests emit zero output. No access log, latency measurement, counters, histograms, or health introspection exist. `/health` returns constant `StatusCode::OK` (`src/interfaces/routes.rs:11`) without touching state.

## Q4: AppState construction and shared mutable state patterns

### Findings
- `AppState` (`src/app/state.rs:1-7`): `#[derive(Clone)] pub struct AppState { pub templates: minijinja::Environment<'static>, #[allow(dead_code)] pub db: sqlx::SqlitePool }`. Two fields; no interior mutability declared.
- Production construction: `src/main.rs:8-11` — `templates::init()` (path_loader on `templates/` with HTML auto-escape, `src/app/templates.rs:1-10`) and `db::init(&database_url)` (WAL-mode SQLite pool, `max_connections(5)`, `create_if_missing(true)`, `src/app/db.rs:6-29`).
- Test construction: same struct literal with `"sqlite::memory:"` (`src/test/mod.rs:6-10`).
- Threading: router is `Router<AppState>` (`src/interfaces/routes.rs:7`); state attached via `.with_state(state)` in prod (`src/main.rs:16`) and tests (`src/test/mod.rs:16`); handlers extract with `State(state): State<AppState>` (`home/web.rs:7`, `singlethread/web.rs:7`).
- **Shared mutable state patterns: none exist.** Repo-wide search for `Arc<`, `Mutex`, `RwLock`, `Atomic*`, `OnceCell`, `OnceLock`, `lazy_static` across `src/` returns zero matches. No counters or atomics anywhere.
- Implicit sharing lives inside library types: `SqlitePool` and `minijinja::Environment` are internally Arc-backed; handlers clone `AppState` per request via the `Clone` derive.
- `state.db` is dead code from the router's perspective (constructed, never queried) — flagged by its own comment at `src/app/state.rs:4-5`.

## Q5: Testing infrastructure for HTTP endpoints

### Findings
- Harness: `pub async fn start_app() -> SocketAddr` (`src/test/mod.rs:5-21`) builds real `AppState` (in-memory SQLite), binds `127.0.0.1:0` (random port), assembles the real production router `routes().with_state(state)` (`test/mod.rs:16`), and spawns `axum::serve` on a tokio task. Note: uses `into_make_service()`, **not** `into_make_service_with_connect_info::<SocketAddr>()` as production does (`src/main.rs:17`).
- `pub fn test_client() -> reqwest::Client` returns `reqwest::Client::new()` (`src/test/mod.rs:23-25`); tests make real HTTP requests over TCP. Test module wired via `#[cfg(test)] mod test;` (`src/main.rs:24-25`).
- Route tests (`src/interfaces/routes.rs`):
  - `static_icon_is_served` (`routes.rs:20-34`): GET `/static/singlethread-icon.png`; asserts status `OK` + content-type header contains `"image/png"`; no body assertions.
  - `health_returns_200` (`routes.rs:38-45`): GET `/health`; asserts status `OK` only — no content-type or body assertion.
- Handler tests use identical harness: home page asserts status + content-type contains `"text/html"` + body `contains(...)` checks for title/nav strings (`src/interfaces/handlers/home/web.rs:15-37`); singlethread page likewise (`src/interfaces/handlers/singlethread/web.rs:15-40`).
- Dev-dependencies: exactly one entry, `reqwest = { version = "0.13", features = ["json"] }` (`Cargo.toml:13-14`); json feature unused by current tests.
- Assertion styles in use: status-code equality everywhere; content-type substring for png/html; body-text `contains(...)` for HTML pages only. No JSON/body parsing tests. Tests bind real sockets; no in-process `oneshot` testing.

## Q6: Deployment & runtime configuration

### Findings
- Port binding: hardcoded `TcpListener::bind("0.0.0.0:3000")` (`src/main.rs:12`) — no `PORT` env var read. Fly.io matches: `internal_port = 3000` (`fly.toml:13`). Dockerfile declares no `EXPOSE` directive.
- Dockerfile: multi-stage cargo-chef build (`Dockerfile:1`); release binary `cargo build --release --bin vardy` (`Dockerfile:14`); runtime `debian:bookworm-slim` (`Dockerfile:17`); copies sqlx CLI + `migrations`, `templates`, `static` into `/app` (`Dockerfile:19-22`); `ENV DATABASE_URL=sqlite:data/vardy.db` (`Dockerfile:23`); creates DB and runs migrations at image build time (`Dockerfile:24-26`); `ENTRYPOINT ["/usr/local/bin/vardy"]` (`Dockerfile:27`).
- fly.toml: app `vardy`, region `ord` (`fly.toml:7-8`); `force_https`, auto stop/start machines, `min_machines_running = 1` (`fly.toml:14-18`); **HTTP health check defined**: `GET /health`, interval 30s, timeout 5s, grace_period 10s (`fly.toml:20-25`); VM 1 CPU / 512MB (`fly.toml:27-29`).
- Health route backing the check: closure returning bare 200 (`src/interfaces/routes.rs:11`), tested at `src/interfaces/routes.rs:38-45`.
- GitHub workflows: `.github/workflows/fly-deploy.yml` deploys on push to main via `flyctl deploy --remote-only` with `FLY_API_TOKEN` secret (`fly-deploy.yml:5-18`). `ci.yml` runs nextest, fmt, clippy `-D warnings --locked`, TODO lint. Others (codeql, dependabot, rust-version-bump) are not deploy-related.
- `.env_template` contains only `DATABASE_URL=sqlite:data/vardy.db` (`.env_template:1`).
- **No metrics/scrape configuration exists anywhere**: grep for `metrics`/`prometheus`/`scrape` across config files and docs returns nothing. Single port 3000 is the only exposure.

## Cross-Cutting Observations
- Everything funnels through `interfaces::routes::routes()`: production (`src/main.rs:16`) and tests (`src/test/mod.rs:16`) share the exact same router construction, so any change to routes/layers affects both paths uniformly.
- Observability is essentially absent: three print statements total, no tracing/logging/metrics dependencies, no middleware. Any instrumentation would introduce these patterns for the first time (no existing atomics/counters/Arc patterns to extend — see Q3/Q4).
- `AppState` is a small two-field Clone struct built once at startup; adding fields follows the existing struct-literal pattern duplicated in exactly two places (`src/main.rs:8-11`, `src/test/mod.rs:6-10`).
- The `/health` endpoint is the only non-page/non-static route and uses an inline closure rather than the `handlers/<feature>/web.rs` module pattern used by page handlers.
- Content-type conventions: HTML pages return `text/html` (axum `Html`); static files served by ServeDir with correct mime types. Tests assert via header substring matching.
- CI enforces `cargo fmt`, `clippy --all-targets --all-features --locked -D warnings`, and nextest — new code must satisfy locked-deps builds and warning-free clippy.

## Open Areas
- Whether metrics should live on the same port 3000 vs a separate listener/port: no precedent exists either way (single-port service, no extra ports exposed in Dockerfile or fly.toml).
- No evidence of how request-level metric labels (method/path/status) would interact with `ServeDir` under `/static`, which bypasses normal handler routing.
- The `reqwest` dev-dep `json` feature is enabled but unused; unclear whether future tests intend structured-body assertions.
- `state.db` is constructed but unused; whether metrics should be tied to DB pool stats has no existing signal in the codebase.
