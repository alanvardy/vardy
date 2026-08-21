# Implementation Plan

## Overview

Extract the inline `<style>` block into `static/site.css`, make every static
asset URL self-versioned via a startup SHA-256 content hash exposed as a
minijinja global function (`asset_url(...)`), and attach a long-lived
immutable `Cache-Control` header to all of `/static`. Missing assets panic at
boot (fail fast).

Verification commands used throughout (match CI):
- `cargo nextest run` (CI: `.github/workflows/ci.yml:62`)
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo fmt --all -- --check`

---

## Phase 1: Immutable `Cache-Control` on all of `/static`

### Changes

#### 1. `Cargo.toml`
**File**: `Cargo.toml`
**Action**: modify — enable the `set-header` feature on tower-http (line 11).

```toml
tower-http = { version = "0.6", features = ["fs", "set-header"] }
```

#### 2. `src/interfaces/routes.rs`
**File**: `src/interfaces/routes.rs`
**Action**: modify — new imports + layered `nest_service` wiring.

```rust
use axum::{
    Router,
    http::{StatusCode, header, HeaderValue},
    routing::get,
};
use tower_http::{services::ServeDir, set_header::SetResponseHeaderLayer};
```

Replace the static mount (currently `routes.rs:12`):

```rust
.nest_service(
    "/static",
    ServeDir::new("static").layer(SetResponseHeaderLayer::overriding(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    )),
)
```

Notes:
- `header::CACHE_CONTROL` comes from `axum::http::header` (http 1.x, same
  version tower-http 0.6 uses) — no new direct dependency on `http` needed.
- `overriding` (not `appending`) so the header is always set, even on 304
  responses.

#### 3. New test in `src/interfaces/routes.rs` `mod tests`
**File**: `src/interfaces/routes.rs`
**Action**: modify — add one test, reusing the `static_icon_is_served`
idiom (`routes.rs:19-33`).

```rust
#[tokio::test]
async fn static_files_have_immutable_cache_control() {
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
            .get("cache-control")
            .is_some_and(|v| v.to_str().unwrap().contains("max-age=31536000"))
    );
}
```

### Verification

#### Automated
- [x] `cargo nextest run` passes (new test + all existing tests green)
- [x] `cargo clippy --all-targets --all-features --locked -- -D warnings` passes
- [x] `cargo fmt --all -- --check` passes

#### Manual
- [ ] `cargo run`, then `curl -I http://localhost:3000/static/singlethread-icon.png`
      shows `cache-control: public, max-age=31536000, immutable`

---

## Phase 2: Startup asset hashing + `asset_url` template global

### Changes

#### 1. New module `src/app/assets.rs`
**File**: `src/app/assets.rs`
**Action**: create.

```rust
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Startup asset hashes, keyed by path relative to `static/`.
/// Lazily computed on first use (i.e., during `templates::init()`).
static ASSET_HASHES: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Hash every file under `dir` (recursive). Panics, naming the path, if the
/// directory or any file cannot be read — fail fast on broken deploys.
pub fn hash_all(dir: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    hash_dir(Path::new(dir), dir.len() + 1, &mut map);
    map
}

fn hash_dir(dir: &Path, prefix_len: usize, map: &mut HashMap<String, String>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|err| {
        panic!("failed to hash static asset {}: {err}", dir.display())
    });
    for entry in entries {
        let path = entry.expect("readable dir entry").path();
        if path.is_dir() {
            hash_dir(&path, prefix_len, map);
        } else {
            let bytes = std::fs::read(&path).unwrap_or_else(|err| {
                panic!("failed to hash static asset {}: {err}", path.display())
            });
            let digest = Sha256::digest(&bytes);
            let rel = path.to_string_lossy()[prefix_len..].to_string();
            map.insert(rel, format!("{digest:x}")[..12].to_string());
        }
    }
}

/// `/static/<file>?v=<12-hex sha256 prefix>`. Panics on unknown files.
pub fn asset_url(file: &str) -> String {
    let hashes = ASSET_HASHES.get_or_init(|| hash_all("static"));
    match hashes.get(file) {
        Some(hash) => format!("/static/{file}?v={hash}"),
        None => panic!("unknown static asset {file}"),
    }
}
```

Notes:
- Repo-relative `"static"` matches the existing `ServeDir::new("static")`
  (`routes.rs:12`) / `path_loader("templates")` (`templates.rs:4`) CWD
  convention (Dockerfile `WORKDIR /app`).
