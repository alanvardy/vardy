# Research Findings

## Q1: Full request pipeline from `main.rs` through `routes.rs`

### Findings
- `#[tokio::main] async fn main()` at `src/main.rs:12`; `app::log::init()` first (`src/main.rs:14`), then `Env::init()` (`:16`), optional Sentry init gated by `env.enable_sentry` (`:17-19`), metrics/HTTP-client/DB setup (`:20-23`), and one `AppState` built at `src/main.rs:25-32`.
- Two listeners served concurrently under `tokio::try_join!` (`src/main.rs:37-49`):
  - App on `0.0.0.0:3000`: `routes().with_state(state).layer(app::log::trace_layer()).into_make_service_with_connect_info::<std::net::SocketAddr>()` (`src/main.rs:40-43`).
  - Metrics on `0.0.0.0:9090`: `metrics_router(metrics).into_make_service()` — no layers, no connect-info (`src/main.rs:45-47`).
- Routers in `src/interfaces/routes.rs`:
  - `pub fn routes() -> Router<AppState>` (`routes.rs:20-38`): `/`, `/singlethread`, `/unsplash`, `/dump/{key}` (GET+POST), `/health`, and `nest_service("/static", SetResponseHeader::overriding(ServeDir::new("static"), CACHE_CONTROL, ...))` (`routes.rs:30-37`) — the only other middleware-like layer, attached inside routing.
  - `pub fn metrics_router(metrics: Arc<AppMetrics>) -> Router` (`routes.rs:41-45`); state type is `Arc<AppMetrics>`, not `AppState`.
  - Handler state extraction confirms the two types: `State<AppState>` (e.g. `handlers/dump/web.rs:12`, `handlers/home/web.rs:7`) and `State<Arc<AppMetrics>>` (`handlers/metrics/web.rs:8`). `AppState` defined at `src/app/state.rs:8-19`.
- Per-request execution order on :3000 (outermost→innermost): TCP accept → `IntoMakeServiceWithConnectInfo` inserts a `ConnectInfo<SocketAddr>` request extension → `TraceLayer` → router matching → handler; errors funnel through `WebError::into_response` (`src/app/error.rs`).
- `into_make_service_with_connect_info::<SocketAddr>()` mechanics (axum 0.8.9 vendored source, `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/axum-0.8.9/src/extract/connect_info.rs:120-133`): the make-service wraps each connection's service with `Extension(ConnectInfo(C::connect_info(target)))`, where for `TcpListener` the value comes from the accepted stream's `remote_addr()` (`connect_info.rs:81-85`). Handlers/middleware read it via the `ConnectInfo<SocketAddr>` extractor, which is an Extension lookup with a `MockConnectInfo` fallback for tests (`connect_info.rs:150-166`).
- Currently nothing in `src/` consumes `ConnectInfo` — grep for `ConnectInfo|SocketAddr|X-Forwarded|peer` matches only `src/main.rs:43` and type imports in `src/test/mod.rs`. No proxy-header handling exists.

## Q2: Middleware crates, features, versions, test usage

### Findings
- Runtime middleware deps: `tower-http = { version = "0.6", features = ["fs", "set-header", "trace"] }` (Cargo.toml:16); axum `= "0.8.9"` (Cargo.toml:7). `tower = "0.5"` exists only under `[dev-dependencies]` (Cargo.toml:23).
- Resolved versions (Cargo.lock): tower **0.5.3** (lock:3333), tower-http **0.6.11** (lock:3349), tower-layer **0.3.3**, tower-service **0.3.3**, axum **0.8.9** (lock:296), axum-core **0.5.6**, http **1.5.0**. Exactly one resolved `tower` version in the lockfile.
- Source usage: `tower_http` trace imports at `src/app/log.rs:4-8`; layer construction at `src/app/log.rs:62-67`; applied at `src/main.rs:42`; `ServeDir`/`SetResponseHeader` at `src/interfaces/routes.rs:7,30-37`.
- Test `oneshot` usage: exactly one — `use tower::ServiceExt;` inside `#[cfg(test)] mod tests` at `src/interfaces/routes.rs:75`, exercising `metrics_router` with `router.oneshot(Request...)` asserting status 200 and content-type `text/plain; version=0.0.4` (`routes.rs:80-88`). All other tests use real HTTP via `crate::test::{start_app, test_client}`.
- Compatibility constraints (observed facts): axum 0.8 / tower 0.5 / tower-http 0.6 all sit on the `http 1.x` + `tower-service 0.3` + `tower-layer 0.3` stack; `Router::layer` accepts any compatible `tower-layer::Layer` over `Request<Body>` (demonstrated at `src/main.rs:42`). Toolchain pinned Rust 1.97.1 (`rust-toolchain.toml:4`), edition 2024 (Cargo.toml:4).
- A legacy `http 0.2.12` entry exists in Cargo.lock (:1140) pulled by some other dep — not part of the axum/tower trees shown.

## Q3: `src/app/env.rs` configuration loading

