# Implementation Plan

## Overview

Add a Prometheus `/metrics` endpoint served by a dedicated second listener on port 9090 (own router, own state), an `AppMetrics` registry-owner type with a `page_views_total{page}` CounterVec incremented inside the home and singlethread handlers, and a Fly.io `[metrics]` scrape block — mirroring the sibling `api` repo's observability idiom.

**Reference implementations to copy from** (verified paths):
- `~/dev/api/src/infra/metrics.rs` — `AppMetrics` struct
- `~/dev/api/src/main.rs:47-63` — two-listener `tokio::try_join!`
- `~/dev/api/src/interfaces/routes.rs:29-33` — `metrics_router`
- `~/dev/api/src/interfaces/handlers/metrics.rs` — `metrics_handler`
- `~/dev/api/fly.toml:34-36` — `[metrics]` block

---

## Phase 1: `/metrics` endpoint on dedicated port 9090

### Changes

#### 1. Add dependency
**File**: `Cargo.toml`
**Action**: modify — add to `[dependencies]`:

```toml
prometheus = { version = "0.14", default-features = false }
```

Then run `cargo check` (or any cargo command) so `Cargo.lock` picks up the new dep. **Commit `Cargo.lock` together with `Cargo.toml`** — CI runs clippy with `--locked`.

#### 2. New `infra` module root
**File**: `src/infra/mod.rs`
**Action**: create

```rust
pub mod metrics;
```

#### 3. Register the module
**File**: `src/main.rs`
**Action**: modify — add `mod infra;` alongside the existing declarations at the top (vardy has no `lib.rs`; all modules are declared in `main.rs`):

```rust
mod app;
mod infra;
mod interfaces;
```

#### 4. `AppMetrics` type
**File**: `src/infra/metrics.rs`
**Action**: create — mirrors `api/src/infra/metrics.rs:4-31`, minus the counters that don't apply yet (they arrive in Phase 2):

```rust
use prometheus::{Encoder, Registry, TextEncoder};

pub struct AppMetrics {
    registry: Registry,
}

impl AppMetrics {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();
        Ok(Self { registry })
    }

    pub fn render(&self) -> String {
        let encoder = TextEncoder::new();
        let mut buf = Vec::new();
        encoder.encode(&self.registry.gather(), &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }
}
```

(`render()` mirrors api verbatim; the `unwrap`s are safe — encoding into a `Vec` and UTF-8 conversion of encoder output cannot fail.)

#### 5. Metrics router
**File**: `src/interfaces/routes.rs`
**Action**: modify — add a free function below `routes()` (mirrors `api/src/interfaces/routes.rs:29-33`). Note it has its **own** state type, so it is not part of `routes()`:

```rust
/// Router for the dedicated metrics port; owns its own state.
pub fn metrics_router(metrics: std::sync::Arc<crate::infra::metrics::AppMetrics>) -> Router {
    Router::new()
        .route("/metrics", get(handlers::metrics::web::metrics_handler))
        .with_state(metrics)
}
```

(Use whatever import style matches the file after fmt — `use std::sync::Arc;` at top is fine.)

#### 6. Handler modules
**File**: `src/interfaces/handlers/mod.rs`
**Action**: modify — add one line:

```rust
pub mod metrics;
```

**File**: `src/interfaces/handlers/metrics/mod.rs`
**Action**: create (follows the `handlers/<feature>/mod.rs` convention containing `pub mod web;`):

```rust
pub mod web;
```

#### 7. Metrics handler
**File**: `src/interfaces/handlers/metrics/web.rs`
**Action**: create — mirrors `api/src/interfaces/handlers/metrics.rs`:

```rust
use axum::{extract::State, http::header::CONTENT_TYPE, response::IntoResponse};
use prometheus::TextEncoder;
use std::sync::Arc;

use crate::infra::metrics::AppMetrics;

/// GET /metrics — returns Prometheus text exposition format.
pub async fn metrics_handler(State(metrics): State<Arc<AppMetrics>>) -> impl IntoResponse {
    let content_type = TextEncoder::new().format_type().to_owned();
    let body = metrics.render();
    ([(CONTENT_TYPE, content_type)], body)
}
```

