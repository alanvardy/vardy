# Structure Outline — Add Logging (VAR-674)

## Approach

Port the proven `../api` logging pattern into three independently verifiable
slices: subscriber init (JSON to BrokenPipe-safe stderr), per-request
tracing on the main 3000 router only, and structured error events in
`WebError`. Tests stay untouched and subscriber-free — the harness bypasses
`main`, so nothing new reaches test output.

**Version note (resolves design's open risk)**: this repo pins
`tower-http = { version = "0.6", features = ["fs", "set-header"] }`
(`Cargo.toml:15`), not 0.7. We add `"trace"` to the existing 0.6 features —
`TraceLayer`/`MakeWriter` APIs are identical across 0.6/0.7, so `../api`'s
`log.rs` ports verbatim. No minor bump.

---

## Phase 1: Logging Foundation — subscriber init + startup banner

End-to-end: the binary boots with a real subscriber and emits one flattened
JSON line per event to stderr. Replaces the stdout banner with two
timestamped `tracing::info!` lines (one per successful bind, fixing the
current ordering gap where the banner prints before the 9090 bind).

**Files**: `Cargo.toml`, `src/app/log.rs` (new), `src/app/mod.rs`, `src/main.rs`
**Key changes**:
- `Cargo.toml`: add `tracing = "0.1"`, `tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }`; extend tower-http features to `["fs", "set-header", "trace"]`
- `struct StderrWriter` implementing `std::io::Write` + `MakeWriter` — swallows `BrokenPipe` (ported verbatim from `../api/src/app/log.rs:14-41`)
- `pub fn init()` — `EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,tower_http=info"))` → `fmt().json().flatten_event(true).with_current_span(false).with_env_filter(..).with_writer(StderrWriter).init()`
- `mod log;` added to `src/app/mod.rs`
- `app::log::init();` as first statement of `main`; delete `println!` at `src/main.rs:16`; add `tracing::info!` after each `TcpListener::bind` (3000, then 9090)

**Verify**: `./scripts/test.sh` passes (tests unaffected — no subscriber in
test processes). Manual: `cargo run` emits two JSON startup lines on stderr
in bind order; `RUST_LOG=debug` increases verbosity, `RUST_LOG=off` silences.

---

## Phase 2: Request Tracing — TraceLayer on the main router

End-to-end: every HTTP request to the app router produces a JSON span with
the low-cardinality route pattern, method, and path, plus INFO
request/response events. Metrics router on 9090 stays unwrapped (scrape
noise excluded, per design).

**Files**: `src/app/log.rs`, `src/main.rs`
**Key changes**:
- `pub fn trace_layer() -> TraceLayer<...>` — `TraceLayer::new_for_http().make_span_with(make_span)` with INFO `on_request`/`on_response` (ported from `../api/src/app/log.rs:62-68`)
- `async fn make_span(request: &Request<Body>) -> Span` — `MatchedPath` route pattern with `uri().path()` fallback, fields `method` and `path`, span name `http_request` (from `log.rs:70-81`)
- `main.rs`: `interfaces::routes::routes().with_state(state).layer(app::log::trace_layer())` before `.into_make_service_with_connect_info::<SocketAddr>()` — metrics `axum::serve` arm unchanged

**Verify**: `./scripts/test.sh` passes. Manual: `cargo run` then
`curl localhost:3000/health` — one JSON span + request/response events on
stderr with `"target":"vardy_http"`-style route field `/health`;
`curl localhost:9090/metrics` produces **no** span output.

---

## Phase 3: Structured Error Events — `WebError` via `tracing::error!`

End-to-end: server-side failures (bad template, DB error) emit a structured
JSON error event including the source chain (`{err:?}`), correlated to the
request by Phase 2's response events. HTTP responses are byte-identical —
only the output mechanism inside `IntoResponse` changes.

**Files**: `src/app/error.rs`
**Key changes**:
- `Database(err)` arm: `eprintln!("database error: {err}")` → `tracing::error!(error = ?err, "database error")`
- `Template(err)` arm: `eprintln!("template render error: {err}")` → `tracing::error!(error = ?err, "template render error")`
- `NotFound` arm unchanged (no log, per current behavior and design)
- Existing unit tests (`error.rs:43-71`) unchanged — they assert status + variant shape only

**Verify**: `./scripts/test.sh` passes. Manual: `cargo run`, hit an endpoint
that renders a template with the `templates/` dir temporarily renamed —
stderr shows a JSON `level:"ERROR"` event with the full minijinja source
chain, and TraceLayer logs the corresponding 500 response for correlation.

---

## Testing Checkpoints

| After phase | True state |
|---|---|
| 1 | Binary boots; JSON startup lines on stderr in bind order; `RUST_LOG` respected; `./scripts/test.sh` green with zero test-output change |
| 2 | App-router requests emit route/method/path spans; `/metrics` scrapes emit nothing; startup lines still work |
| 3 | Forced 500s emit structured `ERROR` events with source chains; all HTTP responses identical to pre-change; full suite green |

Resume hint: each phase leaves the repo in a committable state — if context
resets, `git log` plus this table tells you where to continue.

## Notes

- No phase requires a migration, store method, or UI change — this task's
  layers are `Cargo.toml` → `src/app/log.rs` → wiring (`main.rs`) →
  `error.rs`; each phase above crosses its full relevant stack.
- Nothing in the design is unsliceable; no horizontal-layer phase needed.
- Deliberately deferred per design: no request-ID middleware, no
  `.env_template` docs, no handler-level logging.