### Findings
- `Env` struct has four fields, read eagerly by `Env::init()` (`src/app/env.rs:5-26`): `unsplash_api_key` (`UNSPLASH_API_KEY`), `database_url` (`DATABASE_URL`), `sentry_dsn` (`SENTRY_DSN`) via `get_string_env`; `enable_sentry` (`ENABLE_SENTRY`) via `get_bool_env`.
- No defaults exist for any variable. `get_string_env` (`env.rs:29-34`): missing or empty value both panic with `{key} must be set and non-empty`. `get_bool_env` (`env.rs:36-42`): strict case-sensitive `"true"`/`"false"` match; anything else panics with `{key} must be 'true' or 'false', got '{other}'`.
- Failure mode is fail-fast panic at startup — `Env::init()` runs before server bind (`src/main.rs:16` vs `:33`); no retry/fallback/logging beyond the panic message.
- Doc comment claims `.env` support (`env.rs:1-3`) but no dotenv loader call exists in `src/`; `.envrc` uses direnv (shell-level only). `SENTRY_DSN` is required even when `ENABLE_SENTRY=false`.
- Unit tests (`env.rs:44-113`): six tests serialized on a process-wide `static ENV_MUTEX: Mutex<()>` (`env.rs:49-54`) because env vars are global; lock helper recovers from poison via `.unwrap_or_else(|e| e.into_inner())` (`env.rs:59-61`). Tests use dedicated scratch keys (`TEST_GET_ENV_KEY`, `TEST_GET_BOOL_KEY`, `env.rs:56-57`) with `unsafe { std::env::set_var/remove_var }` (Rust ≥2024 unsafe API). Coverage: string happy path, empty-string panic, missing panic, bool true/false, invalid bool panic. `Env::init()` itself untested.

## Q4: Error convention in `src/app/error.rs`

### Findings
- Four variants (`src/app/error.rs:10-14`): `Template(minijinja::Error)`, `Database(sqlx::Error)`, `NotFound`, `External(String)`. `From` impls: minijinja→Template (`:17-21`), sqlx→Database (`:23-27`), `UnsplashError`→External (`:29-33`).
- `IntoResponse` mapping (`error.rs:35-53`):

| Variant | Status | Body | Side effects |
|---|---|---|---|
| `NotFound` | 404 | `"not found"` | none |
| `Database(err)` | 500 | `"internal server error"` | `tracing::error!` + `sentry::capture_error` |
| `Template(err)` | 500 | `"internal server error"` | `tracing::error!` + `sentry::capture_error` |
| `External(msg)` | 502 | `"bad gateway"` | `tracing::error!` only |

- Response bodies are **plain text, not JSON**: each arm returns `(StatusCode, &'static str)` tuples (`Content-Type: text/plain; charset=utf-8`). Confirmed behaviorally by integration test asserting body == `"bad gateway"` (`src/interfaces/handlers/unsplash/json.rs:196-208`). No JSON error envelope exists anywhere.
- Middleware reuse (structural facts only): `WebError` and its variants are `pub`, and it implements `IntoResponse` directly (`error.rs:35`), so code outside handlers can construct a variant and call `.into_response()` to obtain an `axum::response::Response`. There is no extracted standalone body-builder function — the format lives inside the `into_response` match arms. No existing middleware references `WebError`; the only layers are `TraceLayer` (`src/main.rs:42`) and `SetResponseHeader` (`routes.rs:30-35`).
- Asymmetry observed: `External` errors are never sent to Sentry, unlike Database/Template (`error.rs:40-42` vs `:49-52`). `NotFound` is dead in production builds per doc comment (`error.rs:6-8`).

## Q5: Integration test harness (`src/test/mod.rs`)

### Findings
- `start_app()` delegates to `start_app_with("https://api.unsplash.com")` (`src/test/mod.rs:12-14`). The harness builds a real `Env` struct literal (hard-coded: `test-key`, `sqlite::memory:`, `test-dsn`, `enable_sentry: false` — `mod.rs:20-26`), in-memory SQLite + migrations, then binds a real TCP listener on `127.0.0.1:0` and spawns `axum::serve(listener, router.into_make_service())` as a background task (`mod.rs:39-48`).
- Requests come over **real TCP**, not oneshot: `test_client()` returns a plain `reqwest::Client` (`mod.rs:91-93`); typical pattern `start_app().await` → `client.get(format!("http://{addr}/"))`.
- Critical for per-IP keying: test servers use plain `into_make_service()` (`mod.rs:45,79,84`) while production uses `into_make_service_with_connect_info::<SocketAddr>()` (`src/main.rs:43`). With the plain make-service, no `ConnectInfo<SocketAddr>` extension is inserted, so a `ConnectInfo` extractor would fail in every integration test unless the harness wiring changes. Even then, all test traffic originates from 127.0.0.1, so per-IP differentiation across simulated clients is impossible without header-based identity.
- The single `oneshot` test targets only the metrics router (`src/interfaces/routes.rs:75-88`), which is also served without connect-info in production (`src/main.rs:47`).
- Per-test configuration overrides: one knob only — `start_app_with(unsplash_base_url: &str)` (`mod.rs:19`), used to point at a local stub server (`mod.rs:97-135`). Everything else in `Env`/`AppState` is hard-coded struct literals; there is no generic config builder or env-var override hook. `start_app_with` returns the `SqlitePool` so tests can seed rows directly; `start_app_with_metrics()` does not return the pool (`mod.rs:53-88`).
- No rate-limiting code exists anywhere in `src/` today.

