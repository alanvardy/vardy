# Research Findings

Branch: `alanvardy-var-670-handle-css-cache-invalidation-for-staticsitecss`
Scope: axum router/static serving, templates/, deployment config, tests.

## Q1: How is `/static` served and what caching headers does it emit?

### Findings
- `/static` is wired as a bare `ServeDir` with no layers:
  `.nest_service("/static", ServeDir::new("static"))` — `src/interfaces/routes.rs:12`;
  import at `src/interfaces/routes.rs:2`.
- Router is `Router<AppState>` built in `routes()` (`src/interfaces/routes.rs:7-13`);
  production serving via `axum::serve(...)` in `src/main.rs` (`.with_state(state).into_make_service_with_connect_info()`); test serving via `into_make_service()` (`src/test/mod.rs:16`).
- Dependency: `tower-http = { version = "0.6", features = ["fs"] }` — only `fs` enabled,
  **not** `set-header` (`Cargo.toml:11`). Resolved version: tower-http 0.6.11 (`Cargo.lock`).
- Headers emitted by ServeDir on a successful file response (from vendored 0.6.11 source):
  - `Content-Type` (mime_guess), `Accept-Ranges: bytes`, and `Last-Modified`
    (only when filesystem mtime is available).
  - Conditional requests honored natively: `If-Modified-Since` → `304 Not Modified`; `Range` supported.
- **Not emitted:** `ETag` (ServeDir 0.6.x never generates ETags) and `Cache-Control`.
  Setting `Cache-Control` would require the tower-http `set-header` feature +
  `SetResponseHeaderLayer`, neither of which is present.

## Q2: CSS references, template organization, rendered output

### Findings
- All styling is a single inline `<style>` block in the base layout:
  opens at `templates/layout.html:7`, closes at `templates/layout.html:57`, inside `<head>`.
  Defines `:root` CSS custom properties (`--bg`, `--surface`, `--text`, `--muted`,
  `--accent`) plus global rules for `*`, `body`, `.container`, `.card`, `nav`, `nav a`.
- There are **no `<link rel="stylesheet">` tags and no external CSS files anywhere**
  (grep across `*.rs`/`*.html`/`*.toml`). Only external asset reference today:
  `<img src="/static/singlethread-icon.png">` in `templates/singlethread.html:6`.
- Template loading: `minijinja::Environment` with
  `set_loader(minijinja::path_loader("templates"))` and an HTML auto-escape callback —
  `src/app/templates.rs:1-13`. minijinja 2.x with `debug` feature (`Cargo.toml:8`);
  resolved 2.24.0. Environment stored on `AppState.templates` (`src/app/state.rs:3`),
  initialized once in `src/main.rs` and in tests (`src/test/mod.rs:7`).
- Inheritance: one level. `templates/layout.html` declares blocks
  `title`, `heading`, `content` plus shared nav; `templates/home.html:1` and
  `templates/singlethread.html:1` both `{% extends "layout.html" %}`.
- Render call sites: `get_template("home.html")?.render(context! {})` → `Html<String>`
  (`src/interfaces/handlers/home/web.rs:8-12`, route `src/interfaces/routes.rs:9`);
  same pattern for `singlethread.html` (`src/interfaces/handlers/singlethread/web.rs:8-12`,
  route `src/interfaces/routes.rs:10`).
- An external stylesheet URL today would simply be a literal `<link>` written into
  `layout.html`'s `<head>` and emitted verbatim on every page render. No code generates
  or versions asset URLs; no fingerprinting/hash/query-string mechanism exists anywhere.

## Q3: Router/middleware layers and header-setting patterns

### Findings
- The entire router is four entries in `routes()` (`src/interfaces/routes.rs:7-13`):
  `GET /` → home handler (:9), `GET /singlethread` → singlethread handler (:10),
  inline `GET /health` closure returning `StatusCode::OK` (:11),
  `nest_service("/static", ...)` (:12).
- What does **not** exist anywhere in `src/`: no `.layer(...)`, no
  `middleware::from_fn/from_extractor`, no `.fallback(...)`, no `.merge(...)`,
  no `.route_layer(...)`. Grep confirms zero matches.
- Handlers return `Result<Html<String>, WebError>` extracting `State<AppState>`
  (`src/interfaces/handlers/home/web.rs:5-12`,
  `src/interfaces/handlers/singlethread/web.rs:5-12`).
- Errors build responses from `(StatusCode, &str)` tuples in `WebError::into_response`
  (`src/app/error.rs:22-33`) — no custom headers set there.
- Header-setting patterns: **none exist**. Zero occurrences of `HeaderName`,
  `HeaderValue`, typed-header tuples, or any code that sets a response header.
  The only header code is three read-only test assertions on `content-type`
  (`src/interfaces/routes.rs:31`, `src/interfaces/handlers/home/web.rs:29-31`,
  `src/interfaces/handlers/singlethread/web.rs:29-31`).

## Q4: Build/deploy pipeline and asset identity across deploys

