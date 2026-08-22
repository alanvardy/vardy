# Research Findings

## Q1: Full request-to-response flow for `/singlethread`

### Findings
- Route registration: `src/interfaces/routes.rs:14` — `.route("/singlethread", get(handlers::singlethread::web::index))` inside `routes() -> Router<AppState>` (`src/interfaces/routes.rs:10`); module declared at `src/interfaces/handlers/mod.rs:4`.
- Handler `src/interfaces/handlers/singlethread/web.rs:7-14`:
  ```rust
  pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
      state.metrics.inc_page_view("singlethread");
      let html = state.templates.get_template("singlethread.html")?.render(context! {})?;
      Ok(Html(html))
  }
  ```
- **Rendering context is empty** (`context! {}`, `web.rs:12`) — every value in the page comes from template literals plus the global `asset_url` function.
- Metrics: `inc_page_view("singlethread")` (`web.rs:8`) increments `page_views_total{page="singlethread"}` (`IntCounterVec` labeled `page`, defined `src/infra/metrics.rs:11-23`, `inc_page_view` at `metrics.rs:22-24`). Exposed on a separate router/port: `metrics_router` at `src/interfaces/routes.rs:33-38`, served on port 9090 (`src/main.rs:41-44`).
- Template environment: `app::templates::init()` (`src/app/templates.rs:3-18`) — `path_loader("templates")`, auto-escape `Html` for `.html` names, global `asset_url` registered at `templates.rs:14-16` returning `Value::from_safe_string` (not escaped).
- Errors propagate via `?` into `WebError::Template(minijinja::Error)` (`src/app/error.rs:11,17`).
- Layout inheritance: `templates/singlethread.html:1` `{% extends "layout.html" %}`; overrides blocks `title` (line 2), `heading` (line 3), `content` (lines 4-14).
- `templates/layout.html` structure: `<title>{% block title %}</title>` (line 7), stylesheet link `{{ asset_url('site.css') }}` (line 8), bare `<nav>` with links to `/` and `/singlethread` (lines 10-13), `<div class="container">` wrapping `<h1>{% block heading %}</h1>` and `{% block content %}` (lines 14-18).
- CSS classes the singlethread render relies on (`static/site.css`): `:root` theme vars (lines 1-7), `body` (13-19), `.container` (21-25), `.card` (27-32), `nav`/`nav a`/`nav a:hover` (34-49). The icon `<img>` (singlethread.html:5, 96×96) has no dedicated CSS rule; homepage-only classes (`.home*`, `.portrait`, `.invite-list`, `.section-heading`, `.wave`) are unused here.
- Effective DOM: `nav > a[href=/], a[href=/singlethread]` then `div.container > h1` + `img` + `div.card` (card contains one `<p>` with "single line of work" copy and a 3-item `<ul>` with `<strong>` lead-ins, singlethread.html:6-13).

## Q2: Static assets — serving, cache busting, `asset_url`

### Findings
- `asset_url` (`src/app/assets.rs:36-43`): returns `/static/<file>?v=<12-hex sha256 prefix>`. Hashes are computed lazily once into a process-wide `OnceLock<HashMap<String,String>>` (`assets.rs:8`, `hash_all`/`hash_dir` at `assets.rs:12-34`) walking `static/` recursively. Unknown filenames **panic** (`assets.rs:42`).
- Registered as minijinja global at `src/app/templates.rs:14-16` (safe string, unescaped).
- `/static` service (`src/interfaces/routes.rs:21-28`): `nest_service("/static", SetResponseHeader::overriding(ServeDir::new("static"), CACHE_CONTROL, "public, max-age=31536000, immutable"))` — every static response gets one-year immutable caching, which is why URLs carry the content-hash `?v=`.
- Existing assets live **flat** in `static/`: `alanvardy.jpg`, `github.svg`, `linkedin.svg`, `quill.png`, `singlethread-icon.png`, `wave.svg`, `site.css`.
- Two reference styles coexist in templates:
  - Versioned (current pattern): `{{ asset_url('site.css') }}` (`templates/layout.html:8`), `{{ asset_url('singlethread-icon.png') }}` (`templates/singlethread.html:5`).
  - Literal unversioned (older): `/static/wave.svg`, `/static/github.svg`, `/static/linkedin.svg`, `/static/alanvardy.jpg` hardcoded in `templates/home.html:4,24,30,37`.
- Asset tests: unit tests in `src/app/assets.rs:49-77` (URL shape, determinism, panic on unknown) and `src/app/templates.rs:51-62` (resolves in template); HTTP tests in `src/interfaces/routes.rs` — icon served as `image/png` (~lines 43-56), immutable cache-control (~96-109), homepage image and stylesheet served (~125, ~142).

## Q3: Layout/styling patterns across templates

### Findings
- Only three templates exist: `layout.html`, `home.html`, `singlethread.html`; all extend `layout.html` and share the single `static/site.css`.
- Richer patterns are concentrated in `templates/home.html`:
  - Two-column flex: `<div class="home-columns">` (home.html:7) with `.home-text` (`flex: 3`) and `.home-portrait` (`flex: 1`) — `site.css:57-62`; media query at `max-width: 48rem` stacks columns and reorders portrait above text (`site.css` ~107-116).
  - Portrait image section: `<img class="portrait" src="/static/alanvardy.jpg">` (home.html:25-26); CSS max-width 200px, radius 8px, 1px #333 border (`site.css:64-68`).
  - Icon-in-heading: `<img class="wave">` inside the h1 heading block (home.html:4-6), `vertical-align: middle` (`site.css:52-55`).
  - Link list with icons: `<ul class="invite-list">` (home.html:14-23), accent left-border callout (`site.css:78-83`), flex rows with 32×32 icons (`site.css:85-100`).
  - Typography: `.section-heading` (home.html:12; muted color, 1.25rem, `site.css:70-74`); body copy is unclassed `<p>`.