#### 8. Second listener in `main`
**File**: `src/main.rs`
**Action**: modify — construct `AppMetrics`, bind port 9090, serve both routers via `tokio::try_join!` (mirrors `api/src/main.rs:47-63`, adapted to this repo's simpler startup). Replace the current serve block:

```rust
let metrics = std::sync::Arc::new(infra::metrics::AppMetrics::new()?);
let state = app::state::AppState {
    templates: app::templates::init(),
    db: app::db::init(&database_url).await,
};
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
println!("Hosting on http://localhost:3000");
let metrics_listener = tokio::net::TcpListener::bind("0.0.0.0:9090").await?;
tokio::try_join!(
    axum::serve(
        listener,
        interfaces::routes::routes()
            .with_state(state)
            .into_make_service_with_connect_info::<std::net::SocketAddr>(),
    ),
    axum::serve(
        metrics_listener,
        interfaces::routes::metrics_router(metrics).into_make_service(),
    ),
)?;
Ok(())
```

Notes:
- `AppMetrics::new()?` works because `main` already returns `Result<(), Box<dyn std::error::Error>>` and `prometheus::Error` implements `std::error::Error`.
- Keep `into_make_service_with_connect_info` on the public router exactly as today; plain `into_make_service()` suffices for the metrics router (no connect-info needed).
- Do not thread `metrics` into `AppState` yet — that's Phase 2.

#### 9. In-process test for the metrics router
**File**: `src/interfaces/routes.rs`
**Action**: modify — add a test to the existing `#[cfg(test)] mod tests`, mirroring `api/src/interfaces/routes.rs:244-262`. This is the first `oneshot` test in the repo (no socket needed):

```rust
#[tokio::test]
async fn metrics_router_serves_metrics_endpoint() {
    use crate::infra::metrics::AppMetrics;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;

    let metrics = Arc::new(AppMetrics::new().expect("test metrics"));
    let router = super::metrics_router(metrics);

    let response = router
        .oneshot(Request::builder().uri("/metrics").body(Body::empty()).unwrap())
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(content_type.contains("text/plain; version=0.0.4"));
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    // Empty registry still emits the prometheus version header line.
    assert!(String::from_utf8(body.to_vec()).unwrap().is_empty());
}
```

(The exact final body assertion can be adjusted once observed — an empty registry renders as empty text; the status + content-type checks are the load-bearing assertions.)

### Verification

#### Automated
- [x] `cargo nextest run` passes, including new `metrics_router_serves_metrics_endpoint`
- [x] `cargo clippy --all-targets --all-features --locked -D warnings` clean (also validates Cargo.lock is committed)
- [x] `cargo fmt --check` clean

#### Manual
- [ ] `cargo run`, then `curl -i localhost:9090/metrics` returns 200 with `content-type: text/plain; version=0.0.4` and Prometheus exposition text (empty registry → effectively empty body)
- [ ] `curl localhost:3000/health` still returns 200; `curl localhost:3000/` still serves HTML

---

## Phase 2: Page-view counters in handlers

### Changes

#### 1. `AppState` gains a metrics field (the one deliberate deviation from api)
**File**: `src/app/state.rs`
**Action**: modify:

```rust
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub templates: minijinja::Environment<'static>,
    /// Unused until the first handler query lands; mirrors the
    /// `#[allow(dead_code)]` precedent on `WebError::NotFound`.
    #[allow(dead_code)]
    pub db: sqlx::SqlitePool,
    pub metrics: Arc<crate::infra::metrics::AppMetrics>,
}
```

(`Arc<AppMetrics>` clones cheaply per request; the `Clone` derive keeps working.)

#### 2. Counter registration + helper
**File**: `src/infra/metrics.rs`
**Action**: modify — add an `IntCounterVec` labelled by `page`, register it, and expose a thin increment helper so handlers never touch the CounterVec directly:

```rust
use prometheus::{Encoder, IntCounterVec, Opts, Registry, TextEncoder};

pub struct AppMetrics {
    registry: Registry,
    page_views_total: IntCounterVec,
}

impl AppMetrics {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();
        let page_views_total = IntCounterVec::new(
            Opts::new("page_views_total", "Total number of page views"),
            &["page"],
        )?;
        registry.register(Box::new(page_views_total.clone()))?;
        Ok(Self {
            registry,
            page_views_total,
        })
    }

    pub fn inc_page_view(&self, page: &str) {
        self.page_views_total.with_label_values(&[page]).inc();
    }

    // render() unchanged from Phase 1
}
```

#### 3. Update both construction sites
**File**: `src/main.rs`
**Action**: modify — move construction above the state literal (it's already there from Phase 1) and add the field:

```rust
let metrics = std::sync::Arc::new(infra::metrics::AppMetrics::new()?);
let state = app::state::AppState {
    templates: app::templates::init(),
    db: app::db::init(&database_url).await,
    metrics: metrics.clone(),
};
```

(the later `metrics_router(metrics)` call then takes ownership of the outer `Arc`; clone order keeps both alive).

**File**: `src/test/mod.rs`
**Action**: modify — same field in the test state literal:

```rust
let state = crate::app::state::AppState {
    templates: crate::app::templates::init(),
    db: crate::app::db::init("sqlite::memory:").await,
    metrics: std::sync::Arc::new(crate::infra::metrics::AppMetrics::new().expect("metrics")),
};
```

No other construction sites exist (research Q4: exactly two).

#### 4. Increment in page handlers
**File**: `src/interfaces/handlers/home/web.rs` and `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify — one line at the top of each `index`:

```rust
pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    state.metrics.inc_page_view("home"); // "singlethread" in the other file
    let html = state.templates.get_template("home.html")?...
```

