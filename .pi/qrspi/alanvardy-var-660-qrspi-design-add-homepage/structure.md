# Structure Outline — Add an HTML homepage to `vardy`

## Approach
Convert the bare, zero-dependency `vardy` console program into an HTTP-only
minijinja HTML service that mirrors `api`'s minimal template stack: a single
`app::templates::init()` environment, a `#[derive(Clone)] AppState` holding it,
a `Router<AppState>` serving `/` from a handler that renders
`templates/layout.html` composed with `templates/home.html`. No DB/auth/email.
Covered, non-`main.rs` code must clear the CI 90% patch target.

Vertical slices (three phases): each crosses bootstrap → route → handler →
template → live HTTP test, so every phase leaves a runnable, testable server.

---

## Phase 1: Server bootstrap + live HTTP harness (static root)
Turn `vardy` into an HTTP server serving a static HTML page at `/`, with a
reqwest-over-TCP test harness. Establishes the dependency footing and the
test/run loop that later phases build on.

**Files**: `Cargo.toml`, `Cargo.lock`, `src/main.rs`, `src/interfaces/routes.rs`,
`src/interfaces/handlers/home/web.rs`, `src/test/mod.rs`

**Key changes**:
- `Cargo.toml [dependencies]` (add): `axum = "0.8.9"`,
  `tokio = { version = "1.52.3", features = ["rt-multi-thread","macros","net","io-util"] }`,
  dev `reqwest = { version = "0.13", features = ["json"] }`.
- `src/main.rs` — **remove** `greeting()` / `test_greeting`; replace console
  `main` with:
  `#[tokio::main] async fn main() -> Result<(), Box<dyn std::error::Error>>`
  → `tokio::net::TcpListener::bind("0.0.0.0:8080").await?` →
  `axum::serve(listener, interfaces::routes::routes().into_make_service()).await?`.
- `src/interfaces/routes.rs` — `pub fn routes() -> Router` returning
  `Router::new().route("/", get(handlers::home::web::index))`.
- `src/interfaces/handlers/home/web.rs` —
  `pub async fn index() -> Html<String>` returning a literal body (no template yet).
- `src/test/mod.rs` — `pub async fn start_app() -> SocketAddr` (bind
  `127.0.0.1:0`, `tokio::spawn` the router into `axum::serve`); `pub fn
  test_client() -> reqwest::Client`.
- Handler `#[cfg(test)]` test: GET `/` → `StatusCode::OK`, `content-type`
  contains `text/html`, body contains expected markup.

**Verify**: `cargo test` passes; `cargo run` then `curl http://localhost:8080/`
returns `200 text/html`. `cargo clippy` + `cargo fmt --check` clean.

---

## Phase 2: Minijinja templated homepage (layout + home)
Swap the static root for a real template render through `AppState`, `WebError`,
and filesystem templates — the actual deliverable. If this phase fails, Phase 1
still yields a working server + harness.

**Files**: `Cargo.toml`, `Cargo.lock`, `src/app/mod.rs` (declares `templates`,
`state`, `error`), `src/app/templates.rs`, `src/app/state.rs`,
`src/app/error.rs`, `src/interfaces/routes.rs`,
`src/interfaces/handlers/home/web.rs`, `templates/layout.html`,
`templates/home.html`, `src/main.rs`, `src/test/mod.rs`

**Key changes**:
- `Cargo.toml` — add `minijinja = { version = "2", features = ["debug"] }`.
- `src/app/templates.rs` —
  `pub fn init() -> minijinja::Environment<'static>` — `Environment::new()`,
  `set_loader(minijinja::path_loader("templates"))`, autoescape callback
  (`.html` → `AutoEscape::Html`, else `None`). Mirrors `api` templates.rs:1-11.
- `src/app/state.rs` — `#[derive(Clone)] pub struct AppState { pub templates:
  minijinja::Environment<'static> }`.
- `src/app/error.rs` — `pub enum WebError { Template(minijinja::Error),
  NotFound }` + minimal `IntoResponse` (Template→500, NotFound→404).
- `src/interfaces/routes.rs` — `pub fn routes() -> Router<AppState>`.
- `src/interfaces/handlers/home/web.rs` —
  `pub async fn index(State(state): State<AppState>) -> Result<Html<String>,
  WebError>` → `state.templates.get_template("home.html")?.render(context! {})?`
  → `Ok(Html(html))`.
- `src/main.rs` — build `AppState { templates: app::templates::init() }`,
  serve `routes().with_state(state).into_make_service_with_connect_info::<SocketAddr>()`.
- `templates/layout.html` — `{% block title %}{% endblock %}` +
  `{% block heading %}{% endblock %}` + `{% block content %}{% endblock %}`.
- `templates/home.html` — `{% extends "layout.html" %}`+ overrides
  `title`/`heading`/`content` (no `active_*`; single page).
- `src/test/mod.rs` — bootstrap now seeds `init()` + `AppState`; render-based
  assert updated to check composed markup (e.g. `extends`ed `<title>`/content).

**Verify**: `cargo test` passes (200 + `text/html` + template-composed body);
`cargo run` then browse `http://localhost:8080/` shows the rendered homepage.
`cargo clippy`, `cargo fmt --check` clean.

---

## Phase 3: Coverage & CI gate (verification/hardening)
Close the coverage gap behind `main.rs` and lock the CI gates for all new
non-`main.rs` code (90% patch target), plus template-path robustness.

**Files**: `src/app/templates.rs`, `src/test/mod.rs` (or `.config/nextest.toml`),
`codecov.yml` (unchanged — confirm keep ignore `src/main.rs`)

**Key changes**:
- Add `#[cfg(test)]` in `templates.rs` asserting the autoescape callback returns
  `AutoEscape::None` for a `.txt` name (covers the else-branch the live HTTP
  test never hits). Confirm `routes.rs`/handler path is covered.
- Ensure the `path_loader("templates")` CWD-relative path resolves under the
  test runner (run from repo root; verify/adjust nextest CWD as needed).

**Verify**: `cargo test`, `cargo clippy`, `cargo fmt --check` all pass; new
non-`main` `.rs` coverage meets the 90% patch target; `scripts/lint_string.sh`
still passes (only scans `*.rs`). Manual: `curl /` returns the composed page.

---

## Testing Checkpoints (resume if context resets)
- **After Phase 1**: repo builds; `cargo test` green; `curl /` → 200
  `text/html` static body; deps (axum, tokio, reqwest) wired in `Cargo.lock`.
- **After Phase 2**: templates render via `layout.html`/`home.html` with
  `{% extends %}`; live test asserts composed markup; `AppState`,
  `templates::init`, `WebError`, `Router<AppState>` all in place; main serves
  the homepage; `main.rs` stays coverage-ignored.
- **After Phase 3**: 90% patch target met on non-`main` code; CI-equivalent
  suite (`test`/`clippy`/`fmt --check`/`lint_string.sh`) green.

## Noted "Can't slice vertically"
Dependency/`Cargo.lock` wiring and the `codecov.yml` ignore of `main.rs` are
project-level preconditions, not independently testable slices — they are
folded into Phase 1 (deps) and held constant across phases (codecov).