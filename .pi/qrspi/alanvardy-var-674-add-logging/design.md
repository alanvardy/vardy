# Design — Add Logging (VAR-674)

## Current State

The app has no logging infrastructure at all. Total runtime output is three
unstructured lines:

- `println!("Hosting on http://localhost:3000")` — the only stdout output,
  printed after the 3000 bind but before 9090 binds (`src/main.rs:16`)
- `eprintln!("database error: {err}")` and `eprintln!("template render
  error: {err}")` inside `WebError`'s `IntoResponse` impl
  (`src/app/error.rs:31-38`) — Display-only, no source chain, no timestamps,
  no levels

No logging/tracing crate exists in `Cargo.toml:6-18`; no `.layer(...)` or
`.middleware(...)` calls exist in `src/interfaces/routes.rs` (only
`ServeDir` + `SetResponseHeader` for static files, `routes.rs:20-26`).

Topology (`src/main.rs:5-30`): two routers — main app on `0.0.0.0:3000`
served with `.into_make_service_with_connect_info::<SocketAddr>()`, metrics
router on `0.0.0.0:9090` served bare — joined via `tokio::try_join!`.

Test harness (`src/test/mod.rs`) bypasses `main` entirely: `start_app` /
`start_app_with_metrics` build their own `AppState`, bind ephemeral ports,
and spawn their own `axum::serve` per test. Under nextest's
one-process-per-test model, anything initialized only in `main` never runs
under tests.

Deployment: Fly.io (`fly.toml`), binary is PID-1 (`Dockerfile:29`), fd 1/2
go directly to Fly machine logs. Env surface is a single `DATABASE_URL`
(`.env_template:1`).

The sibling `../api` already solved this exact problem with an established
pattern (see Patterns to Follow).

## Desired End State

- `tracing` + `tracing-subscriber` (features `env-filter`, `json`) and
  `tower-http` `trace` feature added to `Cargo.toml`.
- A `log::init()` subscriber — flattened JSON, one event per line, to a
  BrokenPipe-safe stderr writer — called as the first statement of `main`.
- Per-request HTTP tracing on the main 3000 router only: `TraceLayer`
  with a `make_span` capturing the matched route pattern (low cardinality),
  method, and path; INFO `on_request`/`on_response`.
- `WebError`'s `eprintln!` calls replaced with `tracing::error!` events
  including the source chain via `{err:?}`.
- Startup banner replaced with `tracing::info!` per bound port.
- Filtering via `RUST_LOG` with default `info,tower_http=info`.
- Tests keep passing unchanged (`./scripts/test.sh` green); test output is
  unaffected since tests never call `main` and no subscriber is initialized
  there.

Verification: `./scripts/test.sh` passes; running the app locally emits
JSON startup lines and per-request spans to stderr; a forced 500 (e.g.
corrupt template) emits a structured `tracing::error!` event.

## Patterns to Follow

Copy from `../api` — it is the reference implementation for every
touchpoint:

1. **Subscriber init** — `../api/src/app/log.rs:47-57`: `pub fn init()`
   builds `EnvFilter::try_from_default_env().unwrap_or_else(|_|
   EnvFilter::new("info,tower_http=info"))`, then
   `fmt().json().flatten_event(true).with_current_span(false)
   .with_env_filter(filter).with_writer(StderrWriter).init()`.
   JSON so Fly.io capture forwards to Loki/Grafana (`log.rs:45-46`).
2. **BrokenPipe-safe writer** — `../api/src/app/log.rs:14-41`: custom
   `StderrWriter` implementing `Write` + `MakeWriter`, swallowing
   `BrokenPipe` so pipe closure (journald/Fly/terminal exit) doesn't panic.
   Replicate verbatim.
3. **Request tracing layer** — `../api/src/app/log.rs:62-81`:
   `trace_layer()` returning `TraceLayer::new_for_http()` with
   `.make_span_with(make_span)`; `make_span` uses `MatchedPath` for the
   route pattern (falls back to `uri().path()`), fields `method` and
   `path`, INFO `on_request`/`on_response`. Replicate.
