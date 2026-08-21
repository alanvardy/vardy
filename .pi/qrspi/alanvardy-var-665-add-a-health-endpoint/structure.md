# Structure Outline

## Approach
Ship the health endpoint as two small vertical slices, each independently
verifiable: (1) the route + integration test in Rust code, mirroring the
sibling `../api` inline-closure pattern; (2) the Fly platform check in
`fly.toml`, which depends on the route existing but is verified separately.
No database, no state, no handler module — deliberately minimal per design.

---

## Phase 1: `/health` endpoint with integration test

Delivers end-to-end HTTP functionality: `GET /health` returns 200 over real
HTTP, proven by a test using the existing `start_app()`/`test_client()`
harness.

**Files**: `src/interfaces/routes.rs`
**Key changes**:
- Import `axum::http::StatusCode` alongside existing `axum::routing::get`
- Route registration (modified `pub fn routes() -> Router<AppState>`):
  ```rust
  .route("/health", get(|| async { StatusCode::OK }))
  ```
- New test in existing `#[cfg(test)] mod tests`:
  ```rust
  #[tokio::test]
  async fn health_returns_200() // start_app().await → test_client().get(.../health) → assert StatusCode::OK
  ```

**Verify**: `cargo nextest run health_returns_200` passes; `cargo clippy` is
clean; manually `cargo run` and `curl -i localhost:3000/health` → `200 OK`
with empty body.

---

## Phase 2: Fly health check + config cleanup

Delivers platform-level monitoring: Fly probes `/health` every 30s so the
app's uptime is actively checked. Also removes the stale `PORT = '8080'` env
line that contradicts the actual bind port (3000).

**Files**: `fly.toml`
**Key changes**:
- Add inside `[http_service]` (mirroring `../api/fly.toml:22-26`):
  ```toml
  [[http_service.checks]]
    grace_period = "10s"
    interval = "30s"
    method = "GET"
    timeout = "5s"
    path = "/health"
  ```
- Delete `PORT = '8080'` from `[env]` (`fly.toml:12`)

**Verify**: TOML validity — `flyctl config validate` (or `fly tomly check`
if available; at minimum `flyctl deploy --dry-run` locally or careful
eyeball). Manual: confirm `internal_port = 3000` matches the check target
and that no other code reads `PORT` (`rg "PORT" src/` → no hits). Full
end-to-end confirmation (Fly shows the check healthy) only happens after
deploy to main — note as post-merge validation.

---

## Testing Checkpoints

- **After Phase 1**: `cargo nextest run` fully green including
  `health_returns_200`; clippy clean; `/health` returns 200 locally. The
  app is complete from a code perspective even if Phase 2 never happens.
- **After Phase 2**: `fly.toml` parses and contains the `[[http_service.checks]]`
  block targeting `/health`; stale `PORT` env gone. After merge/deploy, Fly
  dashboard shows the `health` check passing (grace 10s covers startup).
- **Resume hint**: if context resets, check `git log` — Phase 1 touches only
  `src/interfaces/routes.rs`, Phase 2 only `fly.toml`.

## Slicing Notes
- Nothing in this design resists vertical slicing; the work is small enough
  that two slices suffice. Phases 1→2 are ordered by dependency: the Fly
  check is meaningless without the route, and Phase 1 is independently
  valuable on its own.
- Post-deploy Fly check health is inherently a CI/CD-phase verification and
  cannot be fully automated locally — flagged explicitly rather than forced
  into a fake checkpoint.
