# Research Findings

Branch: `alanvardy-var-674-add-logging`

## Q1: How is the application initialized in `src/main.rs`?

### Findings
- `src/main.rs:1-3` — module declarations only (`mod app; mod infra; mod interfaces;`). No logging/tracing setup exists anywhere at startup.
- Initialization order inside `async fn main` (`src/main.rs:5-30`):
  1. Read `DATABASE_URL` env var with fallback `"sqlite:data/vardy.db"` (`src/main.rs:7-8`).
  2. Build metrics: `Arc::new(infra::metrics::AppMetrics::new()?)` (`src/main.rs:9`; `AppMetrics::new` at `src/infra/metrics.rs:9-21`, creates its own prometheus `Registry`).
  3. Build `AppState` (`src/main.rs:10-14`): `templates::init()` (sync minijinja env, `src/app/templates.rs:3`), `db::init(&database_url).await` (`src/app/db.rs:7`, panics via `.expect("invalid DATABASE_URL")` on bad URL), shared `metrics.clone()`.
  4. Bind main listener `0.0.0.0:3000` (`src/main.rs:15`).
  5. `println!("Hosting on http://localhost:3000")` — **the only stdout output in the codebase** (`src/main.rs:16`).
  6. Bind metrics listener `0.0.0.0:9090` (`src/main.rs:17`).
  7. `tokio::try_join!` serves both concurrently (`src/main.rs:18-29`); either failing short-circuits startup.
- Main service uses `.into_make_service_with_connect_info::<SocketAddr>()` (`src/main.rs:20-24`); metrics service uses plain `.into_make_service()` (`src/main.rs:25-28`).
- Other output: only stderr writes in `src/app/error.rs:32,36` (see Q3). No other `println!`/`eprintln!`/`dbg!` outside tests.
- Note: the "Hosting" banner prints after the 3000 bind but before the 9090 bind succeeds.

## Q2: Middleware/layers on the routers in `src/interfaces/routes.rs`

### Findings
- Imports (`src/interfaces/routes.rs:5`): only `tower_http::{services::ServeDir, set_header::SetResponseHeader}`. **No `.layer(...)` or `.middleware(...)` calls exist anywhere in the file** — no TraceLayer, CORS, timeout, or compression.
- Main router `pub fn routes() -> Router<AppState>` (`src/interfaces/routes.rs:11-28`):
  - Plain routes: `/` (`routes.rs:13`), `/singlethread` (:14), `/dump/{key}` GET+POST (:15-18), `/health` returning `StatusCode::OK` (:19).
  - Static files (`routes.rs:20-26`): `nest_service("/static", SetResponseHeader::overriding(ServeDir::new("static"), CACHE_CONTROL, "public, max-age=31536000, immutable"))`. This is the sole tower-http usage.
- Metrics router `pub fn metrics_router(Arc<AppMetrics>) -> Router` (`src/interfaces/routes.rs:31-34`): single route `/metrics`, state bound eagerly with `.with_state(metrics)` producing a concrete `Router`.
- Composition differences:
  - Main router is generic `Router<AppState>`, state injected externally (`.with_state(state)` in `src/main.rs:21-22` and `src/test/mod.rs:20,44`); metrics router binds its own state inline.
  - Main router served with connect-info make service; metrics router without (`src/main.rs:20-28`).
  - Test harness mirrors the split with two listeners (`src/test/mod.rs:44-54`).

## Q3: `WebError`'s `IntoResponse` impl in `src/app/error.rs`

### Findings
- Enum variants (`src/app/error.rs:9-13`): `Template(minijinja::Error)`, `Database(sqlx::Error)`, `NotFound` (constructed only from tests, kept via `#[allow(dead_code)]`, `error.rs:8`).
- `From<minijinja::Error>` and `From<sqlx::Error>` conversions at `error.rs:15-19` and `error.rs:21-25`.
- `IntoResponse` (`error.rs:27-41`):
  - `NotFound` → `(NOT_FOUND, "not found")`; nothing written to stderr (`error.rs:30`).
  - `Database(err)` → `eprintln!("database error: {err}")` then `(INTERNAL_SERVER_ERROR, "internal server error")` (`error.rs:31-34`).
  - `Template(err)` → `eprintln!("template render error: {err}")` then 500 (`error.rs:35-38`).
