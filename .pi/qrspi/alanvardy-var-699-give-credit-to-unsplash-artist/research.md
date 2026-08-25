# Research Findings

## Q1: Home page render flow (handler → `Picture` → context → `layout.html`)

### Findings
- Route wiring: `src/interfaces/routes.rs:38` registers `.route("/", get(handlers::home::web::index))`. The same axum state (`AppState`) is shared by all routes via `.with_state(state)` (`src/test/mod.rs:110`).
- Handler entry: `src/interfaces/handlers/home/web.rs:11` `pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError>`. First line increments a page-view metric: `state.metrics.inc_page_view("home")` (`web.rs:12`).
- The only context value is the wallpaper: `web.rs:16`
  `let wallpaper_url = picture::current(&state).await.ok().map(|p| p.url);`
  `.ok()` swallows *any* `WebError` and `.map(|p| p.url)` extracts **only** `Picture.url`. Comment (`web.rs:14-15`): "The wallpaper is decorative: render the page without it rather than failing the whole request if Unsplash is unavailable."
- Context build + render: `web.rs:18-20`
  `state.templates.get_template("home.html")?.render(context! { wallpaper_url })?`
  `context!` from `minijinja::context` (imported `web.rs:5`) creates a single-key context binding the local `Option<String>` as `wallpaper_url`. Any error from template fetch or render propagates through `?` as `WebError` (handled by `WebError`'s `IntoResponse`, `src/app/error.rs`).
- `picture::current` (`src/app/picture.rs:17`): if `latest(&state.db)` returns a Picture that is **not** stale it is returned from cache; otherwise `fetch_random(...)` (Unsplash API) is called and persisted via `create(...)`. `picture.rs` re-exports `fetch_random` from infra: `pub use crate::infra::unsplash::fetch_random;` (`picture.rs:4`) — interfaces reach the Unsplash layer only through `app` (comment at `picture.rs:1`).
- `latest` (`src/app/picture.rs:20-26`): `SELECT url, photographer, photographer_url, created_at FROM unsplash_pictures ORDER BY id DESC LIMIT 1`.
- `create` (`src/app/picture.rs:30-38`): `INSERT ... VALUES (?,?,?) RETURNING url, photographer, photographer_url, created_at`.
- `AppState` lives in `src/app/state.rs:14-24` (fields `templates`, `db`, `env`, `metrics`, `http`, `unsplash_base_url`).
- Templates reach the `.wallpaper` div via minijinja layout/block inheritance:
  - `templates/home.html:1` `{% extends "layout.html" %}`; overrides `{% block title %}` (`home.html:2`), `{% block heading %}` (`home.html:4`), `{% block content %}` (`home.html:7`).
  - `templates/layout.html:6` `<title>{% block title %}`; `:11-14` nav chrome; `:15` `<main class="page">`→`.container`(`:16`)→`<h1>{% block heading %}`(`:17`)+`{% block content %}`(`:18`).
  - The `.wallpaper` div at `layout.html:10` consumes `wallpaper_url` directly: `<div class="wallpaper" aria-hidden="true" {% if wallpaper_url %}style="background-image: url('{{ wallpaper_url }}')"{% endif %}></div>`. Because it sits on the layout (not overridden per-page), every extending page inherits it.
- `templates::init` (`src/app/templates.rs:4-19`): `path_loader("templates")`, HTML auto-escape callback, and `asset_url` registered as a template function. Called in state construction (`src/test/mod.rs:97`) and presumably `main.rs`/`src/app/db.rs` init.
- Metrics: `inc_page_view("home")` is emitted at `src/interfaces/handlers/home/web.rs:13`; test observes it at `src/test/mod.rs:119` (`page_views_total` label `page="home"`).

### Patterns
- `.ok().map(...)` swallows upstream errors so decoration never fails a page render — applied identically in both `home/web.rs:16` and `singlethread/web.rs:16`.
- Single-key `context!` is the site-wide idiom; both page handlers pass only `wallpaper_url`.

## Q2: The `<Picture>` type — population and distinguishing fields

### Findings
- Definition: `src/domain/picture.rs:7-13` — `struct Picture { url: String, photographer: String, photographer_url: String, created_at: String }`, `#[derive(Serialize, sqlx::FromRow)]`. Doc comment: "A picture served by the `/unsplash` endpoint, persisted in the `unsplash_pictures` table."
- Beyond `url`, it carries `photographer` (name) and `photographer_url` (Unsplash profile link) — these are the "credit" data the business concepts rely on; `created_at` is a string timestamp populated by the DB, not the upstream.
- Fresh from API: `src/infra/unsplash.rs:31-37` builds `Picture` from a `RandomPhotoResponse`:
  - `url` ← `body.urls.regular`
  - `photographer` ← `body.user.name`
  - `photographer_url` ← `body.user.links.html`
  - `created_at: String::new()` — "populated by the DB on insert" (comment `unsplash.rs:36`; `create` fills it via `RETURNING`, `source/app/picture.rs:30-38`).
- Upstream parse structs: `RandomPhotoResponse` (`unsplash.rs:12-15`), URLs `{ regular }` (`:17-19`), User `{ name, links{ html } }` (`:21-27`). `name`/`links` make the parse strict (`serde`).
- Where the fields are empty:
  - **Legacy rows**: INSERTs predating `photographer_url` leave it as `''` (column added in `0005_add_photographer_url.sql` as `DEFAULT ''`). The JSON test `fresh_row_does_not_call_upstream` seeds via `INSERT INTO unsplash_pictures (url, photographer) ...` and asserts `photographer_url:""` (`src/interfaces/handlers/unsplash/json.rs:53-77`, assert at `:75`).
  - **Test stub / harness**: `seed_wallpaper` (`src/test/mod.rs:135-141`) inserts only `(url, photographer)` → `photographer_url` remains `''`. Home test wallpaper row is `https://example.com/wallpaper.jpg` (`src/test/mod.rs:135-141`).
  - `created_at`: empty in the unit test fixture `insert_picture_returns_row_with_created_at` (`src/app/picture.rs:55`) and in the infra parser; DB fills it.
- Empty-fields semantics: `photographer`/`photographer_url` are `NOT NULL` but may be `''` (never NULL) for legacy/seeded rows. `is_stale()` treats an *unparseable* `created_at` as stale (forces refresh): `src/domain/picture.rs:21-25`.

### Patterns
- The DB is a cache: API data is written through `create` and read back via `latest`; `created_at` is sourced from the DB `datetime('now')` default (`migrations/0003_unsplash_pictures.sql:16`).

## Q3: `layout.html` handling optional/missing template data

### Findings
- `.wallpaper` conditional — `templates/layout.html:10`:
  `<div class="wallpaper" aria-hidden="true" {% if wallpaper_url %}style="background-image: url('{{ wallpaper_url }}')"{% endif %}></div>`
  When `wallpaper_url` is absent (`None`, silenced error via `.ok()`), the `{% if %}` fails and the div renders with only `class` + `aria-hidden` — an empty, screen-reader-hidden, fixed `<div>` whose only visible trait is the CSS `background-color: var(--color-bg)` fallback (`.wallpaper` rule). The `<div>` always renders; only the inline `style=` appears conditionally.
- `aria-hidden="true"` (`layout.html:10`) marks the wallpaper decorative for AT even when it carries an inline background.
- Conditional-rendering idiom across templates is `{% if x %}...{% endif %}`; the only per-page context value is `wallpaper_url`, injected by each handler via `context!` (see Q1).
- Link-with-`target`/`rel` idiom: `templates/home.html:23` (GitHub) and `:31` (LinkedIn):
  `<a href="..." target="_blank" rel="noopener noreferrer" class="flex items-center gap-2 py-2"> … <img … alt="" …> … </a>`. The paired `alt=""` marks the decorative brand images. Internal nav links are plain `<a href="/">Home</a>` / `<a href="/singlethread">SingleThread</a>` (`layout.html:12-13`).

### Patterns
- `asset_url('...')` is the way to reference every static file from templates (registered in `src/app/templates.rs`).
- Missing data degrades gracefully: absent `wallpaper_url` → empty decorative div, never a broken page.

## Q4: Tailwind CSS v4 build pipeline + `asset_url`

### Findings
- Source: `css/site.css` is the CSS-first input, never served directly (header comment `css/site.css:1-6`). It does **not** import Preflight; only `theme` + `utilities` layers are imported plus an own `base` layer. `@source`/`@layer`/`@theme` directives:
  - `@source not "../.pi"` and `@source not "../static"` (`css/site.css:7,8`) — excludes `.pi/ planning notes` and generated `static/site.css` from content scanning so the utility set doesn't depend on local/build files (comment `css/site.css:1-6`).
  - `@layer theme, base, components, utilities;` (`css/site.css:9`).
  - `@import "tailwindcss/theme.css"` / `@import "tailwindcss/utilities.css"` with `layer(theme)`/`layer(utilities)` (`css/site.css:10-11`).
  - `@theme {}` tokens (`css/site.css:14-21`): `--color-bg #1a1a1a`, `--color-surface`, `--color-text`, `--color-muted`, `--color-accent`, `--color-accent-strong`, `--color-border`; `--radius-lg` referenced by `.container`.
  - Custom hand-written base rules live in `@layer base { ... }` (`css/site.css:27-93`): `box-sizing`(`:28`), `body`(`:31-39`), `a`(`:41-44`), `a:visited`(`:46-48`), `a:hover`(`:50-52`), `a:focus-visible`(`:54-57`), `.wallpaper`(`:59-67`), `.page`(`:68-70`), `.container`(`:72-81`), `nav`(`:83-88`), `nav a`(`:90-92`).
- Build: `scripts/build-css.sh` — pins Tailwind standalone CLI `v4.3.3` with per-platform SHA-256 checksums, caches the binary under `target/tailwindcss-cli` (`build-css.sh:3-25`), then `"$bin" -i css/site.css -o static/site.css --minify` (`build-css.sh:43`).
- Output: `static/site.css` is committed. It is referenced from templates via `asset_url('site.css')` (`layout.html:7`) → resolved by `asset_url` in `src/app/assets.rs`.
- `asset_url`/versioning (`src/app/assets.rs:26-33`): hashes every file under `static/` once (`ASSET_HASHES` `OnceLock`, `assets.rs:12`), returns `/static/<file>?v=<12-hex sha256 prefix>`; panics on unknown files. Hashing is recursive (`hash_dir`, `assets.rs:18-24`). The version string for `site.css` invalidates browser cache when the compiled file changes (so a rebuild changes the hash → new `?v=`).
- How new rules enter `static/site.css`: rerun `scripts/build-css.sh`; Tailwind scans templates for utility classes (content detection), picks up any new utility class used in `home.html`/`singlethread.html`/`layout.html`; hand-written base rules must be added inside `@layer base` in `css/site.css`. `static/site.css` is regenerated (`--minify`), and `asset_url` recomputes its hash on next template render (hashes are lazy-on-first-use).
- Covered by test `asset_url_function_resolves_in_templates` (`src/app/templates.rs:55`). `scripts/test.sh` formats/refresh sqlx offline metadata/lints/tests (AGENTS.md).

## Q5: Styling & accessibility conventions in shared chrome

### `css/site.css` global rules
- Global links: `a { color: var(--color-accent); text-decoration: none; }` (`css/site.css:41-44`); `a:visited { color: var(--color-accent); }` (`:46-48`); `a:hover { color: var(--color-accent-strong); }` (`:50-52`); `a:focus-visible { outline: 2px solid var(--color-accent-strong); outline-offset: 2px; }` (`:54-57`).
- WCAG 1.4.1 comment (`css/site.css:22-26`): "All links are accent-coloured with no underline — the site has no inline body-text links today, but if one is added it MUST carry a non-color differentiator (underline, weight change, or icon) to satisfy WCAG 1.4.1." Links distinguish themselves from body text solely by color today; no non-color differentiator is present on the live `<a>` elements (`home.html:23,31`, nav `layout.html:12-13`) — the comment flags this as a required change if inline links are added.
- `.wallpaper` (`css/site.css:59-67`): `position: fixed; inset: 0; z-index: -1; background-color: var(--color-bg); background-size: cover; background-position: center;`.
- `.page` (`:68-70`): `min-height: 100vh;`.
- `.container` (`:72-81`): `max-width: 48rem; margin: 3rem auto; padding: 3rem 2rem; background: var(--color-bg); border: 1px solid var(--color-border); border-radius: var(--radius-lg);` — opaque panel keeps text readable over wallpaper.
- `nav` (`css/site.css:83-88`): `flex`, `gap: 1.5rem`, `background: var(--color-surface)`, bottom border; `nav a { padding: 0.5rem 0; }` (`:90-92`).

### Findings — decorative vs interactive `role`/`aria`
- Decorative images use `alt=""` (e.g. GitHub icon `home.html:25`, LinkedIn icon `home.html:33`; SingleThread screenshots). Informative images carry real `alt` (e.g. portrait `home.html:41`).
- The wallpaper is explicitly `aria-hidden="true"` (`layout.html:10`).
- No explicit `role=` attributes observed in the templates; page semantics come from `nav`/`main`/`h1` structure (`layout.html:11-20`).
- The invited links use `target="_blank"` + `rel="noopener noreferrer"` (`home.html:23,31`).

## Q6: Testing the home HTML and the `/unsplash` JSON

### Findings — harness (`src/test/mod.rs`)
- `start_app()` (`src/test/mod.rs:17-20`) → `start_app_with("https://api.unsplash.com")`.
- `start_app_with(base)` (`:22-25`) → `serve_app` (`:52`), binds a random port, spawns the app with in-memory SQLite (`sqlite::memory:`), applies `./migrations`, calls `seed_wallpaper()`, builds full `AppState`.
- `serve_app` (`:54-84`) builds `Env` (test key/dsn, `enable_sentry:false`, rate-limit `per_ms`/`burst`), `db::init`, `migrate!`, `seed_wallpaper`, routes wrapped in `rate_limit::with_global_limit`, serves via `axum::serve` with `ConnectInfo`.
- **`seed_wallpaper`** (`src/test/mod.rs:135-141`) inserts a fresh cached row `('https://example.com/wallpaper.jpg', 'Wallpaper Photographer')` so page renders never hit the network.
- `start_unsplash_stub(status)` (`src/test/mod.rs:153-196`) spawns a local `GET /photos/random` returning canned JSON (`urls.regular=https://images.example.com/photo.jpg`, `user.name=Stub Photographer`, `user.links.html=https://unsplash.com/@stub`) for success statuses, verbatim status otherwise; exposes `base_url` + `call_count` (`AtomicUsize`).
- `start_app_with_rate_limits(...)` (`:25-31`) for 429 tests.
- `test_client()` (`:129-131`) → `reqwest::Client::new()`.

### Findings — home tests (`src/interfaces/handlers/home/web.rs` `mod tests`)
- `index_serves_ok_html`: GET `/`→200, `content-type` contains `text/html`; asserts title `"<title>Home</title>"`, body snippets (`Hi!`, name, bio, `high-output individual contributor`, `You are invited to`); link hrefs to GitHub/LinkedIn; versioned assets (`/static/wave.svg?v=`, `.jpg?v=`, github/linkedin `?v=`); absence of legacy classes `home-columns`/`invite-list`; nav `<a href="/">Home</a>` + `<a href="/singlethread">SingleThread</a>`; and that `site.css` is linked with `/static/site.css?v=` and no inline `<style>`.
- `index_renders_wallpaper_from_cache` (`home/web.rs:65-70`): GET `/` → asserts `body.contains("url('https:&#x2f;&#x2f;example.com&#x2f;wallpaper.jpg')")` — minijinja escapes `/` in attribute context; browsers decode it back.
- `index_still_renders_when_wallpaper_fetch_fails` (`home/web.rs:80-93`): stub 500 (`start_unsplash_stub(INTERNAL_SERVER_ERROR)`) → `start_app_with(&stub.base_url)`, `DELETE FROM unsplash_pictures`, GET `/` still 200, and `!body.contains("background-image")`.
- These tests assert both status **and** body tokens (per project convention).

### Findings — `/unsplash` JSON tests (`src/interfaces/handlers/unsplash/json.rs` `mod tests`)
- Local `clear_pictures(db)` (`:15-20`) wipes `unsplash_pictures` since processing seeded a fresh render row.
- `no_row_triggers_fetch_and_insert`: 200; body contains photo URL + `Stub Photographer` + `https://unsplash.com/@stub`; DB count==1; `stub.call_count==1`.
- `fresh_row_does_not_call_upstream` (`json.rs:53-77`): seed legacy INSERT (url, photographer only) → body has `https://example.com/fresh.jpg`, `Fresh Photographer`, `"photographer_url":""` (`:75`); call_count==0; count==1.
- `stale_row_triggers_refetch`: seed created_at `datetime('now','-7 hours')` → refetches, body contains stub photo+photographer (`@stub`), call_count==1, count==2.
- `upstream_failure_is_502`: stub 500 → status 502, body `"bad gateway"` (WebError External mapping).
- `malformed_upstream_json_missing_user_links_is_502`: custom stub missing `user.links` → 502 `"bad gateway"`, call_count==1.
- `second_request_within_window_is_cached`: two GETs → identical bodies, call_count==1, count==1.
- `unsplash_tier_trips_while_global_budget_stays_open`: 20 concurrent GETs → mix of 200 and 429; 429s carry `retry-after` header and body `"too many requests"`, `ok>=1`, `limited>=5`, stub sees <20 calls (rate-limited).

## Cross-Cutting Observations
- **Decoration vs. content**: wallpaper is treated as pure decoration — `.ok().map(|p| p.url)` swallows all errors in both handlers, `aria-hidden` on the div, and failure-path test asserts no `background-image`. Only the `url` is surfaced to templates, even though `Picture` carries `photographer`/`photographer_url`.
- **Single source of truth**: the Unsplash fetch lives in `infra`; `app/picture.rs` re-exports it and owns DB cache; `interfaces` imports only `app` (modular-sanity rule, mirrored at `interfaces/routes.rs`).
- **DB cache + staleness**: `MAX_AGE_HOURS=6` in `src/domain/picture.rs:4`; `created_at` from DB; strictly unparseable → stale → refetch.
- **Optional template data** handled uniformly with `{% if %}` and thin `context!` from a single handler.
- **Testing conventions**: integration tests boot the real router via `start_app` families / in-memory SQLite + a fake-network stub; assert on both HTTP status and rendered body tokens / JSON fields; the "environ pickle" is irrelevant except the harness locally overrides rate limit relative to global.

## Open Areas
- No handler currently passes `photographer`/`photographer_url` into any template context — they exist in `Picture` and the `/unsplash` JSON but are unused by page rendering (Q2 remains open as to where credit UI would live).
- The committed `static/site.css` is binary-built; whether the artifact on `fly.toml`/Dockerfile re-runs `build-css.sh` at deploy was not traced (Dockerfile not analyzed).
- The custom `@layer base` rules (e.g. `.wallpaper`) are hand-written; exact purpose of the two `@source not` directives beyond determinism is stated in the CSS header comment only.
- Emptiness of the `photographer_url` in legacy rows is a known data state (migration default `''`), surfaced only by tests/likely soon by whatever feature consumes it.