- Singlethread page uses only `.card` (singlethread.html:6; `site.css:27-32`) and a bare unclassed `<img>`.
- Design tokens: dark theme `--bg/--surface/--text/--muted/--accent` in `:root` (`site.css:1-7`); global `box-sizing` reset (9-11); body system font stack, line-height 1.6 (13-19).
- What does not exist: no CSS grid (flex only), no screenshot/mockup layout, no card grids (one `.card` instance total), no footer, no typography scale beyond `.section-heading`.

## Q4: Page-handler tests and what singlethread tests assert

### Findings
- Harness: `start_app()` (`src/test/mod.rs:11-40`) builds in-memory SQLite (`sqlite::memory:`), runs migrations, uses real templates via `app::templates::init()` (`mod.rs:27`), binds `127.0.0.1:0`, serves the real router in a spawned tokio task. `test_client()` is a plain `reqwest::Client` (`mod.rs:87-89`). Tests are inline `#[cfg(test)] mod tests` with `#[tokio::test]`.
- Singlethread test `index_serves_ok_html` (`src/interfaces/handlers/singlethread/web.rs:21-43`) asserts:
  - Status `StatusCode::OK` (web.rs:30)
  - Content-type contains `text/html` (web.rs:31-35)
  - Body contains `<title>SingleThread</title>` (web.rs:37)
  - Body contains `<h1>SingleThread</h1>` (web.rs:38)
  - Body contains `single line of work` (web.rs:39 — literal copy phrase from singlethread.html:7)
  - Body contains `<img src="/static/singlethread-icon.png?v=` (web.rs:40 — versioned asset URL)
  - Body contains `<a href="/">Home</a>` and `<a href="/singlethread">SingleThread</a>` (web.rs:41-42 — layout nav)
- No negative assertions in this test.
- Cross-file coupling: `src/interfaces/handlers/home/web.rs:49` also asserts `<a href="/singlethread">SingleThread</a>` ("nav chrome unchanged") — any nav change in `templates/layout.html:12` affects both tests.
- Adjacent static tests touching the same icon: `static_icon_is_served` (200 + `image/png`) and `static_files_have_immutable_cache_control` (`max-age=31536000`) in `src/interfaces/routes.rs`.
- Metrics integration test pattern: `src/test/mod.rs:140-159` asserts `/metrics` exposes `page_views_total` with a given `page` label.

## Q5: What must be updated when page markup changes

### Findings
- `ROUTES.md` organization: one self-contained section per endpoint — `### METHOD /path` heading, description, bullet lists for Response/Errors, terminated by `---`. The `/singlethread` section is `ROUTES.md:14-19`; it documents only status code, content type, template name, and error path — **content-only markup changes do not require a ROUTES.md edit** unless route/response semantics change. Mandated by `AGENTS.md:66-69`.
- Metrics label tied to the page: `"singlethread"` passed to `inc_page_view` (`web.rs:8`) → `page_views_total{page="singlethread"}` (`src/infra/metrics.rs:11-23`). Label only changes if the handler changes.
- Nav references to the page: `templates/layout.html:12`; asserted in `web.rs:42` and `src/interfaces/handlers/home/web.rs:49`.
- Update surface for a markup change: `templates/singlethread.html` itself; body assertions at `src/interfaces/handlers/singlethread/web.rs:37-42` (title, h1, "single line of work", img `?v=` tag, both nav links); `static/site.css` if classes change; ROUTES.md only for route/response changes; `AGENTS.md:14` mentions `singlethread/web.rs` as an example handler location (documentation only).
- `README.md` has no singlethread references; archived ticket artifacts under `.pi/qrspi/**` reference singlethread but are not live docs.

## Cross-Cutting Observations
- The singlethread handler passes an **empty render context**; all page content is static template markup + `asset_url` output. Any dynamic data would require both handler and template changes.
- Cache busting is content-hash based (`?v=<12-hex>`) and pairs with `Cache-Control: public, max-age=31536000, immutable` on `/static`; `asset_url` panics on unknown files, so a new asset must exist under `static/` before any template references it.
- Two asset-reference styles coexist: versioned `asset_url(...)` (layout.html:8, singlethread.html:5) vs literal `/static/...` paths (home.html). The versioned style is what the singlethread test asserts on.
- Test assertions on rendered HTML are `body.contains(...)` string checks, so markup changes that alter exact tag text (`<title>`, `<h1>`, `<img src=...?v=`, nav `<a>` tags) or the copy phrase "single line of work" will fail tests in two files (`singlethread/web.rs`, `home/web.rs`).
- All handler errors flow through `WebError`'s `IntoResponse` (`src/app/error.rs`); template render failures map to `WebError::Template` → 500.

## Open Areas
- No screenshot/mockup or grid layout patterns exist in the codebase to model richer presentations on — the closest precedents are the homepage flex columns (`.home-columns`) and the `.card` component.
- The `/metrics` endpoint details beyond `page_views_total` (e.g., other metric families in `AppMetrics`) were not exhaustively surveyed; only the page-view counter is tied to singlethread.
