# Implementation Plan

## Overview

Port the sibling `../api` logging pattern into this repo: a flattened-JSON
tracing subscriber to BrokenPipe-safe stderr (initialized first in `main`),
per-request `TraceLayer` tracing on the main 3000 router only, and structured
`tracing::error!` events replacing `WebError`'s `eprintln!` calls. HTTP
responses are byte-identical; tests stay subscriber-free and green.

**Version note**: this repo pins `tower-http = { version = "0.6", features =
["fs", "set-header"] }` (`Cargo.toml:15`). Add `"trace"` to the existing 0.6
features — do **not** bump to 0.7 despite `../api` using it;
`TraceLayer`/`MakeWriter` APIs are identical across 0.6/0.7 so
`../api/src/app/log.rs` ports verbatim.

---

## Phase 1: Logging Foundation — subscriber init + startup banner

End-to-end goal: binary boots with a real subscriber and emits one flattened
JSON line per event to stderr; startup banner becomes two timestamped
`tracing::info!` lines (one per successful bind, fixing the ordering gap
where today's banner prints before the 9090 bind).

### Changes

#### 1. Dependencies

**File**: `Cargo.toml`
**Action**: modify

In `[dependencies]`:

```toml
tower-http = { version = "0.6", features = ["fs", "set-header", "trace"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
```

(Keep alphabetical-ish placement consistent with the existing list; add the
two new crates after `tokio` / near `tower-http`.)

#### 2. New logging module

**File**: `src/app/log.rs`
**Action**: create

Copy from `../api/src/app/log.rs` (lines 14–57) — imports, `StderrWriter`,
and `init()`, unchanged:

```rust
use std::io::{self, Write};
use tracing_subscriber::{EnvFilter, fmt};

/// Writer that silently drops `BrokenPipe` errors on stderr instead of
/// panicking. On Unix, stderr is often a pipe (journald, Fly.io capture,
/// or a terminal that exits); when the downstream end closes, write()
/// returns Err(BrokenPipe) and the default writer panics on it.
struct StderrWriter;

impl Write for StderrWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match io::stderr().write(buf) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(buf.len()),
            Err(e) => Err(e),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match io::stderr().flush() {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            Err(e) => Err(e),
        }
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for StderrWriter {
    type Writer = Self;

    fn make_writer(&self) -> Self::Writer {
        StderrWriter
    }
}

// Emit one structured JSON log line per event so Fly.io's stdout capture can
// forward request logs to downstream aggregators such as Loki/Grafana.
pub fn init() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info"));

    fmt()
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .with_env_filter(filter)
        .with_writer(StderrWriter)
        .init();
}
```

Phase 2 appends the tracing-layer items to this same file (see below); you
may create the file complete in Phase 1 and simply not call `trace_layer()`
until Phase 2 — but if you prefer strict slicing, add only the code above in
Phase 1 and add the Phase-2 block as its own commit-sized step.

#### 3. Register module

**File**: `src/app/mod.rs`
**Action**: modify

```rust
pub mod assets;
pub mod db;
pub mod error;
pub mod log;
pub mod state;
pub mod templates;
```

#### 4. Wire init + replace banner with per-bind info events

**File**: `src/main.rs`
**Action**: modify

- First statement of the `main` body (before reading `DATABASE_URL`):
  `app::log::init();`
- Delete `println!("Hosting on http://localhost:3000");` (currently line 16).
- Add an `info!` after **each** successful bind:

```rust
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::log::init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/vardy.db".to_string());
    // ... AppState construction unchanged ...

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    info!("Hosting on http://localhost:3000");
    let metrics_listener = tokio::net::TcpListener::bind("0.0.0.0:9090").await?;
    info!("Metrics listening on http://localhost:9090");

    // try_join! unchanged
}
```

(`use tracing::info;` at the top of the file, alongside the existing `mod`
declarations.)

Tests are unaffected by all of the above: `src/test/mod.rs` boots its own
servers and never calls `main`, so no subscriber exists in test processes
and output stays quiet.

### Verification

#### Automated
- [x] `./scripts/test.sh` passes (fmt, sqlx offline refresh, check, clippy,
      nextest, TODO grep) with zero change in test output
- [x] `cargo tree -i tracing-subscriber` resolves (dependency actually wired)

#### Manual
- [ ] `cargo run` prints exactly two JSON lines on **stderr**, in bind order:
      3000 first ("Hosting"), then 9090 ("Metrics") — each with `timestamp`,
      `"level":"INFO"`, `"target":"vardy"`, flattened fields
- [ ] Nothing appears on **stdout**
- [ ] `RUST_LOG=debug cargo run` increases verbosity; `RUST_LOG=off cargo run`
      silences both startup lines; unset `RUST_LOG` falls back to
      `info,tower_http=info`
- [ ] `RUST_LOG=off cargo run | head` then Ctrl-D/pipe close does not panic
      (BrokenPipe swallowed) — optional spot-check of the writer

---

## Phase 2: Request Tracing — TraceLayer on the main router

End-to-end goal: every request to the 3000 router emits a JSON span with
low-cardinality route pattern, method, and path, plus INFO request/response
events. The metrics router on 9090 stays unwrapped (no scrape noise).

### Changes

#### 1. Append tracing layer to the logging module

**File**: `src/app/log.rs`
**Action**: modify (append)

Add imports at the top and the two functions at the bottom — copied verbatim
from `../api/src/app/log.rs:62-81`:

```rust
use axum::extract::{MatchedPath, Request};
use tower_http::{
    classify::{ServerErrorsAsFailures, SharedClassifier},
    trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::{Level, Span};

type MakeSpanFn = fn(&Request) -> Span;

// Log the matched route (e.g. `/dump/{key}`) rather than the concrete path
// so per-request logs stay low cardinality.
pub fn trace_layer() -> TraceLayer<SharedClassifier<ServerErrorsAsFailures>, MakeSpanFn> {
    TraceLayer::new_for_http()
        .make_span_with(make_span as MakeSpanFn)
        .on_request(DefaultOnRequest::new().level(Level::INFO))
        .on_response(DefaultOnResponse::new().level(Level::INFO))
}

fn make_span(request: &Request) -> Span {
    let path = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or_else(|| request.uri().path());

    tracing::info_span!(
        "http_request",
        method = %request.method(),
        path = %path,
    )
}
```

Notes:
- `axum::extract::Request` is the `http::Request<Body>` alias — correct for
  axum 0.8 here and matches `../api` verbatim.
- These APIs exist identically in tower-http 0.6 with the `trace` feature.

#### 2. Apply the layer in `main`

**File**: `src/main.rs`
**Action**: modify

Chain `.layer(...)` after `.with_state(state)` in the **main router's**
serve arm only:

```rust
axum::serve(
    listener,
    interfaces::routes::routes()
        .with_state(state)
        .layer(app::log::trace_layer())
        .into_make_service_with_connect_info::<std::net::SocketAddr>(),
),
```

The metrics `axum::serve(metrics_listener, ...)` arm stays untouched —
deliberately unwrapped, matching `../api`.

No changes to `src/interfaces/routes.rs`: keeping the layer application at
the composition root mirrors `../api` (`main.rs:83`) and avoids affecting
the test harness servers, which call `routes()` directly.

### Verification

#### Automated
- [ ] `./scripts/test.sh` passes (tests never attach the layer → no span
      noise in test output)

#### Manual
- [ ] `cargo run`, then `curl -s localhost:3000/health > /dev/null` — stderr
      shows one JSON span event plus two INFO events (`started processing
      request` / `finished processing request`) with `"method":"GET"` and
      `"path":"/health"` (route pattern, not query string if one is appended)
- [ ] `curl -s localhost:3000/dump/somekey` logs `"path":"/dump/{key}"`
      (matched route, low cardinality)
- [ ] `curl -s localhost:9090/metrics > /dev/null` produces **no** span
      output on stderr
- [ ] Startup JSON lines from Phase 1 still appear

---

## Phase 3: Structured Error Events — `WebError` via `tracing::error!`

End-to-end goal: server-side failures emit a structured JSON ERROR event
including the source chain (`{err:?}`), correlated to the request by
Phase 2's response events. HTTP responses are byte-identical.

### Changes

#### 1. Replace `eprintln!` with `tracing::error!`

**File**: `src/app/error.rs`
**Action**: modify

```rust
impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        match self {
            WebError::NotFound => (StatusCode::NOT_FOUND, "not found").into_response(),
            WebError::Database(err) => {
                tracing::error!(error = ?err, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
            }
            WebError::Template(err) => {
                tracing::error!(error = ?err, "template render error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
            }
        }
    }
}
```

- `?err` (Debug) captures the full source chain (minijinja/sqlx), replacing
  Display-only `{err}`.
- The `NotFound` arm stays log-free (current behavior; per design).
- No import needed — fully qualified `tracing::error!`.
- Existing unit tests (`error.rs` bottom) assert status + variant shape only
  and remain unchanged. With no subscriber initialized under nextest, the
  events are no-ops in tests.

### Verification

#### Automated
- [ ] `./scripts/test.sh` passes

#### Manual
- [ ] `cargo run`; temporarily `mv templates templates.bak`, then
      `curl -si localhost:3000/` — response is still `HTTP/1.1 500` with body
      `internal server error` (byte-identical to pre-change); stderr shows a
      JSON event with `"level":"ERROR"`, `"message":"template render error"`,
      and an `error` field containing the minijinja source chain
      (e.g. `TemplateNotFound`)
- [ ] `mv templates.bak templates`; confirm `/` renders normally again and
      TraceLayer logged the corresponding 500 response during the failure
      (correlation between error event and request span)
- [ ] `curl -si localhost:3000/nonexistent` → 404 `not found`, and **no**
      ERROR event emitted (NotFound stays silent)

---

## Testing Checkpoints

| After phase | True state |
|---|---|
| 1 | Binary boots; JSON startup lines on stderr in bind order; `RUST_LOG` respected; `./scripts/test.sh` green with zero test-output change |
| 2 | App-router requests emit route/method/path spans; `/metrics` scrapes emit nothing; startup lines still work |
| 3 | Forced 500s emit structured ERROR events with source chains; HTTP responses identical to pre-change; full suite green |

Resume hint: each phase leaves the repo committable — `git log` plus this
table tells you where to continue.

## Notes

- No migrations, store methods, or UI changes anywhere in this task.
- Deliberately out of scope per design: no request-ID middleware, no
  `.env_template` documentation, no handler-level logging, no changes to the
  test harness or `routes.rs`.
