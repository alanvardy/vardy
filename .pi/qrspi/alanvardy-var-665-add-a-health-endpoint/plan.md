# Implementation Plan — VAR-665: Add a Health Endpoint

## Overview

`GET /health` returns HTTP 200 (empty body, stateless) with a real-HTTP
integration test proving it, and `fly.toml` gains a `[[http_service.checks]]`
block probing `/health` every 30s so Fly actively monitors uptime; the stale
`PORT = '8080'` env line is removed in the same edit.

---

## Phase 1: `/health` endpoint with integration test

### Changes

#### 1. Route registration + inline closure handler
**File**: `src/interfaces/routes.rs`
**Action**: modify

Add `StatusCode` to the axum import and one `.route(...)` line to the builder
chain. The handler is an inline closure — the established precedent is
`../api/src/interfaces/routes.rs:18`. No new module, no state.

```rust
use axum::{Router, http::StatusCode, routing::get};
```

```rust
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(handlers::home::web::index))
        .route("/singlethread", get(handlers::singlethread::web::index))
        .route("/health", get(|| async { StatusCode::OK }))
        .nest_service("/static", ServeDir::new("static"))
}
```

Notes:
- Closure returns `StatusCode`, which implements `IntoResponse`; no error
  type needed.
- Placement anywhere in the chain works; shown above before `nest_service`
  for readability. Do not touch the existing routes.

#### 2. Integration test
**File**: `src/interfaces/routes.rs`
**Action**: modify (append to existing `#[cfg(test)] mod tests`)

The tests module already imports `crate::test::{start_app, test_client}` and
`axum::http::StatusCode` — no new imports required there.

```rust
#[tokio::test]
async fn health_returns_200() {
    let addr = start_app().await;
    let client = test_client();
    let res = client
        .get(format!("http://{addr}/health"))
        .send()
        .await
        .expect("request to /health should succeed");
    assert_eq!(res.status(), StatusCode::OK);
}
```

This mirrors `static_icon_is_served` (same file) minus the header assertion:
plain `#[tokio::test]` (no DB in this repo), real HTTP against an
OS-assigned port via `start_app()` (`src/test/mod.rs:6-20`) +
`test_client()` (`src/test/mod.rs:22-24`). Empty body is implied by the
closure returning only `StatusCode` — no explicit assertion needed.

### Verification

#### Automated
- [x] `cargo nextest run health_returns_200` passes
- [x] `cargo nextest run --profile ci` fully green (existing
      `static_icon_is_served` unaffected)
- [x] `cargo clippy --all-targets --all-features --locked -- -D warnings`
      clean (exact CI command from `.github/workflows/ci.yml:126`)
- [x] `cargo fmt --all -- --check` clean

#### Manual
- [ ] `cargo run`, then `curl -i localhost:3000/health` → `HTTP/1.1 200 OK`
      with empty body

---

## Phase 2: Fly health check + config cleanup

### Changes

#### 1. Add `[[http_service.checks]]`, remove stale `PORT` env
**File**: `fly.toml`
**Action**: modify

Delete line `PORT = '8080'` from `[env]` (leaving an empty `[env]` section is
fine, or remove the section if it becomes empty — either parses). Add the
checks block inside `[http_service]`, after `processes = ['app']`, mirroring
`../api/fly.toml:22-26` with `path` changed to `/health`:

```toml
[http_service]
  internal_port = 3000
  force_https = true
  auto_stop_machines = 'stop'
  auto_start_machines = true
  min_machines_running = 1
  processes = ['app']

  [[http_service.checks]]
    grace_period = "10s"
    interval = "30s"
    method = "GET"
    timeout = "5s"
    path = "/health"
```

Rationale for removal of `PORT`: the app hardcodes `0.0.0.0:3000`
(`src/main.rs:9`) and Fly routes to `internal_port = 3000` (`fly.toml:16`),
so the env var is dead config that contradicts both. Confirmed no code reads
it (see verification below).

### Verification

#### Automated
- [x] TOML parses: run `flyctl deploy --dry-run` if available; otherwise
      `flyctl config validate`; fallback if flyctl is unavailable or errors
      on auth: `python3 -c "import tomllib; tomllib.load(open('fly.toml','rb'))"`
      (parse-only sanity check)
- [x] `rg "PORT" src/` → no hits (nothing reads the removed env var)
- [x] Full suite still green: `cargo nextest run` (Phase 1 code untouched,
      confirms nothing else regressed)

#### Manual
- [x] `fly.toml` contains the `[[http_service.checks]]` block with
      `path = "/health"` and the check targets the same port as
      `internal_port = 3000` (Fly checks hit `internal_port` by default;
      they agree by construction)
- [x] No leftover `PORT = '8080'` line in `fly.toml`

#### Post-merge (cannot be verified locally)
- [ ] After deploy to main (`.github/workflows/fly-deploy.yml` runs
      `flyctl deploy --remote-only`), the Fly dashboard shows the `service
      check 'vardy-check'` passing on `/health`. The 10s grace period covers
      app startup.

---

## Testing Checkpoints

- **After Phase 1**: all four automated checks green; `/health` returns 200
  locally. Code-side work complete.
- **After Phase 2**: `fly.toml` parses and carries the checks block; `PORT`
  env gone. Final Fly-dashboard confirmation is post-deploy only.
- **Resume hint**: Phase 1 touches only `src/interfaces/routes.rs`;
  Phase 2 touches only `fly.toml`.