- Detail available at the logging point: only the wrapped error's `Display` output plus an implicit variant discriminator in the hardcoded prefix string. Not available: source chain (`{:?}` not used), backtrace, timestamp, log level, request info — the signature is `fn into_response(self)`, so no method/URI/path access.
- Callers surfacing `WebError`: `src/interfaces/handlers/home/web.rs:4,7`; `src/interfaces/handlers/dump/web.rs:1,18,33`; `src/interfaces/handlers/singlethread/web.rs:4,7`.
- Unit tests (`error.rs:43-71`) assert status codes and variant shape only; none assert stderr content:
  - `not_found_is_404` (:47-51), `template_error_is_500` (:53-58), `database_error_is_500` (:60-64), `sqlx_error_converts_via_from` (:66-70).

## Q4: Logging patterns in sibling project `../api`

### Findings
- Crates (`../api/Cargo.toml`): `tracing = "0.1"` (:33), `tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }` (:34), `tower-http = { version = "0.7", features = ["trace"] }` (:31).
- Subscriber init — `pub fn init()` (`../api/src/app/log.rs:47-57`):
  - Filter: `EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,tower_http=info"))` (`log.rs:48-49`).
  - Format: `fmt().json().flatten_event(true).with_current_span(false).with_env_filter(filter).with_writer(StderrWriter).init()` — one flattened JSON line per event to stderr (`log.rs:51-57`); comment says JSON so Fly.io capture can forward to Loki/Grafana (`log.rs:45-46`).
  - Custom `StderrWriter` (`log.rs:14-41`): implements `Write` + `MakeWriter`, swallows `BrokenPipe` errors so pipe closure (journald/Fly.io/terminal exit) doesn't panic.
- Request tracing — `pub fn trace_layer()` (`log.rs:62-68`): `TraceLayer::new_for_http()` with `.make_span_with(make_span)`, INFO `on_request`/`on_response`. `make_span` (`log.rs:70-81`) logs the matched route pattern via `MatchedPath` (low cardinality, e.g. `/users/{id}`) falling back to `uri().path()`, in span `http_request` with `method` and `path` fields.
- Wiring call sites (`../api/src/main.rs`):
  - `app::log::init();` as the first statement of `main` (`main.rs:22`).
  - `let router = interfaces::routes::routes(env).layer(app::log::trace_layer());` inside `fn app(...)` (`main.rs:83`).
  - Startup log via `tracing::info!("Server running on ...")` (`main.rs:38`).
  - The metrics router on 9090 (`main.rs:52-56`) is NOT wrapped with `trace_layer()` — only the main router.

## Q5: Test harness in `src/test/mod.rs`

### Findings
- Tests never call `main`. `#[cfg(test)] mod test;` is declared at `src/main.rs:33-34`, but the harness boots everything itself:
  - `start_app` (`src/test/mod.rs:5-24`): fresh in-memory DB `sqlite::memory:` (:6), runs migrations via `sqlx::migrate!("./migrations")` (:7-9), builds its own `AppState` (:10-14), binds ephemeral port `127.0.0.1:0` (:15-18), builds `routes().with_state(state)` (:19), and **spawns its own `axum::serve`** via `tokio::spawn` (:20-23). Returns the bound address (:24). Real HTTP + real `reqwest` client (dev-dep, `Cargo.toml:19`), not tower `oneshot`.
  - `start_app_with_metrics` (`src/test/mod.rs:30-56`): same construction plus a second listener and two separate spawned `axum::serve` tasks (:48-56).
- Every test spins up its own full stack; call sites include `src/interfaces/routes.rs:39,44,96,113,125,142`, `src/interfaces/handlers/home/web.rs:18,23`, `src/interfaces/handlers/dump/web.rs:47,52,71,97,126`, `src/interfaces/handlers/singlethread/web.rs:18,23`, `src/test/mod.rs:65`.
- No process-wide subscriber exists today: grep for `tracing|subscriber|log::|env_logger` across `src/` returns nothing. Since tests bypass `main`, anything initialized in `main` never runs under tests.
- Nextest behavior: `.config/nextest.toml` contains only `[profile.ci.junit]`; `scripts/test.sh:25` runs plain `cargo nextest run` → default one-process-per-test isolation. A process-wide subscriber initialized per test process would run once per test process (N times across a run). Existing global-ish state is limited to `ASSET_HASHES: OnceLock` (`src/app/assets.rs:8,38`), which re-inits harmlessly per process; `AppMetrics` uses per-instance registries (`src/infra/metrics.rs:4,8`), not the prometheus default registry.
- Each test uses `#[tokio::test]` with its own runtime; spawned servers terminate when the test ends.