### Findings
- Multi-stage Dockerfile: `chef` base `lukemathwalker/cargo-chef:latest-rust-1-bookworm`
  (`Dockerfile:1`), `planner` runs `cargo chef prepare` after `COPY . .` (:4-6),
  `builder` cooks deps then `COPY . .` again and `cargo build --release --bin vardy`
  (:14-16), runtime stage `debian:bookworm-slim` (:19).
- Runtime stage copies verbatim from builder: `migrations` (`Dockerfile:22`),
  `templates` (`Dockerfile:23`), **`static` (`Dockerfile:24`)**, then the binary (:25);
  DB created + migrations run at image build time (:27-29); entrypoint `/usr/local/bin/vardy` (:30).
- `.dockerignore` excludes `.git`, CI configs, `scripts/`, etc., but **not**
  `static/` or `templates/`.
- fly.toml: app `vardy`, region `ord`, empty `[build]` section (`fly.toml:9`) so fly.io
  builds the Dockerfile remotely (`flyctl deploy --remote-only` in
  `.github/workflows/fly-deploy.yml:17`); internal_port 3000, health check `GET /health`.
- Asset identity: paths are stable across deploys (e.g. `/static/singlethread-icon.png`)
  and contents change only when the git-tracked file changes. **No renaming, hashing,
  fingerprinting, versioning, or cache-busting step exists anywhere in the repo**
  (grep over `src/`, scripts, workflows found nothing). No CI workflow processes assets.
- Current contents: `static/` holds only `singlethread-icon.png` (~82 KB);
  `templates/` holds `home.html`, `layout.html`, `singlethread.html`.
- Note: CSS currently is not a separate file — it lives inline in
  `templates/layout.html:7-57`, so it ships inside the rendered HTML document.

## Q5: Test patterns for router/static serving and header assertions

### Findings
- Test helper module `src/test/mod.rs` (mounted via `#[cfg(test)] mod test;` in `src/main.rs`):
  - `start_app()` (`src/test/mod.rs:5-21`): real `AppState` with `templates::init()` +
    in-memory SQLite (`sqlite::memory:`), binds `127.0.0.1:0`, builds the full production
    router via `routes().with_state(state)`, spawns `axum::serve` in a background task,
    returns the bound `SocketAddr`.
  - `test_client()` (`src/test/mod.rs:23-25`): plain `reqwest::Client::new()`.
- Pattern: live HTTP against a bound listener using reqwest (dev-dependency,
  `Cargo.toml:14`). No `tower::ServiceExt::oneshot` usage anywhere.
- Existing route tests:
  - `static_icon_is_served` (`src/interfaces/routes.rs:19-33`): GET
    `/static/singlethread-icon.png`, asserts status OK and `content-type` contains
    `image/png`.
  - `health_returns_200` (`src/interfaces/routes.rs:36-45`).
  - Home page test asserts OK, `text/html`, body substrings incl. `<title>`/nav links
    (`src/interfaces/handlers/home/web.rs:15-40`).
  - SingleThread test same shape, body includes `/static/singlethread-icon.png`
    (`src/interfaces/handlers/singlethread/web.rs:15-41`).
- Header assertion idiom used consistently:
  `res.headers().get("content-type").is_some_and(|v| v.to_str().unwrap().contains("..."))`
  (`src/interfaces/routes.rs:30-33`, both handler web.rs files ~lines 29-31).
- No test asserts any cache-related header (`cache-control`, `etag`, `last-modified`)
  anywhere in the suite.
- Other unit-test patterns (non-router): pure sync tests asserting `IntoResponse`
  status codes from `WebError` (`src/app/error.rs:43-70`), `#[sqlx::test]` migrations
  (`src/app/db.rs`), sync minijinja auto-escape test (`src/app/templates.rs:14+`).

## Cross-Cutting Observations
- The repo-relative-path convention spans serving and deployment: `ServeDir::new("static")`
  (`src/interfaces/routes.rs:12`) and `path_loader("templates")` (`src/app/templates.rs:4`)
  both assume the process CWD is `/app`, which the Dockerfile guarantees
  (`WORKDIR /app`, `COPY ... ./static`, `./templates` — `Dockerfile:20,23-24`).
- Only two cargo features are relevant to HTTP behavior today: axum 0.8.9 and
  tower-http with just `fs` (`Cargo.toml:11`) — no `set-header` feature, hence no
  existing mechanism to attach custom headers to the static service.
- Everything rendered is HTML assembled by minijinja at request time; the only
  cacheable-by-browser assets are whatever appears under `/static`, and today that is
  a single PNG referenced by unversioned path (`templates/singlethread.html:6`).
- Tests exercise the exact production router through a real listener
  (`src/test/mod.rs:14-19` mirrors `src/main.rs` wiring), so new routes/layers added to
  `routes()` are automatically covered by the harness pattern.

## Open Areas
- Runtime behavior was not executed/observed (e.g., actual `Last-Modified` values or
  304 responses from the running server); Q1 header findings come from vendored
  tower-http 0.6.11 source, which is authoritative for the locked version.
- Browser-side heuristic caching behavior (in absence of `Cache-Control`) is external
  to this codebase and was not measured.
- Whether fly.io's proxy adds/overrides any caching headers at the edge is not
  observable from the repository.
