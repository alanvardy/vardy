# Implementation Plan

## Overview

A persistent top nav bar on every page linking Home and SingleThread, a new
`/singlethread` marketing-style page with the SingleThread icon, and static
asset serving via `tower-http`'s `ServeDir` from a new `static/` directory.

Project commands: build/lint with `cargo build`, tests with `cargo test`
(no separate linter configured). Tests use the real-socket harness in
`src/test/mod.rs`.

---

## Phase 1: Static Asset Serving + Icon Asset

### Changes

#### 1. Add `tower-http` dependency
**File**: `Cargo.toml`
**Action**: modify

Add under `[dependencies]` (current `axum` is 0.8.9; tower-http 0.6 is the
compatible line and its `fs` feature does not pin an axum version):

```toml
tower-http = { version = "0.6", features = ["fs"] }
```

Then run `cargo add tower-http --features fs` OR edit manually + `cargo build`
to update `Cargo.lock`. If codegen/registry resolution fails (offline), fall
back to checking `Cargo.lock` for an already-vendored compatible version;
there is none today, so the dependency is mandatory — stop and ask if it
cannot be fetched.

#### 2. Generate the icon asset
**File**: `static/singlethread-icon.png`
**Action**: create (generated, committed to repo)

```fish
mkdir -p static
sips -Z 256 ~/Downloads/AppIcon2.png --out static/singlethread-icon.png
```

Source (`~/Downloads/AppIcon2.png`, 1024×1024 RGBA, 1.4 MB) stays out of the
repo. Verify result is ~256×256 PNG before committing. If muddy in the browser
(manual check below), regenerate with `sips -Z 512`.

#### 3. Nest ServeDir route
**File**: `src/interfaces/routes.rs`
**Action**: modify

```rust
use axum::{Router, routing::get};
use tower_http::services::ServeDir;

use crate::app::state::AppState;
use crate::interfaces::handlers;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(handlers::home::web::index))
        .nest_service("/static", ServeDir::new("static"))
}
```

`nest_service` on `Router<AppState>` requires no state; keep `routes()` the
single source of routes.

#### 4. Include `static/` in the deploy image
**File**: `Dockerfile`
**Action**: modify

The runtime stage currently copies only templates. Add static alongside:

```dockerfile
COPY --from=builder /app/templates ./templates
COPY --from=builder /app/static ./static
```

(`fly.toml` needs no change.)

#### 5. HTTP test for the icon
**File**: `src/interfaces/routes.rs` (colocated `#[cfg(test)] mod tests`)
**Action**: modify

Append at bottom of file, following the project's colocated-test style:

```rust
#[cfg(test)]
mod tests {
    use crate::test::{start_app, test_client};
    use axum::http::StatusCode;

    #[tokio::test]
    async fn static_icon_is_served() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/static/singlethread-icon.png"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()
                .get("content-type")
                .is_some_and(|v| v.to_str().unwrap().contains("image/png"))
        );
    }
}
```

Note: `start_app()` binds from cwd-relative paths (`templates/`, `static/`) —
tests must be run from the repo root (`cargo test` default).

### Verification

#### Automated
- [x] `cargo build` passes (dependency resolved, `Cargo.lock` updated)
- [x] `cargo test` passes, including new `static_icon_is_served`
- [x] Existing home test (`index_serves_ok_html`) still passes unchanged

#### Manual
- [ ] `cargo run`, open `http://localhost:3000/static/singlethread-icon.png` — icon renders, looks crisp at 256px (regenerate at 512px if muddy)

---

## Phase 2: SingleThread Page

### Changes

#### 1. Register handler module
**File**: `src/interfaces/handlers/mod.rs`
**Action**: modify

```rust
pub mod home;
pub mod singlethread;
```

#### 2. Handler module files
**File**: `src/interfaces/handlers/singlethread/mod.rs`
**Action**: create

```rust
pub mod web;
```

**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: create

Clone of `home/web.rs` handler shape (empty context, `WebError` propagation):

```rust
use axum::{extract::State, response::Html};
use minijinja::context;

use crate::app::error::WebError;
use crate::app::state::AppState;

pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    let html = state
        .templates
        .get_template("singlethread.html")?
        .render(context! {})?;
    Ok(Html(html))
}
```

Colocated test module asserting status, content-type, and page substrings.
Nav-link assertions are deliberately deferred to Phase 3 so each phase stays
independently green:

```rust
#[cfg(test)]
mod tests {
    use crate::test::{start_app, test_client};
    use axum::http::StatusCode;

    #[tokio::test]
    async fn index_serves_ok_html() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/singlethread"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()
                .get("content-type")
                .is_some_and(|v| v.to_str().unwrap().contains("text/html"))
        );
        let body = res.text().await.unwrap();
        assert!(body.contains("<title>SingleThread</title>"));
        assert!(body.contains("<h1>SingleThread</h1>"));
        assert!(body.contains("One thread."));
        assert!(body.contains(r#"<img src="/static/singlethread-icon.png""#));
    }
}
```