#### 5. Unit test: counter delta
**File**: `src/infra/metrics.rs`
**Action**: modify — append a test module, asserting the +1.0 delta like api's heartbeat tests (`get()` before/after):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inc_page_view_increments_counter() {
        let metrics = AppMetrics::new().expect("metrics");
        let initial = metrics
            .page_views_total
            .with_label_values(&["home"])
            .get();
        metrics.inc_page_view("home");
        assert_eq!(
            metrics.page_views_total.with_label_values(&["home"]).get(),
            initial + 1.0
        );
    }
}
```

(If clippy prefers, route the reads through a small `#[cfg(test)] fn page_view_count(&self, page: &str) -> f64` accessor instead of touching the private field — either is fine.)

#### 6. Integration test: end-to-end through both routers
**File**: `src/test/mod.rs`
**Action**: modify — the existing `start_app()` serves only the main router, so add a full-stack helper that reproduces the two-listener production topology on ephemeral ports (existing `start_app` and its call sites stay untouched):

```rust
/// Like `start_app`, but also serves the metrics router; returns (app_addr, metrics_addr).
pub async fn start_app_with_metrics() -> (SocketAddr, SocketAddr) {
    let state = crate::app::state::AppState {
        templates: crate::app::templates::init(),
        db: crate::app::db::init("sqlite::memory:").await,
        metrics: std::sync::Arc::new(
            crate::infra::metrics::AppMetrics::new().expect("metrics"),
        ),
    };
    let app_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let app_addr = app_listener.local_addr().expect("local addr");
    let metrics_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let metrics_addr = metrics_listener.local_addr().expect("local addr");
    let router: Router =
        crate::interfaces::routes::routes().with_state(state.clone());
    let metrics_router = crate::interfaces::routes::metrics_router(state.metrics.clone());
    tokio::spawn(async move {
        axum::serve(app_listener, router.into_make_service())
            .await
            .expect("server");
    });
    tokio::spawn(async move {
        axum::serve(metrics_listener, metrics_router.into_make_service())
            .await
            .expect("server");
    });
    (app_addr, metrics_addr)
}
```

**File**: put the test wherever it fits best — recommended: bottom of `src/test/mod.rs` or in `src/interfaces/routes.rs` tests. Content:

```rust
#[tokio::test]
async fn page_hits_show_up_in_metrics() {
    let (app_addr, metrics_addr) = crate::test::start_app_with_metrics().await;
    let client = test_client();
    client.get(format!("http://{app_addr}/")).send().await.unwrap();
    client.get(format!("http://{app_addr}/")).send().await.unwrap();

    let res = client
        .get(format!("http://{metrics_addr}/metrics"))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);
    let body = res.text().await.unwrap();
    assert!(body.contains("page_views_total"));
    assert!(body.contains(r#"page="home""#));
}
```

### Verification

#### Automated
- [x] `cargo nextest run` passes, including new `inc_page_view_increments_counter` and `page_hits_show_up_in_metrics`
- [x] All existing tests still pass (both `AppState` construction sites updated — no compile breakage)
- [x] `cargo clippy --all-targets --all-features --locked -D warnings` clean
- [x] `cargo fmt --check` clean

#### Manual
- [ ] `cargo run`; hit `localhost:3000/` and `localhost:3000/singlethread` a few times; `curl localhost:9090/metrics` shows `page_views_total` with growing counts under correct `page="home"` / `page="singlethread"` labels

---

## Phase 3: Fly.io scrape configuration

### Changes

#### 1. Scrape block
**File**: `fly.toml`
**Action**: modify — append at the end of the file (mirrors `api/fly.toml:34-36`):

```toml
[metrics]
  port = 9090
  path = "/metrics"
```

No Rust, Dockerfile, workflow, or health-check changes. (Dockerfile needs no `EXPOSE 9090` — Fly ignores EXPOSE.)

### Verification

#### Automated
- [x] None applies — config-only phase. CI's existing deploy flow (`flyctl deploy --remote-only` in `.github/workflows/fly-deploy.yml`) implicitly validates that fly.toml parses.

#### Manual
- [ ] After merge-to-main deploy, open the Fly dashboard → vardy app → Metrics tab and confirm it populates. Note design risk: a grace period may apply on first scrape; if empty, wait and re-check before investigating.

---

## Testing Checkpoints

- **After Phase 1**: `curl localhost:9090/metrics` returns valid Prometheus text; port 3000 routes untouched and passing; fmt/clippy/nextest green.
- **After Phase 2**: `page_views_total` appears at `/metrics` and increments on page hits; both `AppState` construction sites updated.
- **After Phase 3**: deployed service scraped by Fly; metrics tab populated.

## Notes

- Commit `Cargo.toml` + `Cargo.lock` together (CI clippy runs `--locked`).
- Phase ordering matters only in that Phase 2 builds on Phase 1's types; do not reorder phases.
- Phase 3 has no automated checkpoint by nature — flagged, not fixable (pure deployment config).