## Q6: Production deployment & config conventions

### Findings
- Fly.io deployment: `fly.toml:5-6` app `vardy`, region `ord`; HTTP internal port 3000 (`fly.toml:12`) matching `src/main.rs:15`; health check `GET /health` (`fly.toml:19-25`) matching `src/interfaces/routes.rs:19`; metrics port 9090 path `/metrics` (`fly.toml:31-33`) matching `src/main.rs:17` and `src/interfaces/routes.rs:31-34`; 512MB / 1 CPU VM (`fly.toml:27-29`). CI deploys on push to main via `flyctl deploy --remote-only` with `FLY_API_TOKEN` secret (`.github/workflows/fly-deploy.yml:17-19`).
- Dockerfile: multi-stage cargo-chef build (`Dockerfile:1`); `SQLX_OFFLINE=true` with committed `.sqlx/` metadata (`Dockerfile:17-18`); runtime stage `debian:bookworm-slim` copying migrations/templates/static/binary (`Dockerfile:21-24`); `ENV DATABASE_URL=sqlite:data/vardy.db` baked in (`Dockerfile:25`); migrations run at image build time (`Dockerfile:26-28`), not app start; `ENTRYPOINT ["/usr/local/bin/vardy"]` (`Dockerfile:29`) — binary is PID-1, stdout/stderr go directly to container/Fly logs.
- Output capture today: no logging crate in `Cargo.toml:6-18` (axum, minijinja, prometheus, serde, sha2, sqlx, tokio, tower-http only). All runtime output is ad-hoc: stdout banner `src/main.rs:16`; stderr `src/app/error.rs:32,36`. Unstructured — no timestamps, levels, or request logging. Fly captures fd 1/2 as machine logs.
- Environment conventions:
  - `.env_template:1` contains exactly `DATABASE_URL=sqlite:data/vardy.db`; convention `cp .env_template .env` (`README.md:5`). `.env` is gitignored (`.gitignore:3`). There is no `.env.example`.
  - Code reads only `DATABASE_URL` (`src/main.rs:7-8`, hardcoded fallback; parse failure panics at `src/app/db.rs:9`).
  - `SQLX_OFFLINE=true` is build-time only (`Dockerfile:17`); scripts source `.env` (`scripts/test.sh:2-3`).
  - Ports are hardcoded, not env-configurable (`src/main.rs:15,17`).

## Cross-Cutting Observations
- Total runtime output today is three unstructured lines: one stdout banner (`src/main.rs:16`) and two stderr error lines (`src/app/error.rs:32,36`). No logging dependency exists in `Cargo.toml`.
- The sibling `../api` provides a complete, established template for all three touchpoints this repo lacks: subscriber init called first in `main` (`main.rs:22`), a `trace_layer()` applied via `.layer()` on the main router only (`main.rs:83`), and a BrokenPipe-safe stderr JSON writer (`../api/src/app/log.rs:14-41`).
- Both projects share the two-router topology (app on 3000 with connect-info, metrics on 9090 bare); in `../api` the trace layer covers only the app router, not metrics.
- The test harness bypasses `main` entirely and spawns its own servers per test under nextest's one-process-per-test model — any subscriber initialization placed only in `main` will be invisible to tests.
- Error detail loss is structural: `IntoResponse::into_response(self)` receives no request context, and handlers return bare `WebError` without wrapping request info.
- Env configuration surface is minimal: a single `DATABASE_URL` variable with hardcoded fallbacks for everything else (ports, DB path).

## Open Areas
- Whether any Fly.io log shipping/aggregation is configured outside the repo (secrets, log drains) could not be verified from the codebase; `../api`'s comment references Loki/Grafana forwarding but this repo has no equivalent config.
- Runtime behavior of `eprintln!` under Fly.io pipe closure (the problem `../api`'s `StderrWriter` solves) was inferred from code, not observed live.
