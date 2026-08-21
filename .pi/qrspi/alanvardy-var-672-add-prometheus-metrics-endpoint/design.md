# Design — VAR-672: Add Prometheus metrics endpoint

## Current State

The vardy service is a tiny axum app with essentially zero observability
(`research.md` Q2–Q4):

- Router built by `routes()` in `src/interfaces/routes.rs:7-13`; routes are
  `/`, `/singlethread`, `/health` (inline closure), `/static` (ServeDir).
- **No middleware or tower layers anywhere** — zero `.layer()` calls
  (`research.md` Q2).
- **No logging/tracing/metrics dependencies**: console output is three
  println/eprintln statements (`src/main.rs:13`, `src/app/error.rs:32,36`)
  (`research.md` Q3).
- `AppState` (`src/app/state.rs:1-7`) is a two-field Clone struct; no
  atomics/Arc/Mutex patterns exist anywhere (`research.md` Q4).
- Tests use a real-socket harness: `start_app()` in `src/test/mod.rs:5-21`
  + `reqwest` client; route tests assert status + content-type substring
  (`src/interfaces/routes.rs:20-45`).
- Deployed on Fly.io, single port 3000, health-checked at `/health`
  (`fly.toml:13-25`); no scrape config exists (`research.md` Q6).

The sibling **api** project already runs Prometheus in production and is the
design reference for every major decision below.

## Desired End State

- A `/metrics` endpoint serving Prometheus text exposition format on a
  **dedicated port 9090**, scraped by Fly.io via a `[metrics]` block in
  `fly.toml`.
- An `AppMetrics` type owning a `prometheus::Registry` and named counters,
  shared as `Arc<AppMetrics>`.
- Page-view counters incremented inside the home and singlethread handlers,
  visible at `/metrics`.
- Verified by: unit test on the metrics router (in-process `oneshot`),
  counter-increment tests, and `cargo clippy -D warnings --locked` +
  nextest passing in CI.

## Patterns to Follow

From the **api** repo (the precedent this feature copies):

- **Registry-owner struct**: `AppMetrics { registry, counters… }` with
  `new() -> Result<Self, prometheus::Error>` and `render() -> String` —
  `api/src/infra/metrics.rs:4-31`. Place ours at `src/infra/metrics.rs`
  (create the `infra` module) to mirror it.
- **Dedicated metrics port**: bind `0.0.0.0:9090` on a second listener and
  serve a separate router alongside the main one via `tokio::try_join!` —
  `api/src/main.rs:47-63`. Adapt into `src/main.rs:12-19`.
- **Separate metrics router with own state**: `metrics_router(metrics:
  Arc<AppMetrics>) -> Router` with `.route("/metrics",
  get(handlers::metrics::metrics_handler)).with_state(metrics)` —
  `api/src/interfaces/routes.rs:28-33`.
- **Handler extracts Arc<AppMetrics>**: `State(metrics): State<Arc<AppMetrics>>`,
  returns `(CONTENT_TYPE header, body)` from `TextEncoder::format_type()`
  and `registry.gather()` — `api/src/interfaces/handlers/metrics.rs:10-18`.
- **Fly.io scrape config**: `[metrics]` block, `port = 9090`,
  `path = "/metrics"` — `api/fly.toml:34-36`.
- **Explicit `counter.inc()` at the point of interest**, not middleware —
  e.g. `api/src/app/jobs/heartbeat.rs` (`n.job_events_total.inc()`).

From this repo:

- Feature-module handler layout: new handler goes under
  `src/interfaces/handlers/<feature>/web.rs` per `handlers/mod.rs:1-2`
  convention (`research.md` Q1). Suggest `handlers/metrics/web.rs`.
- Test assertion style: status-code equality + content-type/header
  substring checks (`research.md` Q5); counter deltas asserted like
  api's heartbeat tests (`get()` before/after, `assert_eq!(+1.0)`).
