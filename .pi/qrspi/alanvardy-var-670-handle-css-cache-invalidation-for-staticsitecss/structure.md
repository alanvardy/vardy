# Structure Outline

Branch: `alanvardy-var-670-handle-css-cache-invalidation-for-staticsitecss`

## Approach
Extract the inline `<style>` block into `static/site.css`, then make every
static asset URL self-versioned via a startup content hash exposed as a
minijinja global function (`asset_url(...)`), and put a long-lived immutable
`Cache-Control` header on `/static`. Hashes are computed once at boot;
missing assets panic (fail fast).

Phase order rationale: the header layer is independent and immediately
testable; the hash machinery is next because templates can't reference it
until it exists; extraction/versioned URLs land last and consume both.

---

## Phase 1: Immutable `Cache-Control` on all of `/static`

Delivers end-to-end: every `GET /static/*` response carries
`Cache-Control: public, max-age=31536000, immutable`, verified by a live-HTTP
test against the real production router. No other behavior changes.

**Files**: `Cargo.toml`, `src/interfaces/routes.rs`
**Key changes**:
- `Cargo.toml`: tower-http features `["fs"]` → `["fs", "set-header"]`
- `use tower_http::set_header::SetResponseHeaderLayer;` — new import in routes.rs
- `.nest_service("/static", ServeDir::new("static").layer(SetResponseHeaderLayer::overriding(header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000, immutable"))))` — modified wiring

**Verify**: `cargo test` passes — new test asserts `cache-control` contains
`max-age=31536000` on `GET /static/singlethread-icon.png` (reuse the
`static_icon_is_served` idiom at `routes.rs:19-33`). Manually: `curl -I`
against a dev server shows the header.

---

## Phase 2: Startup asset hashing + `asset_url` template global

Delivers end-to-end: a new module hashes everything under `static/` at
startup (SHA-256, 12-hex prefix) and registers a minijinja global function
so any template can render `/static/<file>?v=<hash>`. Panics naming the path
if an asset is unreadable. Templates don't use it yet — but it's fully
exercised by its own tests.

**Files**: `src/app/assets.rs` (new), `src/app/templates.rs`, `src/app/mod.rs`
(or wherever `app` modules are declared)
**Key changes**:
- `pub fn hash_all(dir: &str) -> HashMap<String, String>` — panics with `failed to hash static asset {path}: {err}` on unreadable files
- `pub fn asset_url(file: &str) -> String` → `/static/site.css?v=a1b2c3d4e5f6`; unknown file → panic
- `Environment::add_global_function("asset_url", ...)` registered inside `templates::init()` before returning

**Verify**: `cargo test` passes — unit tests for: known file yields
`/static/site.css?v=<12 hex>`; missing file panics; rendered template string
containing `{{ asset_url('site.css') }}` resolves correctly (follows the
existing sync minijinja test pattern in `templates.rs:14+`). Determinism:
same input bytes → same hash across two calls.

---

## Phase 3: CSS extraction + versioned asset references in templates

Delivers end-to-end: styling moves verbatim from `layout.html` into
`static/site.css`, the layout links to it via `asset_url('site.css')`, and
the icon reference in `singlethread.html` switches to
`asset_url('singlethread-icon.png')`. Rendered pages show versioned URLs and
no inline `<style>` block; page tests assert this.

**Files**: `static/site.css` (new), `templates/layout.html`,
`templates/singlethread.html`,
`src/interfaces/handlers/home/web.rs`,
`src/interfaces/handlers/singlethread/web.rs`
**Key changes**:
- `<link rel="stylesheet" href="{{ asset_url('site.css') }}">` replaces the `<style>` block (`layout.html:7-57`)
- `<img src="{{ asset_url('singlethread-icon.png') }}">` replaces the literal path (`singlethread.html:6`)
- Test assertions added: home body contains `/static/site.css?v=` and no `<style>`; singlethread body contains `/static/singlethread-icon.png?v=`

**Verify**: `cargo test` passes — existing body-substring assertions must
still pass unchanged (CSS moved byte-identical); new assertions per above.
Manually: load `/` in a browser, confirm styles render identically and the
network tab shows `/static/site.css?v=…` with status 200 and the immutable
cache header.

---

## Testing Checkpoints

- **After Phase 1**: full suite green; `GET /static/*` returns 200 with `cache-control: …max-age=31536000, immutable`. Safe stopping point — no templates touched.
- **After Phase 2**: full suite green; `asset_url('site.css')` renders `/static/site.css?v=<12 hex>`; missing assets panic at init. Templates still unversioned — nothing user-visible changed.
- **After Phase 3**: full suite green; rendered HTML has versioned `<link>`/`<img>` URLs, zero `<style>` blocks, all pre-existing body assertions intact.
- **Post-deploy (manual)**: `curl -I https://<prod>/static/site.css?v=<hash>` confirms the header survives fly.io's edge; health check passes on deploy.

Note on slicing: Phase 2 is the only phase without a user-visible change —
it's pure foundation that Phase 3 consumes. Splitting it further would be
horizontal (all hashing, then all template registration), so it stays whole.
