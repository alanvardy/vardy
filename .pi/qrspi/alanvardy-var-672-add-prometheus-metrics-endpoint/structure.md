# Structure Outline

## Approach
Mirror the sibling `api` repo's observability idiom: a `prometheus`-crate
`AppMetrics` registry-owner served on a dedicated port 9090 via a second
router and `tokio::try_join!`, with explicit `counter.inc()` calls in page
handlers and a Fly.io `[metrics]` scrape block. Three slices, each
end-to-end testable: endpoint → instrumentation → deployment.

---

## Phase 1: `/metrics` endpoint on dedicated port 9090

Delivers a working Prometheus text-format endpoint served by a second
listener, with an empty-but-valid registry. Nothing scrapes it yet, but
`curl localhost:9090/metrics` works end-to-end.

**Files**: `Cargo.toml`, `Cargo.lock`, `src/infra/mod.rs` (new),
`src/infra/metrics.rs` (new), `src/main.rs`, `src/lib.rs` or module decls,
`src/interfaces/routes.rs`, `src/interfaces/handlers/mod.rs` (new),
`src/interfaces/handlers/metrics/web.rs` (new)

**Key changes**:
- `prometheus = { version = "0.14", default-features = false }` — new dep (commit lockfile with it)
- `pub struct AppMetrics { registry: Registry, ... }` — new type
- `AppMetrics::new() -> Result<Self, prometheus::Error>` — constructor
- `AppMetrics::render(&self) -> String` — `TextEncoder` over `registry.gather()`
- `pub fn metrics_router(metrics: Arc<AppMetrics>) -> Router` — own state, `.route("/metrics", get(...))`
- `pub async fn metrics_handler(State(metrics): State<Arc<AppMetrics>>) -> impl IntoResponse` — `(CONTENT_TYPE, render())`
- `main()` — bind `0.0.0.0:9090`, serve both routers via `tokio::try_join!`

**Verify**:
- `cargo nextest run` — new in-process `oneshot` test: GET `/metrics` on
  `metrics_router` returns 200, content-type contains
  `text/plain; version=0.0.4` (mirrors `api/src/interfaces/routes.rs:244-262`)
- `cargo clippy --all-targets --all-features --locked -D warnings` clean
- Manual: `cargo run`, then `curl localhost:9090/metrics` shows Prometheus
  exposition text; `curl localhost:3000/health` still returns 200

---

## Phase 2: Page-view counters in handlers

Delivers the actual signal: `page_views_total{page="home"|"singlethread"}`
increments on each page hit and is visible at `/metrics`. `AppState` gains
one field — the only deliberate deviation from `api`'s pattern, per design
decision 4.

**Files**: `src/app/state.rs`, `src/main.rs`, `src/test/mod.rs`,
`src/interfaces/handlers/home/web.rs`, `src/interfaces/handlers/singlethread/web.rs`,
`src/infra/metrics.rs`, `src/infra/metrics.rs` (tests)

**Key changes**:
- `pub struct AppState { ..., pub metrics: Arc<AppMetrics> }` — field added
- `page_views_total: IntCounterVec` (label `page`) registered in `AppMetrics::new`
- `AppMetrics::inc_page_view(&self, page: &str)` — thin helper so handlers don't touch the CounterVec directly
- Both construction sites updated: `metrics: Arc::new(AppMetrics::new().expect(...))` in `main.rs:8-11` and `test/mod.rs:6-10`
- `state.metrics.inc_page_view("home")` / `("singlethread")` at top of each handler

**Verify**:
- `cargo nextest run` — new tests: (a) unit test asserting
  `inc_page_view` delta of +1.0 via `get()` before/after (api heartbeat-test
  style); (b) existing reqwest harness test: hit `/` via `start_app()`, GET
  `/metrics` body `contains("page_views_total")`
- Manual: `cargo run`, hit `/` and `/singlethread` a few times, `curl
  localhost:9090/metrics` shows growing counts with correct labels

---

## Phase 3: Fly.io scrape configuration

Delivers production visibility: Fly auto-scrapes the private metrics port.
No Rust code changes.

**Files**: `fly.toml`

**Key changes**:
- Append:
  ```toml
  [metrics]
    port = 9090
    path = "/metrics"
  ```

**Verify**:
- No automated test applies (config-only); confirm `fly.toml` parses by a
  successful `fly deploy` (or `flyctl status`) in CI's existing deploy flow
- Manual: after first deploy to main, check the Fly dashboard metrics tab
  populates (note design risk: grace period may apply on first scrape)

---

## Testing Checkpoints
- **After Phase 1**: `curl localhost:9090/metrics` returns valid Prometheus
  text; port 3000 routes untouched and passing; clippy/nextest/fmt green.
- **After Phase 2**: `page_views_total` appears at `/metrics` and increments
  on page hits; both `AppState` construction sites updated (no compile
  breakage in tests).
- **After Phase 3**: deployed service scraped by Fly; metrics tab populated.

## Notes
- All three phases are fully vertical; nothing in the design requires a
  horizontal layer pass. The `Cargo.lock` must be committed alongside
  `Cargo.toml` in Phase 1 (`--locked` CI constraint).
- Phase 3 has no automated checkpoint by nature — flagged, not fixable,
  since it's pure deployment config.