4. **Wiring call sites** — `../api/src/main.rs:22` (`log::init()` first in
   `main`), `main.rs:83` (`.layer(app::log::trace_layer())` on the main
   router only), `main.rs:38` (`tracing::info!` startup line). Mirror in
   `src/main.rs` and `src/interfaces/routes.rs`.
5. **Module placement** — `log.rs` lives in `../api/src/app/` alongside
   `state.rs`/`db.rs`/etc. Put ours at `src/app/log.rs`, registered in
   `src/app/mod.rs`.

**Patterns NOT to follow:**

- Do **not** wrap the metrics router with `trace_layer()` — `../api`
  deliberately leaves it unwrapped (`../api/src/main.rs:52-56`); `/metrics`
  is scraped periodically and tracing it would produce high-frequency noise.
- Do **not** keep `println!`/`eprintln!` diagnostics — they are the
  ad-hoc pattern this task replaces (`src/main.rs:16`,
  `src/app/error.rs:32,36`).
- Do **not** add a global subscriber init reachable from tests — the test
  harness boots its own servers (`src/test/mod.rs:5-24,30-56`) and must not
  start emitting JSON into test output.

## Design Decisions

1. **Output format**: Flattened JSON to stderr, identical to `../api` —
   consistent across both projects, Fly.io/Loki-ready. No pretty mode.
2. **Trace scope**: Main 3000 router only — matches `../api`; metrics
   scrapes don't need spans.
3. **Error logging**: Replace `eprintln!` with `tracing::error!` using
   `{err:?}` to capture the source chain. The event won't carry
   request method/URI (`into_response(self)` has none), but TraceLayer's
   response events correlate 500s to requests.
4. **Default filter**: `EnvFilter::try_from_default_env().unwrap_or_else(
   |_| EnvFilter::new("info,tower_http=info"))` — `RUST_LOG` overrides,
   zero new config surface, matches `../api`.
5. **Startup banner**: Two `tracing::info!` lines, one after each
   successful `TcpListener::bind` (3000 then 9090) — fixes the current
   ordering gap where the banner prints before 9090 binds
   (`src/main.rs:16-17`).
6. **Dependency versions**: Match `../api`'s `Cargo.toml` — `tracing = "0.1"`,
   `tracing-subscriber = { version = "0.3", features = ["env-filter",
   "json"] }`, and add `"trace"` to the existing `tower-http` features.
   `../api` uses tower-http 0.7; use the same minor version to keep the
   `trace` API identical.

## What We're NOT Doing

- No log shipping / Fly drain configuration — outside the repo's scope.
- No request-ID / correlation-ID middleware — `../api` doesn't have it;
  add later if needed.
- No `RUST_LOG` documentation in `.env_template` — the filter falls back
  sensibly without it; document only if the user asks.
- No env-configurable ports or DB path — unrelated to logging.
- No changes to the test harness — tests stay subscriber-free and quiet.
- No new error variants or changes to `WebError`'s HTTP responses — only
  the output mechanism inside `IntoResponse` changes.
- No logging in handlers or infra modules — only the three touchpoints
  (init, trace layer, error events).

## Open Risks

- **Version drift**: if this repo pins tower-http to a different minor
  than 0.7, the `TraceLayer`/`MakeWriter` APIs may differ slightly from
  `../api`'s code — check `Cargo.toml:18` before copying `log.rs` verbatim.
- **Log volume**: `info,tower_http=info` logs every request; if this app
  receives bot traffic on Fly, the default may be noisier than today.
  Mitigation is trivial (`RUST_LOG=tower_http=warn`), but worth noting.
- **Test visibility gap**: because tests bypass `main`, a bug in
  `log::init()` would only surface at runtime, not in CI. Acceptable —
  `init()` is a thin copy of proven `../api` code.