(Substring assertions must match the final copy chosen for the template — see §4.)

#### 3. Route registration
**File**: `src/interfaces/routes.rs`
**Action**: modify

Lowercase `/singlethread` per design decision 3 (note on PR #8 vs VAR-657);
no redirect for `/SingleThread`:

```rust
    Router::new()
        .route("/", get(handlers::home::web::index))
        .route("/singlethread", get(handlers::singlethread::web::index))
        .nest_service("/static", ServeDir::new("static"))
```

#### 4. Page template
**File**: `templates/singlethread.html`
**Action**: create

Extends layout; overrides all three blocks. Original copy (intro paragraph +
feature list), icon near heading sized explicitly to prevent layout jump.
If any assertion copy differs from what you write here, update the test
substrings in §2 to match exactly:

```html
{% extends "layout.html" %}
{% block title %}SingleThread{% endblock %}
{% block heading %}SingleThread{% endblock %}
{% block content %}
<img src="/static/singlethread-icon.png" alt="SingleThread icon" width="96" height="96">
<div class="card">
<p>SingleThread is a focused companion app that keeps your day moving along one thread. No boards, no backlogs, no noise — just the single line of work in front of you, from start to finish.</p>
<ul>
<li><strong>One task at a time.</strong> Your work lives on a single thread, so attention never fragments.</li>
<li><strong>Momentum built in.</strong> Finish a step and the next one is already waiting.</li>
<li><strong>Calm by design.</strong> A dark, quiet interface that stays out of your way.</li>
</ul>
</div>
{% endblock %}
```

No Rust template registration needed (`path_loader` reads disk at runtime).

### Verification

#### Automated
- [x] `cargo test` passes, including new `handlers::singlethread::web::tests::index_serves_ok_html`
- [x] All Phase 1 tests still pass

#### Manual
- [ ] Load `http://localhost:3000/singlethread` — page renders, copy reads well, icon displays at 96px without layout jump
- [ ] `http://localhost:3000/SingleThread` correctly returns axum's default 404 (case-sensitive routing, intentional)

---

## Phase 3: Persistent Nav Bar

### Changes

#### 1. Nav element + CSS in shared layout
**File**: `templates/layout.html`
**Action**: modify

Two edits, both inside existing structure — no Rust changes.

**Edit A — CSS rules** added inside the `<style>` block after `.card`,
consuming the previously unused tokens (`--surface`, `--accent`; also put
`--muted` to work for the secondary link treatment):

```css
        nav {
            display: flex;
            gap: 1.5rem;
            padding: 0.75rem 1.5rem;
            background: var(--surface);
            border-bottom: 1px solid #333;
        }

        nav a {
            color: var(--text);
            text-decoration: none;
        }

        nav a:hover {
            color: var(--accent);
        }
```

No `.active`/current-page selector: there is no dynamic context to detect the
current page, and the design forbids adding unused code paths.

**Edit B — markup** immediately inside `<body>`, before
`<div class="container">`, outside both blocks so every page inherits it:

```html
<body>
    <nav>
        <a href="/">Home</a>
        <a href="/singlethread">SingleThread</a>
    </nav>
    <div class="container">
```

#### 2. Cross-link assertions in home test
**File**: `src/interfaces/handlers/home/web.rs` (existing `mod tests`)
**Action**: modify

Add to the existing `index_serves_ok_html` body-substring assertions:

```rust
        assert!(body.contains(r#"<a href="/">Home</a>"#));
        assert!(body.contains(r#"<a href="/singlethread">SingleThread</a>"#));
```

#### 3. Cross-link assertions in singlethread test
**File**: `src/interfaces/handlers/singlethread/web.rs` (existing `mod tests`)
**Action**: modify

Add the same two substring assertions to `index_serves_ok_html`.

### Verification

#### Automated
- [ ] `cargo test` passes — both `/` and `/singlethread` bodies contain `href="/"…Home</a>` and `href="/singlethread"…SingleThread</a>`
- [ ] All Phase 1 and Phase 2 tests still pass

#### Manual
- [ ] Click Home ↔ SingleThread in the browser; navigation works both directions
- [ ] Nav renders identically on both pages; hover turns link text `--accent` blue on dark surface

---

## Testing Checkpoints (resume guide)

After each phase, `cargo test` green means:
1. **Phase 1**: `/static/singlethread-icon.png` → 200 `image/png`. Home untouched.
2. **Phase 2**: `/singlethread` → 200 `text/html` with expected title/copy/icon tag.
3. **Phase 3**: Both pages contain both nav links.

Resume point if context resets: run `cargo test`; first failing checkpoint is
where work resumes.

## Notes

- PR #8 against VAR-657: record decision that route path is lowercase
  `/singlethread` (deviates from ticket's literal `/SingleThread`; URLs are
  case-sensitive in axum).
- No schema/codegen steps exist in this project — N/A.