## Q6: Infrastructure vs user traffic endpoints

### Findings
- `/health` serves infrastructure probes but lives on the **main user router** sharing full `AppState`: handler at `src/interfaces/routes.rs:15-18` runs `crate::app::db::ping(&state.db)` (`SELECT 1`, `src/app/db.rs:37-39`) and returns bare `StatusCode::OK` or funnels `sqlx::Error` → `WebError::Database` → 500. Registered at `routes.rs:29`. Tests cover 200 and pool-closed 500 (`routes.rs:122-145`).
- `/metrics` is served from a **separate router with separate state**: `metrics_router(Arc<AppMetrics>) -> Router` (`routes.rs:41-45`), state is only the metrics registry — no DB/env/http client. Bound on port 9090 (`src/main.rs:35,45-47`), no trace layer.
- Shared instance, separate routers: `main.rs` creates one `Arc<AppMetrics>` (`src/main.rs:20`), clones it into `AppState.metrics` (`:28`) for page handlers to increment, and passes the same `Arc` to `metrics_router(metrics)` (`:47`).
- Fly.io consumption (`fly.toml`): app `internal_port = 3000` (`fly.toml:12`) with `[[http_service.checks]] path = "/health"`, method GET, interval 30s, timeout 5s, grace 10s (`fly.toml:20-25`); `[metrics] port = 9090, path = "/metrics"` (`fly.toml:28-30`). Machine stop/start depends on the health check (`fly.toml:15-17`). Dockerfile has **no HEALTHCHECK** instruction (ends with `ENTRYPOINT ["/usr/local/bin/vardy"]`, Dockerfile:19-29).

## Q7: Observability patterns in middleware-adjacent components

### Findings
- `trace_layer()` (`src/app/log.rs:62-67`): `TraceLayer::new_for_http()` with custom `make_span_with` (fn pointer), `on_request`/`on_response` at INFO level, classifier `ServerErrorsAsFailures` (5xx = failure). Span named `"http_request"` records exactly two fields: `method` and low-cardinality `path` from the `MatchedPath` extension (falls back to raw URI path if unmatched) (`log.rs:69-80`). No status/latency/request-ID fields in the span; those appear only in tower-http's default response event message. Applied only to the main router (`src/main.rs:42`); logging output is JSON to stderr (`log.rs:19-56`).
- Metrics (`src/infra/metrics.rs:4-31`): a private `prometheus::Registry` holding exactly **one** collector — `page_views_total`, an `IntCounterVec` labeled `["page"]` (`metrics.rs:9-13`). API: `inc_page_view(&self, page)` (`:22-24`) and `render()` via `TextEncoder` (`:26-33`). Incremented only by home and singlethread handlers (`handlers/home/web.rs:8`, `handlers/singlethread/web.rs:8`). No HTTP-level metrics (request count/duration histograms) exist.
- Rate-limit conventions in this stack's dependencies: **cannot be determined from the codebase.** Zero matches for `governor`/`tower-governor`/any rate-limit crate in Cargo.toml or Cargo.lock; no occurrences of `Retry-After`, `X-RateLimit*`, or rate-limit logic anywhere in `src/`. Neither crate is vendored in the local cargo registry cache, so their documented header conventions could not be inspected here. The only response-header manipulation in the repo is `Cache-Control` for static assets (`routes.rs:30-37`).

## Cross-Cutting Observations

- Two-server architecture throughout: everything user-facing (pages, `/dump`, `/unsplash`, `/health`, static files) on port 3000 behind `TraceLayer` + connect-info make-service; observability (`/metrics`) isolated on port 9090 with minimal state and no layers (`src/main.rs:37-49`).
- Production and test wiring diverge: prod attaches `trace_layer` and `into_make_service_with_connect_info` (`src/main.rs:42-43`); the test harness attaches neither and uses plain `into_make_service()` (`src/test/mod.rs:45,79`). No test exercises the production make-service chain.
- All handler errors flow through `WebError`'s `IntoResponse` impl; bodies are static plain-text strings, and the Sentry-capture side effect lives inside that impl.
- Client identity plumbing exists but is unused: peer address reaches requests as a `ConnectInfo<SocketAddr>` extension in prod only; nothing extracts it, and no proxy headers (`X-Forwarded-For`) are handled anywhere — relevant given deployment behind Fly.io's proxy.
- Configuration is fail-fast panics with zero defaults; the only config-injection seam in tests is `start_app_with(unsplash_base_url)`.

## Open Areas

- Conventional rate-limit response headers (`Retry-After`, `X-RateLimit-*`) used by common Rust limiter crates could not be verified from this repository or the local cargo registry cache (no such crate present locally).
- Effective contents of `tower-http 0.6.11` default features were not enumerated (defaults not disabled in Cargo.toml:16).
- Origin of the legacy `http 0.2.12` entry in Cargo.lock (:1140) was not traced; relevant only if a new dependency targeted http 0.2.
