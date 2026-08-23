# Implementation Plan

## Overview

Every request to :3000 passes through a global per-IP GCRA limiter keyed by
`Fly-Client-IP` (with `ConnectInfo` fallback); `POST /dump/{key}` and
`GET /unsplash` additionally pass a stricter hard-coded tier. Over-limit
requests get **429** with plain-text body `"too many requests"`, a
`Retry-After` header, and `X-RateLimit-*` headers, funneled through the
`WebError` chokepoint. Limits come from required env vars
(`RATE_LIMIT_PER_MS`, `RATE_LIMIT_BURST`). `/metrics` (:9090) stays unlimited.
A background task prunes the limiter store every 60 s.

Conventions carried through the whole feature:

- `RATE_LIMIT_PER_MS` semantic (matches `../api`): *milliseconds between
  replenishing one token* — `per_millisecond(N)` in governor terms. So
  `RATE_LIMIT_PER_MS=100` ⇒ 10 req/s sustained.
- All 429 bodies come from `WebError::TooManyRequests`'s `IntoResponse` arm —
  middleware and handlers share one format.
- The test harness keeps limiting effectively disabled (`per_ms: 1`,
  `burst: 1_000_000`) except in dedicated 429 tests.

---

## Phase 1: Global per-IP limiter, wired and invisible to existing tests

### Changes

#### 1. Dependencies
**File**: `Cargo.toml`
**Action**: modify

Add to `[dependencies]`:

```toml
tower-governor = { version = "0.8", features = ["axum"] }
governor = "0.10"
```

`governor` is needed directly for the pruner types (`QuantaInstant`,
`RateLimitingMiddleware`) in Phase 4. After adding, run `cargo tree -i
governor` to confirm a single resolved governor version matching what
tower-governor 0.8 pulls; if they mismatch, align our explicit version to
tower-governor's requirement rather than adding a second major version.

#### 2. Module registration
**File**: `src/app/mod.rs`
**Action**: modify

Add `pub mod rate_limit;` to the module list (alphabetical, after `picture`).

#### 3. Rate-limit module with key extractor
**File**: `src/app/rate_limit.rs`
**Action**: create

Clone of `../api/src/app/rate_limit.rs` minus the auth helper and minus the
pruner spawn (added in Phase 4):

```rust
use std::net::SocketAddr;

use axum::{Router, extract::ConnectInfo};
use tower_governor::{
    GovernorLayer,
    governor::GovernorConfigBuilder,
    key_extractor::KeyExtractor,
};

use crate::app::state::AppState;

/// Reads `Fly-Client-IP` set by the Fly Proxy, which cannot be spoofed.
/// Falls back to the TCP peer address for local development.
///
/// `X-Forwarded-For` is deliberately ignored because Fly Proxy appends to it,
/// making it trivially spoofable by clients.
#[derive(Clone)]
pub struct FlyClientIpKeyExtractor;

impl KeyExtractor for FlyClientIpKeyExtractor {
    type Key = std::net::IpAddr;

    fn extract<T>(&self, req: &axum::http::Request<T>) -> Result<Self::Key, GovernorError> {
        if let Some(ip) = req
            .headers()
            .get("fly-client-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
        {
            return Ok(ip);
        }

        req.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0.ip())
            .ok_or(GovernorError::UnableToExtractKey)
    }
}

/// Apply a global per-IP rate limiter to the router.
pub fn with_global_limit(router: Router<AppState>, per_ms: u64, burst: u32) -> Router<AppState> {
    let governor_cfg = std::sync::Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(FlyClientIpKeyExtractor)
            .per_millisecond(per_ms)
            .burst_size(burst)
            .use_headers()
            .finish()
            .expect("rate-limit config must be valid"),
    );

    router.layer(GovernorLayer::new(governor_cfg))
}
```

(`GovernorError` import needed: `tower_governor::GovernorError`.)

Unit tests inline at the bottom (`#[cfg(test)] mod tests`):

```rust
#[test]
fn extracts_fly_client_ip()               // header "fly-client-ip: 1.2.3.4" -> Ok(IpAddr)
#[test]
fn ignores_x_forwarded_for()              // only "x-forwarded-for" present -> Err(_)
#[test]
fn prefers_fly_client_ip_over_xff()       // both headers -> fly-client-ip wins
#[test]
fn falls_back_to_connect_info()           // no headers; insert ConnectInfo<SocketAddr>
                                          // extension manually into the request -> Ok(peer ip)
#[test]
fn errors_when_no_key_available()         // no headers, no extension -> Err(UnableToExtractKey)
```