- CI must stay clean: fmt, clippy `-D warnings --locked`, nextest
  (`.github/workflows/ci.yml`).

Patterns NOT to follow:
- The inline-closure route style of `/health`
  (`src/interfaces/routes.rs:11`) — fine for a constant 200, but the
  metrics handler needs state extraction; use a named handler module.
- Do NOT add `AppState` fields or any middleware layer — api proves
  neither is needed for this feature.

## Design Decisions

1. **Library**: `prometheus = { version = "0.14", default-features = false }`
   — identical to `api/Cargo.toml:10`. Not the `metrics` facade or
   `axum-prometheus`; matching the sibling repo keeps one observability
   idiom across both services.
2. **Port**: dedicated listener on 9090, second router served via
   `tokio::try_join!` (`api/src/main.rs:48-63`). Keeps the operational
   surface off the public port 3000 and enables Fly.io's native `[metrics]`
   scraping with zero firewall work.
3. **Scope**: application counters only, explicitly incremented in
   handlers. Start with `page_views_total{page="home"|"singlethread"}`
   (CounterVec with one label). No request-latency histograms, no
   process/runtime collector — api ships none, and adding middleware or a
   process collector would introduce patterns (layers, interior mutability)
   this codebase has never had (`research.md` Q2/Q4).
4. **State threading**: the metrics router gets its own state —
   `metrics_router(metrics: Arc<AppMetrics>)` with `.with_state(metrics)`,
   exactly as api does (`api/src/interfaces/routes.rs:29-32`), so
   `AppState` is untouched for the `/metrics` route itself. Because
   vardy's counters live in *page handlers* (api's live in jobs, which
   receive `Arc<AppMetrics>` directly), one field `pub metrics:
   Arc<AppMetrics>` is added to `AppState` (`src/app/state.rs`), updated
   at both construction sites (`src/main.rs:8-11`, `src/test/mod.rs:6-10`).
   This is the single deliberate deviation from api, forced by where our
   counters are incremented.
5. **Testing**: in-process `router.oneshot(...)` test for the metrics
   router mirroring `api/src/interfaces/routes.rs:244-262` (guards against
   silently-dropped routes), plus a counter-increment test on `AppMetrics`
   itself. Existing reqwest harness stays untouched for other routes.
6. **fly.toml**: append `[metrics]` with `port = 9090`, `path = "/metrics"`
   (`api/fly.toml:34-36`). Dockerfile needs no EXPOSE change (Fly ignores
   it); no health-check changes.

## What We're NOT Doing

- No request-metrics middleware (no latency histograms, no method/path/
  status labels) — avoids cardinality and ServeDir questions flagged in
  `research.md` Open Areas.
- No process/runtime metrics (memory, CPU, uptime) despite the task's word
  "runtime" — the prometheus crate's process collector adds platform
  surface for little value on a 1-CPU/512MB Fly VM; can be a follow-up.
- No tracing/logging introduction (`tracing-subscriber` etc.) — separate
  concern, separate ticket.
- No changes to `/health`, ports 3000, Dockerfile, or GitHub workflows.
- No DB pool metrics (`state.db` remains unused/dead as today).
- No auth on `/metrics` — Fly's `[metrics]` scraping handles access; the
  endpoint exposes only page-view totals.

## Open Risks

- **Label choice**: `page="home"` vs unlabelled separate counters — if
  more pages appear, a CounterVec keeps cardinality bounded; low risk.
- **Fly.io `[metrics]` behavior**: assumes Fly auto-scrapes the private
  port like it does for api; verify after first deploy that the metrics
  tab populates (grace period may apply).
- **Two-listener shutdown semantics**: `tokio::try_join!` returns on first
  error, matching api; acceptable for this service's profile.
- **Clippy `--locked`**: adding one dependency updates Cargo.lock in the
  same PR; ensure the lockfile commit accompanies the Cargo.toml change.
