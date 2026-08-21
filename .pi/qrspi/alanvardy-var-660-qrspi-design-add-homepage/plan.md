# Implementation Plan

## Overview

Convert the bare, zero-dependency `vardy` console program into an HTTP-only
minijinja HTML service that mirrors `api`'s minimal template stack: a single
`app::templates::init()` environment, a `#[derive(Clone)] AppState` holding it,
a `Router<AppState>` serving `/` from a handler that renders
`templates/layout.html` composed with `templates/home.html`. No DB/auth/email.
Covered, non-`main.rs` code must clear the CI 90% patch target. Three vertical
phases, each leaving a runnable, testable server.

### New module tree (created across phases)
```
src/main.rs                          # server bootstrap (codecov-ignored)
src/app/mod.rs                       # pub mod error; state; templates;
src/app/templates.rs                 # init() template env
src/app/state.rs                     # AppState{ templates }
src/app/error.rs                     # WebError + IntoResponse
src/interfaces/mod.rs                # pub mod handlers; routes;
src/interfaces/routes.rs             # routes() Router<AppState>
src/interfaces/handlers/mod.rs       # pub mod home;
src/interfaces/handlers/home/mod.rs  # pub mod web;
src/interfaces/handlers/home/web.rs  # index handler
templates/layout.html                # base layout with named blocks
templates/home.html                  # {% extends "layout.html" %}
src/test/mod.rs                      # start_app() + test_client() harness
```

---

## Phase 1: Server bootstrap + live HTTP harness (static root)

Turn `vardy` into an HTTP server serving a static HTML page at `/`, with a
reqwest-over-TCP test harness. Establishes the dependency footing and the
test/run loop later phases build on.

### Changes

#### 1. Dependencies
**File**: `Cargo.toml`
**Action**: modify — fill the empty `[dependencies]` and add `[dev-dependencies]`

```toml
[dependencies]
axum = "0.8.9"
tokio = { version = "1.52.3", features = ["rt-multi-thread", "macros", "net", "io-util"] }

[dev-dependencies]
reqwest = { version = "0.13", features = ["json"] }
```

**File**: `Cargo.lock`
**Action**: modify (auto) — run `cargo build`/`cargo test` to regenerate the
lockfile with the axum/tokio/reqwest transitive graph. Commit the updated lock.

#### 2. Server bootstrap
**File**: `src/main.rs`
**Action**: rewrite — drop the console greeting, become an axum server. Add
`mod app;` and `mod interfaces;` (declared at the top; `app` module comes in
Phase 2 — declare both now so the tree compiles).

```rust
mod app;
mod interfaces;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, interfaces::routes::routes().into_make_service()).await?;
    Ok(())
}
```

This removes `greeting()` / `test_greeting` entirely.

#### 3. Module skeleton (needed for compilation)
**File**: `src/interfaces/mod.rs` — **create**
```rust
pub mod handlers;
pub mod routes;
```
**File**: `src/interfaces/handlers/mod.rs` — **create**
```rust
pub mod home;
```
**File**: `src/interfaces/handlers/home/mod.rs` — **create**
```rust
pub mod web;
```
**File**: `src/app/mod.rs` — **create** (declares modules that Phase 2 fills)
```rust
pub mod error;
pub mod state;
pub mod templates;
```

#### 4. Routes
**File**: `src/interfaces/routes.rs`
**Action**: create — a single route serving `/`. In Phase 1 no state is needed.

```rust
use axum::{Router, routing::get};

use crate::interfaces::handlers;

pub fn routes() -> Router {
    Router::new().route("/", get(handlers::home::web::index))
}
```

#### 5. Handler (static body, no template yet)
**File**: `src/interfaces/handlers/home/web.rs`
**Action**: create

```rust
use axum::response::Html;

pub async fn index() -> Html<String> {
    Html("Hello, world!".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::{start_app, test_client};
    use axum::http::StatusCode;

    #[tokio::test]
    async fn index_serves_ok_html() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert!(res
            .headers()
            .get("content-type")
            .is_some_and(|v| v.to_str().unwrap().contains("text/html")));
        assert!(res.text().await.unwrap().contains("Hello, world!"));
    }
}
```

#### 6. Test harness
**File**: `src/test/mod.rs`
**Action**: create — shared live-HTTP bootstrap, sized for a stateless app.

```rust
use axum::Router;
use std::net::SocketAddr;

/// Bind a random port, spawn the app, return the bound address.
pub async fn start_app() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let router: Router = crate::interfaces::routes::routes();
    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service())
            .await
            .expect("server");
    });
    addr
}

pub fn test_client() -> reqwest::Client {
    reqwest::Client::new()
}
```

> **Note (in module tree)**: main.rs must declare `mod test;` under
> `#[cfg(test)]` so the handler tests can reach it. Add at the bottom of
> `src/main.rs`:
> ```rust
> #[cfg(test)]
> mod test;
> ```

### Verification