Build bare requests with `Request::builder().body(())`; for the fallback test
insert the extension before building:
`Request::builder().extension(ConnectInfo("127.0.0.1:8080".parse().unwrap())).body(())`.

#### 4. Env vars
**File**: `src/app/env.rs`
**Action**: modify

New fields on `Env`:

```rust
pub rate_limit_per_ms: u64,   // milliseconds between replenishing one token
pub rate_limit_burst: u32,
```

New generic parser next to `get_string_env` (fail-fast, matches house style):

```rust
fn get_parse_env<T: std::str::FromStr>(key: &str) -> T {
    let raw = get_string_env(key);
    raw.parse().unwrap_or_else(|_| panic!("{key} must be a valid integer, got '{raw}'"))
}
```

In `Env::init()`:

```rust
let rate_limit_per_ms = get_parse_env::<u64>("RATE_LIMIT_PER_MS");
let rate_limit_burst = get_parse_env::<u32>("RATE_LIMIT_BURST");
```

Tests (under the existing `ENV_MUTEX` serialization):
- happy: `set_var(KEY, "100")` → parses to `100u64`
- invalid: `set_var(KEY, "abc")` → `#[should_panic(expected = "must be a valid integer")]`
- missing: `remove_var(KEY)` → panics with `"must be set and non-empty"` (delegates to `get_string_env`)

Use scratch keys (`TEST_GET_PARSE_KEY`).

#### 5. Wire the global limiter in production
**File**: `src/main.rs`
**Action**: modify

Reorder so the governor layer wraps the traced router *before*
`with_state` (the limiter needs `Router<AppState>`), keeping trace outermost→
governor→handler order irrelevant here since governor ends up outermost:

```rust
let router = interfaces::routes::routes().layer(app::log::trace_layer());
let router = app::rate_limit::with_global_limit(
    router,
    env.rate_limit_per_ms,
    env.rate_limit_burst,
);
// ... build state AFTER this (env fields are Copy, read them first or move state below)
tokio::try_join!(
    axum::serve(
        listener,
        router.with_state(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    ),
    axum::serve(metrics_listener, interfaces::routes::metrics_router(metrics).into_make_service()),
)?;
```

Copy `per_ms`/`burst` into locals before `env` is moved into `AppState`.
The :9090 metrics server is untouched.

#### 6. Test harness: connect-info + disabled limits
**File**: `src/test/mod.rs`
**Action**: modify