- 12-hex SHA-256 prefix per design decision 1; deterministic across rebuilds.
- `OnceLock::get_or_init` makes repeated `templates::init()` calls in tests
  safe (later calls reuse the first map; `static/` doesn't change mid-run).
- `sha2 = "0.10"` must be added to `[dependencies]` (see next change). It is
  already in `Cargo.lock` transitively, so no lockfile surprises.

#### 2. `Cargo.toml`
**File**: `Cargo.toml`
**Action**: modify — add sha2 dependency.

```toml
sha2 = "0.10"
```

#### 3. `src/app/mod.rs`
**File**: `src/app/mod.rs`
**Action**: modify — declare the new module.

```rust
pub mod assets;
```

#### 4. `src/app/templates.rs` — register the global function
**File**: `src/app/templates.rs`
**Action**: modify — register `asset_url` inside `init()` before returning.

```rust
use crate::app::assets;

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
    templates.add_global_function("asset_url", |file: String| {
        Ok::<String, minijinja::Error>(assets::asset_url(&file))
    });
    templates
}
```

Notes:
- `add_global_function` accepts a closure with typed args returning
  `Result<T, minijinja::Error>`; the explicit `Ok::<...>` annotation
  satisfies type inference. If clippy/rustc still balks on the closure
  shape, fall back to a named `fn asset_url_global(file: String) ->
  Result<String, minijinja::Error>` passed by path.
  **[Phase 2 note]** minijinja 2 exposes this as `add_function`, and the
  closure returns `Value::from_safe_string(...)` — without it, HTML
  auto-escape escapes the `/` in the URL (`&#x2f;`), which would break
  Phase 3's body-substring assertions.
- Handler contexts stay `context! {}` untouched (design decision 1).

#### 5. Unit tests in `src/app/assets.rs`
**File**: `src/app/assets.rs`
**Action**: create — `#[cfg(test)] mod tests` covering:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_file_yields_versioned_url() {
        let url = asset_url("singlethread-icon.png");
        assert!(url.starts_with("/static/singlethread-icon.png?v="));
        let hash = url.rsplit("?v=").next().unwrap();
        assert_eq!(hash.len(), 12);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hashes_are_deterministic() {
        let a = hash_all("static");
        let b = hash_all("static");
        assert_eq!(a, b);
    }

    #[test]
    #[should_panic(expected = "failed to hash static asset")]
    fn unreadable_directory_panics() {
        hash_all("static/does-not-exist");
    }

    #[test]
    #[should_panic(expected = "unknown static asset")]
    fn unknown_file_panics() {
        // Ensure initialized even if another test ran first.
        let _ = ASSET_HASHES.get_or_init(|| hash_all("static"));
        asset_url("nope.css");
    }
}
```

#### 6. Template-render test in `src/app/templates.rs`
**File**: `src/app/templates.rs`
**Action**: modify — add a test following the existing sync minijinja
pattern (`templates.rs:14+`).

```rust
#[test]
fn asset_url_function_resolves_in_templates() {
    let mut env = init();
    env.add_template("page.html", r#"{{ asset_url('site.css') }}"#)
        .unwrap();
    let out = env
        .get_template("page.html")
        .unwrap()
        .render(minijinja::context! {})
        .unwrap();
    assert!(out.starts_with("/static/site.css?v="));
}
```

Note: `site.css` does not exist until Phase 3. This test only requires the
hash map to contain the key, so either (a) add this test in Phase 3, or
(b) create a placeholder `static/site.css` in Phase 2. Prefer (a): add the
test in Phase 3 alongside the real file, and in Phase 2 use
`{{ asset_url('singlethread-icon.png') }}` instead.

### Verification

#### Automated
- [x] `cargo nextest run` passes — all new assets/templates tests green
- [x] `cargo clippy --all-targets --all-features --locked -- -D warnings` passes
- [x] `cargo fmt --all -- --check` passes

#### Manual
- [ ] Temporarily `chmod 000 static/singlethread-icon.png`, run `cargo run` →
      process panics at startup with `failed to hash static asset ...`;
      restore permissions. (Or rename `static/` briefly — same panic.)

---

## Phase 3: CSS extraction + versioned asset references in templates

### Changes

#### 1. `static/site.css`
**File**: `static/site.css`
**Action**: create — copy the CSS **verbatim** from the `<style>` block in
`templates/layout.html` (everything between `<style>` and `</style>`, i.e.
layout.html lines 8–56, the rules for `:root`, `*`, `body`, `.container`,
`.card`, `nav`, `nav a`). Byte-identical extraction; no reformatting.

#### 2. `templates/layout.html`
**File**: `templates/layout.html`
**Action**: modify — delete the entire `<style>...</style>` block
(`layout.html:7-57`) and replace it with:

```html
    <link rel="stylesheet" href="{{ asset_url('site.css') }}">
```

placed in the same `<head>` position (after the viewport meta, before
`</title>`'s sibling order is preserved — i.e., where the style block was).

#### 3. `templates/singlethread.html`
**File**: `templates/singlethread.html`
**Action**: modify — version the icon reference (`singlethread.html:6`).

```html
<img src="{{ asset_url('singlethread-icon.png') }}" alt="SingleThread icon" width="96" height="96">
```

#### 4. `src/interfaces/handlers/home/web.rs` — extend page test
**File**: `src/interfaces/handlers/home/web.rs`
**Action**: modify — append to the body assertions in `index_serves_ok_html`:

```rust
assert!(body.contains("/static/site.css?v="));
assert!(!body.contains("<style>"));
```

#### 5. `src/interfaces/handlers/singlethread/web.rs` — extend page test
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify — the existing assertion
`body.contains(r#"<img src="/static/singlethread-icon.png""#)` still passes
unchanged (rendered output is `/static/singlethread-icon.png?v=<hash>`, a
superset). Add one versioned-URL assertion:

```rust
assert!(body.contains("/static/singlethread-icon.png?v="));
```

#### 6. `src/interfaces/routes.rs` — cache header on the new CSS file
**File**: `src/interfaces/routes.rs`
**Action**: modify — extend `static_files_have_immutable_cache_control`
(from Phase 1) to also hit the CSS file (design decision 6):

```rust
let res = client
    .get(format!("http://{addr}/static/site.css"))
    .send()
    .await
    .expect("request failed");
assert_eq!(res.status(), StatusCode::OK);
assert!(
    res.headers()
        .get("cache-control")
        .is_some_and(|v| v.to_str().unwrap().contains("max-age=31536000"))
);
```

(Rename the test to `static_files_have_immutable_cache_control` covering both
assets, or add a second test — either is fine.)

#### 7. Phase-2 template test now uses the real file
**File**: `src/app/templates.rs`
**Action**: modify — if the Phase-2 test used
`singlethread-icon.png`, it can stay as-is; optionally add a second
assertion for `site.css` now that the file exists (see Phase 2 change 6,
option (a)).

### Verification

#### Automated
- [x] `cargo nextest run` passes — all pre-existing body-substring assertions
      (`<title>Home</title>`, `Welcome to vardy`, `single line of work`,
      nav links, `<img src="/static/singlethread-icon.png"`) still pass
      unchanged, plus the new versioned-URL / no-`<style>` assertions
- [x] `cargo clippy --all-targets --all-features --locked -- -D warnings` passes
- [x] `cargo fmt --all -- --check` passes

#### Manual
- [ ] `cargo run`, load `http://localhost:3000/` in a browser: styles render
      identically to before (dark theme, nav, cards); no `<style>` block in
      view-source
- [ ] Browser network tab: `/static/site.css?v=<hash>` returns 200 with
      `cache-control: public, max-age=31536000, immutable`
- [ ] `curl -I http://localhost:3000/static/site.css` shows the immutable
      header

---

## Testing Checkpoints (from structure.md)

- **After Phase 1**: full suite green; `GET /static/*` returns 200 with
  `cache-control: …max-age=31536000, immutable`. Safe stopping point — no
  templates touched.
- **After Phase 2**: full suite green; `asset_url('singlethread-icon.png')`
  renders `/static/singlethread-icon.png?v=<12 hex>`; unreadable assets panic
  at init. Templates still unversioned — nothing user-visible changed.
- **After Phase 3**: full suite green; rendered HTML has versioned
  `<link>`/`<img>` URLs, zero `<style>` blocks, all pre-existing body
  assertions intact.
- **Post-deploy (manual)**: `curl -I https://<prod>/static/site.css?v=<hash>`
  confirms the header survives fly.io's edge; health check passes on deploy.

No schema migrations and no codegen steps are involved in this plan.