#### Automated
- [x] `cargo build` regenerates `Cargo.lock` and compiles clean
- [x] `cargo test` passes (the handler's live HTTP test: 200 + `text/html` + body markup)
- [x] `cargo clippy --all-targets --all-features --locked -- -D warnings` passes
- [x] `cargo fmt --all -- --check` passes (run `cargo fmt` to fix first)

#### Manual
- [ ] `cargo run`, then `curl -i http://localhost:8080/` returns `HTTP/1.1 200 OK` with `content-type: text/html` and the static body
- [ ] Browse `http://localhost:8080/` shows the static page

---

## Phase 2: Minijinja templated homepage (layout + home)

Swap the static root for a real template render through `AppState`, `WebError`,
and filesystem templates — the actual deliverable.

### Changes

#### 1. Dependency
**File**: `Cargo.toml`
**Action**: modify — add minijinja to `[dependencies]`
```toml
minijinja = { version = "2", features = ["debug"] }
```
**File**: `Cargo.lock`
**Action**: modify (auto) via `cargo build`.

#### 2. Template environment
**File**: `src/app/templates.rs`
**Action**: create — mirror `api` templates.rs:1-11.
```rust
pub fn init() -> minijinja::Environment<'static> {
    let mut templates = minijinja::Environment::new();
    templates.set_loader(minijinja::path_loader("templates"));
    templates.set_auto_escape_callback(|name| {
        if name.ends_with(".html") {
            minijinja::AutoEscape::Html
        } else {
            minijinja::AutoEscape::None
        }
    });
    templates
}
```

#### 3. App state
**File**: `src/app/state.rs`
**Action**: create
```rust
#[derive(Clone)]
pub struct AppState {
    pub templates: minijinja::Environment<'static>,
}
```

#### 4. Web error
**File**: `src/app/error.rs`
**Action**: create — minimal `WebError` with `IntoResponse`.
```rust
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};

pub enum WebError {
    Template(minijinja::Error),
    NotFound,
}

impl From<minijinja::Error> for WebError {
    fn from(err: minijinja::Error) -> Self {
        WebError::Template(err)
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        match self {
            WebError::NotFound => (
                StatusCode::NOT_FOUND,
                "not found",
            )
                .into_response(),
            WebError::Template(err) => {
                eprintln!("template render error: {err}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
                    .into_response()
            }
        }
    }
}
```

#### 5. Routes (stateful)
**File**: `src/interfaces/routes.rs`
**Action**: modify — add the `AppState` type param.
```rust
use axum::{Router, routing::get};

use crate::app::state::AppState;
use crate::interfaces::handlers;

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(handlers::home::web::index))
}
```

#### 6. Handler (render through AppState)
**File**: `src/interfaces/handlers/home/web.rs`
**Action**: modify — extract `State`, render `home.html`.
```rust
use axum::{
    extract::State,
    response::Html,
};
use minijinja::context;

use crate::app::error::WebError;
use crate::app::state::AppState;

pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    let html = state
        .templates
        .get_template("home.html")?
        .render(context! {})?;
    Ok(Html(html))
}
```
Update the `#[cfg(test)]` in this file:
- `start_app()` now returns `SocketAddr` still; the harness (Phase 2 mod.rs)
  builds the state.
- Assert the **composed** markup: body contains `extends`-derived `<title>`
  override, the `heading` override, and the `content` (e.g. `contains("<title>Home</title>")`
  and `contains("This is the vardy homepage")`) — not just the static string.

#### 7. Bootstrap state wiring
**File**: `src/main.rs`
**Action**: modify — build `AppState` and serve the stateful router.
```rust
mod app;
mod interfaces;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = app::state::AppState {
        templates: app::templates::init(),
    };
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(
        listener,
        interfaces::routes::routes()
            .with_state(state)
            .into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}
```

#### 8. Test harness (stateful)
**File**: `src/test/mod.rs`
**Action**: modify — build the `AppState` and serve the stateful router.
```rust
use axum::Router;
use std::net::SocketAddr;

pub async fn start_app() -> SocketAddr {
    let state = crate::app::state::AppState {
        templates: crate::app::templates::init(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let router: Router = crate::interfaces::routes::routes().with_state(state);
    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service())
            .await
            .expect("server");
    });
    addr
}

pub fn test_client() -> reqwest::Client {
    reqwest::Client::new()
}
```

#### 9. Templates
**File**: `templates/layout.html`
**Action**: create — the base layout. Trim `api`'s layout to homepage needs:
`<!DOCTYPE html>`/`<html lang="en">`, `<head>` with `<meta charset>` +
`<meta viewport>`, `<title>{% block title %}Home{% endblock %}</title>`, a small
inline `<style>` block (dark theme vars + `.container` + `.card`), `<body>` with
a `.container`, `<h1>{% block heading %}{% endblock %}</h1>` and
`{% block content %}{% endblock %}`. No nav / `active_*` blocks (single page).

**File**: `templates/home.html`
**Action**: create — overrides `title`, `heading`, `content`.
```html
{% extends "layout.html" %}
{% block title %}Home{% endblock %}
{% block heading %}Welcome to vardy{% endblock %}
{% block content %}
<div class="card">
<p>This is the vardy homepage, rendered with minijinja.</p>
</div>
{% endblock %}
```

### Verification

#### Automated
- [x] `cargo test` passes — 200, `content-type` contains `text/html`, body contains the **composed** markup (`<title>Home</title>`, `Welcome to vardy`, the `.card` content)
- [x] `cargo clippy --all-targets --all-features --locked -- -D warnings` passes
- [x] `cargo fmt --all -- --check` passes

#### Manual
- [ ] `cargo run`, then `curl http://localhost:8080/` returns `200` `text/html` with the rendered, `extends`-composed `<title>Home</title>` and heading
- [ ] Browse `http://localhost:8080/` shows the styled homepage

---

## Phase 3: Coverage & CI gate (verification/hardening)

Close the coverage gap behind `main.rs` and lock the CI gates for all new
non-`main.rs` code (90% patch target), plus template-path robustness.

### Changes

#### 1. Template autoescape else-branch coverage
**File**: `src/app/templates.rs`
**Action**: modify — add a `#[cfg(test)]` unit test covering the non-`.html`
branch the live HTTP test never hits.
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_html_names_are_not_escaped() {
        let env = init();
        let template_names = ["note.txt", "note.html"];
        let escaping = template_names.map(|n| env.auto_escape_callback(n));
        assert_eq!(escaping[0], minijinja::AutoEscape::None);
        assert_eq!(escaping[1], minijinja::AutoEscape::Html);
    }
}
```
> If `Environment::auto_escape_callback` is not a public accessor in the pinned
> minijinja version, instead assert the loader resolves a `.txt` template and
> that rendering leaves `<` unescaped while a `.html` render escapes it. Verify
> the API surface against the pinned `minijinja` before finalizing.

#### 2. WebError branch coverage
**File**: `src/app/error.rs`
**Action**: modify — add a `#[cfg(test)]` unit test so the error branches count
toward (and don't drag down) the 90% patch target. The happy-path HTTP test
never exercises `IntoResponse`'s `Template`/`NotFound` arms or `From`.
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn not_found_is_404() {
        let res = WebError::NotFound.into_response();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn template_error_is_500() {
        let err = minijinja::Error::new(minijinja::ErrorKind::TemplateNotFound, "nope.html");
        let res = WebError::from(err).into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
```

#### 3. Template-path robustness under the test runner
**File**: `src/test/mod.rs` (or `.config/nextest.toml`)
**Action**: verify `path_loader("templates")` (CWD-relative) resolves under the
test runner. Run tests from repo root; if nextest runs with a different CWD,
pin it via `.config/nextest.toml`:
```toml
[profile.ci]
test-threads = "num-cpus"   # optional
```
(Confirm current test CWD by running `cargo test` — repo root is the default, so
no change is typically needed. Only add a config change if the loader fails.)

#### 4. Codecov config
**File**: `codecov.yml`
**Action**: confirm — **unchanged**; `src/main.rs` stays ignored (ignore block
already present), 90% patch target stays. Do not edit.

### Verification

#### Automated
- [ ] `cargo test` passes (all unit + live HTTP tests green)
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] `./scripts/lint_string.sh "FIXME "` and `"fixme "` and `"dbg!"` all exit 0 (no forbidden strings in new `*.rs`)
- [ ] Coverage: `cargo llvm-cov nextest --all-features` locally reports ≥90% patch coverage on new non-`main` `.rs` files (or note that CI computes patch coverage on PR merge)
- [ ] `cargo nextest run --profile ci` passes (matches CI runner)

#### Manual
- [ ] `curl http://localhost:8080/` returns the composed homepage
- [ ] Confirm `codecov.yml` still ignores `src/main.rs` (untouched)

---

## Testing Checkpoints (resume if context resets)

- **After Phase 1**: repo builds; `cargo test` green; `curl /` → `200`
  `text/html` static body; axum/tokio/reqwest wired into `Cargo.lock`.
- **After Phase 2**: templates render via `layout.html`/`home.html` with
  `{% extends %}`; live test asserts composed markup; `AppState`,
  `templates::init`, `WebError`, `Router<AppState>` in place; main serves the
  homepage; `main.rs` stays coverage-ignored.
- **After Phase 3**: 90% patch target met on non-`main` code; CI-equivalent
  suite (`test`/`clippy`/`fmt --check`/`lint_string.sh`) green.

## Notes / deviations from structure

- Added the required `interfaces/mod.rs`, `interfaces/handlers/mod.rs`,
  `handlers/home/mod.rs`, and `src/app/mod.rs` module-declaration files (the
  structure lists only the leaf files; these are needed for the tree to
  compile). They are empty declarations and carry no logic.
- Added a `#[cfg(test)]` test in `src/app/error.rs` (Phase 3) to exercise the
  `WebError` `NotFound`/`Template` `IntoResponse` arms and `From`. The structure
  only names `templates.rs` for explicit new coverage tests, but closing these
  uncovered branches is required to meet the stated 90% patch target (new
  `error.rs` is not codecov-ignored). This is required to satisfy the stated
  gate, not an out-of-scope improvement.
- `start_app` in `src/test/mod.rs` is used as the single harness; no
  `.config/nextest.toml` CWD change is anticipated (repo-root CWD is the default)
  — only added if the loader fails to resolve.