- In **both** `start_app_with` and `start_app_with_metrics`: replace
  `router.into_make_service()` with
  `router.into_make_service_with_connect_info::<std::net::SocketAddr>()`
  (mirrors production; required so the extractor's fallback works).
- Add the two new fields to both hard-coded `Env` literals:

```rust
rate_limit_per_ms: 1,
rate_limit_burst: 1_000_000,
```

This keeps every existing integration suite green (bucket never exhausts).
No changes to the Unsplash stub server (unlimited by design).

### Verification

#### Automated
- [x] `./scripts/test.sh` passes (format, sqlx prepare, check, clippy `-D warnings`, nextest, TODO grep)
- [x] `cargo nextest run rate_limit` — new extractor unit tests pass (happy + sad)
- [x] `cargo nextest run env::tests` — new parse-helper tests pass
- [x] All pre-existing suites green (limits disabled in harness)

#### Manual
- [ ] Add `RATE_LIMIT_PER_MS=100` and `RATE_LIMIT_BURST=20` to local `.env`;
      boot with `cargo run` and hammer one bucket in fish:

      ```fish
      for i in (seq 1 40); curl -s -o /dev/null -w "%{http_code}\n" -H "fly-client-ip: 1.2.3.4" http://localhost:3000/health; end
      ```

      Expected: first ~20 requests `200`, then `429`s appear once burst is spent.
- [ ] A different bucket still succeeds concurrently:
      `curl -i -H "fly-client-ip: 9.9.9.9" http://localhost:3000/health` → `200`.
- [ ] Response carries standard headers (`x-ratelimit-limit`, `x-ratelimit-remaining`,
      `retry-after` on 429s).
- [ ] Fresh shell with either var unset: `cargo run` panics at startup with
      `RATE_LIMIT_PER_MS must be set and non-empty`.

---

## Phase 2: 429 through the WebError chokepoint + integration proof

### Changes

#### 1. New `WebError` variant
**File**: `src/app/error.rs`
**Action**: modify

Add variant (keep `#[derive(Debug)]`):

```rust
TooManyRequests {
    retry_after_secs: u64,
},
```

Add match arm in `IntoResponse`. No Sentry capture (client fault — mirrors
`External`). Header value built as owned string via the tuple form:

```rust
WebError::TooManyRequests { retry_after_secs } => (
    StatusCode::TOO_MANY_REQUESTS,
    [("retry-after", retry_after_secs.to_string())],
    "too many requests",
)
    .into_response(),
```

Unit tests in the inline `mod tests`:

```rust
#[test]
fn too_many_requests_is_429_with_body_and_retry_after() {
    let res = WebError::TooManyRequests { retry_after_secs: 7 }.into_response();
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    // body == b"too many requests" (to_bytes)
    // res.headers().get("retry-after") == Some("7")
}
```

(Existing tests stay as-is.)

#### 2. Custom governor error responder
**File**: `src/app/rate_limit.rs`
**Action**: modify

`tower-governor 0.8` exposes `GovernorConfigBuilder::error_handler(Fn(GovernorError) -> Response)`
— confirmed available, so the design's fallback wrapper layer is NOT needed.
Funnel through the chokepoint:

```rust
use axum::response::IntoResponse;
use tower_governor::GovernorError;
use crate::app::error::WebError;

fn rate_limit_error_response(err: GovernorError) -> axum::response::Response {
    match err {
        GovernorError::TooManyRequests { wait_time, headers } => {
            let mut response =
                WebError::TooManyRequests { retry_after_secs: wait_time }.into_response();
            if let Some(headers) = headers {
                for (name, value) in headers.into_iter().flatten() {
                    response.headers_mut().insert(name, value);
                }
            }
            response
        }
        // Unreachable with our extractor (header or ConnectInfo always present),
        // but keep it total and logged.
        other => {
            tracing::error!(?other, "rate limiter failed to extract key");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error",
            )
                .into_response()
        }
    }
}
```

Attach in `with_global_limit` (and later in the tier helper):

```rust
GovernorConfigBuilder::default()
    .key_extractor(FlyClientIpKeyExtractor)
    .per_millisecond(per_ms)
    .burst_size(burst)
    .use_headers()
    .error_handler(rate_limit_error_response)
    .finish()
    .expect("rate-limit config must be valid")
```

If `error_handler` turns out to differ in 0.8's actual signature (design risk),
fall back to the thin rejection-mapping wrapper layer around limited routers
that maps `Result<Response, GovernorError>` rejections via the same
`rate_limit_error_response` builder — do not hand-roll token logic.

Unit test in `rate_limit.rs` tests module:

```rust
#[test]
fn too_many_requests_error_maps_to_web_error_shape() {
    let res = rate_limit_error_response(GovernorError::TooManyRequests { wait_time: 5, headers: None });
    assert_eq!(res.status(), 429);
    // body == "too many requests"
}
```

#### 3. Tight-limit harness entry point
**File**: `src/test/mod.rs`
**Action**: modify

Refactor minimally: extract the shared body of `start_app_with` into one
private helper parameterized on limits; both public builders delegate to it
(one extra function, no config-builder sprawl):

```rust
async fn serve_app(unsplash_base_url: &str, per_ms: u64, burst: u32) -> (SocketAddr, SqlitePool) {
    let env = Env { /* existing literal */, rate_limit_per_ms: per_ms, rate_limit_burst: burst };
    // ... existing db/migrate/state/listener code ...
    let router: Router = {
        let r = crate::interfaces::routes::routes();
        let r = crate::app::rate_limit::with_global_limit(r, per_ms, burst);
        r.with_state(state)
    };
    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service_with_connect_info::<std::net::SocketAddr>())
            .await
            .expect("server");
    });
    (addr, db)
}

pub async fn start_app_with(url: &str) -> (SocketAddr, SqlitePool) {
    serve_app(url, 1, 1_000_000).await
}

/// Tight-limit harness for 429 integration tests.
pub async fn start_app_with_rate_limits(
    unsplash_base_url: &str,
    per_ms: u64,
    burst: u32,
) -> (SocketAddr, SqlitePool) {
    serve_app(unsplash_base_url, per_ms, burst).await
}
```

Deviation from structure.md's two-parameter signature: the base-url parameter
is kept so Phase 3's unsplash tier tests can run against the existing
UnsplashStub instead of the live API — avoids inventing a third builder.

`start_app_with_metrics` keeps its own literal (add the two fields, disabled
values) and is not limit-wrapped.

#### 4. Integration tests
**File**: `src/interfaces/routes.rs` (tests module)
**Action**: modify

Happy + sad, asserting status AND body AND header (house rule):

```rust
#[tokio::test]
async fn under_limit_request_is_not_rate_limited() {
    let (addr, _pool) = crate::test::start_app_with_rate_limits("https://api.unsplash.com", 1_000, 2).await;
    let res = test_client().get(format!("http://{addr}/health")).send().await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn over_limit_requests_get_429_with_exact_body_and_retry_after() {
    let (addr, _pool) = crate::test::start_app_with_rate_limits("https://api.unsplash.com", 1_000, 2).await;
    let client = test_client();
    // burst 2, refill 1 token/sec: 10 rapid sequential requests must trip it
    let mut saw_429 = false;
    for _ in 0..10 {
        let res = client.get(format!("http://{addr}/health")).send().await.unwrap();
        match res.status() {
            StatusCode::TOO_MANY_REQUESTS => {
                saw_429 = true;
                assert_eq!(res.text().await.unwrap(), "too many requests");
                assert!(res.headers().get("retry-after").is_some());
            }
            StatusCode::OK => {}
            status => panic!("unexpected status {status}"),
        }
    }
    assert!(saw_429, "expected at least one 429 within 10 rapid requests");
}
```

All traffic comes from 127.0.0.1 and the key is the IP (not port), so the
single bucket fills deterministically. Each test boots its own app/store, so
tests stay isolated.

### Verification

#### Automated
- [x] `./scripts/test.sh` passes including the three new tests
- [x] `cargo nextest run error::tests` — `TooManyRequests` mapping unit tests pass
- [x] `cargo nextest run over_limit\|under_limit` — 429 status + exact body + `Retry-After` asserted over real TCP

#### Manual
- [ ] Boot locally with a small budget (e.g. `RATE_LIMIT_PER_MS=500`, `RATE_LIMIT_BURST=3`),
      curl past the limit, confirm exact body text `too many requests` and the
      `retry-after` header value is a plain integer.

---

## Phase 3: Stricter tiers for dump + unsplash

### Changes

#### 1. Tier constants + nested-router helper
**File**: `src/app/rate_limit.rs`
**Action**: modify

Hard-coded policy constants (tuned like api's auth tier; adjust freely during
implementation, keep them in one place):

```rust
/// Stricter budgets for expensive endpoints. Policy lives in code, not config.
pub const DUMP_TIER_PER_MS: u64 = 1_000;       // 1 write/sec sustained
pub const DUMP_TIER_BURST: u32 = 3;
pub const UNSPLASH_TIER_PER_MS: u64 = 200;     // 5 upstream calls/sec sustained
pub const UNSPLASH_TIER_BURST: u32 = 5;
```

Helper (cf. api's `auth_routes()`):

```rust
/// Wrap a route group with its own tighter per-IP budget, nested inside the
/// global limiter.
pub fn tiered_routes(limited: Router<AppState>, per_ms: u64, burst: u32) -> Router<AppState> {
    let governor_cfg = std::sync::Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(FlyClientIpKeyExtractor)
            .per_millisecond(per_ms)
            .burst_size(burst)
            .use_headers()
            .error_handler(rate_limit_error_response)
            .finish()
            .expect("tier rate-limit config must be valid"),
    );
    limited.layer(GovernorLayer::new(governor_cfg))
}
```

#### 2. Route registration
**File**: `src/interfaces/routes.rs`
**Action**: modify

Move `POST /dump/{key}` and `GET /unsplash` into the tiered group; everything
else unchanged. Verified against axum 0.8.9 source: merging two routers that
define the same path with *disjoint methods* merges their method routers, so
GET and POST of `/dump/{key}` may live in different routers:

Each endpoint gets its **own** tier group so budgets don't pool:

```rust
pub fn routes() -> Router<AppState> {
    let dump_tier = crate::app::rate_limit::tiered_routes(
        Router::new().route("/dump/{key}", axum::routing::post(handlers::dump::web::create)),
        crate::app::rate_limit::DUMP_TIER_PER_MS,
        crate::app::rate_limit::DUMP_TIER_BURST,
    );
    let unsplash_tier = crate::app::rate_limit::tiered_routes(
        Router::new().route("/unsplash", get(handlers::unsplash::json::index)),
        crate::app::rate_limit::UNSPLASH_TIER_PER_MS,
        crate::app::rate_limit::UNSPLASH_TIER_BURST,
    );

    Router::new()
        .route("/", get(handlers::home::web::index))
        .route("/singlethread", get(handlers::singlethread::web::index))
        .route("/dump/{key}", get(handlers::dump::web::index)) // global budget only
        .merge(dump_tier)
        .merge(unsplash_tier)
        .route("/health", get(health))
        .nest_service(
            "/static",
            /* unchanged: SetResponseHeader::overriding(ServeDir::new("static"), CACHE_CONTROL, ...) */
        )
}
```

(The first `tiered` sketch above is wrong — each endpoint gets its **own**
tier group so budgets don't pool; implement the two-group version.)

#### 3. Integration tests
**File**: `src/interfaces/handlers/dump/web.rs` and
`src/interfaces/handlers/unsplash/json.rs` (inline test modules)
**Action**: modify

Use the existing test helpers (`crate::test::{start_app_with_rate_limits,
start_unsplash_stub, test_client}`):

- `dump/web.rs` — tier trips independently of global budget:

```rust
#[tokio::test]
async fn dump_post_tier_trips_while_global_budget_stays_open() {
    let (addr, _pool) = crate::test::start_app_with_rate_limits(
        "https://api.unsplash.com",
        1, 1_000_000,          // global effectively disabled
    ).await;
    let client = crate::test::test_client();
    // DUMP_TIER_BURST = 3; fire 15 concurrent POSTs of tiny JSON
    let mut created = 0; let mut limited = 0;
    let handles: Vec<_> = (0..15).map(|_| {
        let client = client.clone(); let url = format!("http://{addr}/dump/tier-test");
        client.post(url).json(&serde_json::json!({ "n": 1 }))
    }).collect();
    // join all, then: assert created >= 1 && limited >= 5;
    // every 429: body == "too many requests" and retry-after present
}
```

- `dump/web.rs` — GET shares only the global budget:

```rust
#[tokio::test]
async fn dump_get_is_not_tier_limited() {
    let (addr, _pool) = start_app_with_rate_limits(..., 1, 1_000_000).await;
    // 30 sequential GETs to /dump/anything -> all 200 (would trip any sane tier)
}
```

- `unsplash/json.rs` — tier trips with the stub upstream (base_url override is
  why the harness takes it):

```rust
#[tokio::test]
async fn unsplash_tier_trips_while_global_budget_stays_open() {
    let stub = crate::test::start_unsplash_stub(StatusCode::OK).await;
    let (addr, _pool) = crate::test::start_app_with_rate_limits(
        &stub.base_url, 1, 1_000_000,
    ).await;
    // 20 concurrent GETs to /unsplash: mix of 200 and 429;
    // 429s carry body "too many requests"; stub saw fewer calls than requests
}
```

#### 4. Document 429 behavior
**File**: `ROUTES.md`
**Action**: modify

Each affected `###` block (self-contained, cut at `---`) gains a rate-limit
line. Wording template:

- Every :3000 endpoint block (`GET /`, `GET /singlethread`, `GET /dump/{key}`,
  `POST /dump/{key}`, `GET /unsplash`, `GET /health`, `GET /static/{file}`):

  ```markdown
  - Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
    plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.
  ```

- Additionally on `POST /dump/{key}`:

  ```markdown
  - Rate limit: also subject to a stricter dedicated tier (see
    `DUMP_TIER_*` in `src/app/rate_limit.rs`) nested inside the global budget.
  ```

- Additionally on `GET /unsplash`: same sentence referencing `UNSPLASH_TIER_*`.
- Note in the file header that `/metrics` (:9090) is not rate-limited.

### Verification

#### Automated
- [x] `./scripts/test.sh` passes
- [x] New tier tests pass: `cargo nextest run tier`
- [x] Existing dump/unsplash suites still green (limits disabled in their harness)

#### Manual
- [ ] Boot locally; exceed the unsplash tier rapidly (curl loop on `/unsplash`)
      → 429s appear, while `curl http://localhost:3000/` still serves normally
      (global budget open).
- [ ] `POST /dump/{key}` 429s before the global budget does; `GET` on the same
      key still returns 200 during the tier throttle window.

---

## Phase 4: Store hygiene + deploy readiness

### Changes

#### 1. Store pruner
**File**: `src/app/rate_limit.rs`
**Action**: modify

Generic prune loop (cf. `../api/src/app/jobs/rate_limit_prune.rs`, kept inside
this module — no jobs framework):

```rust
use governor::{clock::QuantaInstant, middleware::RateLimitingMiddleware};
use std::{hash::Hash, time::Duration};
use tower_governor::governor::SharedRateLimiter;

const PRUNE_EVERY_SECS: u64 = 60;

/// Prune expired entries from a limiter store forever. Spawned detached;
/// shutdown is handled by process exit (task holds no shutdown-critical state).
pub async fn prune_loop<K, M>(limiter: SharedRateLimiter<K, M>)
where
    K: Clone + Eq + Hash + Send + Sync + 'static,
    M: RateLimitingMiddleware<QuantaInstant> + Send + Sync + 'static,
{
    let mut interval = tokio::time::interval(Duration::from_secs(PRUNE_EVERY_SECS));
    loop {
        interval.tick().await;
        limiter.retain_recent();
        tracing::trace!("pruned rate-limit store");
    }
}
```

Spawn it once inside `with_global_limit` (deviation from structure.md, which
listed `main.rs` for wiring: spawning inside `with_global_limit` means prod
*and* the tight-limit harness both get pruning with zero call-site changes;
`main.rs` therefore needs no Phase 4 change):

```rust
let limiter = governor_cfg.limiter().clone();
tokio::spawn(prune_loop(limiter));
```

Inline unit test (mirrors api's):

```rust
#[tokio::test]
async fn prune_does_not_panic() {
    // build SharedRateLimiter<String, NoOpMiddleware>, check_key once, tick prune body once
}
```

(Structure the test to call the retain step directly rather than sleeping 60 s.)

#### 2. Config documentation
**Files**: `.env_template`, local `.env`
**Action**: modify

Deviation note: the repo has no `.envrc.example`; `.envrc` contains only
`dotenv`, so the documented files are `.env_template` (committed) and `.env`
(local, gitignored).

Add to `.env_template`:

```
RATE_LIMIT_PER_MS=100
RATE_LIMIT_BURST=200
```

Mirror working values into local `.env`. The `.env_template` header comment
already lists where entries must be added — follow it: `.env`,
`.env_template`, Fly.io dashboard/secrets, 1Password. Put the concrete
commands in the PR description (not committed):

```
fly secrets set RATE_LIMIT_PER_MS=100 RATE_LIMIT_BURST=200
```

`fly.toml` needs no change (secrets are runtime env, not checked in).

### Verification

#### Automated
- [x] `./scripts/test.sh` passes (full gate green — ready for PR)

#### Manual
- [ ] Boot the server >60 s under light load; no unbounded memory growth
      (store entries older than the retention window get dropped each minute;
      confirm via `tracing` trace line at TRACE level or by observing stable RSS).
- [ ] `unset RATE_LIMIT_PER_MS; cargo run` fails fast with
      `RATE_LIMIT_PER_MS must be set and non-empty` (same for `_BURST`).
- [ ] Fresh clone + copied `.env_template` boots cleanly.

---

## Testing Checkpoints

- **After Phase 1**: extractor unit tests pass; all pre-existing suites green
  with limits disabled (`per_ms: 1, burst: 1M`); manual curl produces 429s and
  rate-limit headers.
- **After Phase 2**: integration tests prove 429 status + exact body +
  `Retry-After` over real TCP; `WebError::TooManyRequests` owns the format;
  governor errors map through the chokepoint.
- **After Phase 3**: tier limits trip independently of the global budget;
  GET dump stays global-only; `ROUTES.md` documents 429 behavior per block.
- **After Phase 4**: pruner runs on cadence; missing-env startup fails fast;
  full gate green — safe to open PR.

## Deviations from structure.md

1. **Pruner spawn site (Phase 4)**: spawned inside `with_global_limit` rather
   than wired in `main.rs`, so the test harness benefits identically and
   `main.rs` is untouched in this phase.
2. **Harness signature (Phases 2–3)**: `start_app_with_rate_limits` takes the
   unsplash base URL as its first argument, enabling stub-backed unsplash tier
   tests without a third builder.
3. **Env doc targets (Phase 4)**: `.env_template` + `.env` instead of the
   non-existent `.envrc.example`/`.envrc.example` pair named in structure.md.
4. **Error responder (Phase 2)**: confirmed `GovernorConfigBuilder::error_handler`
   exists in tower-governor 0.8, so the design's fallback wrapper layer is
   documented as contingency only.
