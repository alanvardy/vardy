# Research Findings

All line numbers verified by direct read. Template engine is minijinja; site is server-rendered axum with zero client-side JS.

## Q1: How are the content sections on the SingleThread page composed?

### Findings
- `templates/singlethread.html` opens with `{% extends "layout.html" %}` and overrides three blocks: `title` (line 2), `heading` (line 3), and `content` (line 4 through end). It does NOT override `nav`. `src/interfaces/handlers/singlethread/web.rs:13-15` renders it via `state.templates.get_template("singlethread.html")?.render(context!{...})`.
- Page is one vertical stack of static sections inside the `content` block (`templates/singlethread.html:4-90`):
  - **Hero block** — `<div class="hero">` at `templates/singlethread.html:5-24`, a two-column asymmetrical flex `flex flex-col md:flex-row gap-8 items-center` (line 7): left text `space-y-4 md:flex-1` holds `<p class="heading-hero">` tagline (line 9) and `<p class="text-muted">` subtitle (line 10); right icon `<img class="rounded-2xl">` in `order-first md:order-none md:flex-none` (lines 11-16).
  - **Platform badges** — `flex flex-wrap gap-2 justify-center mt-6` (line 18) with four `<span class="badge">` (iPhone/iPad/Mac/Watch, lines 19-22).
  - **Divider** — `<div class="divider"></div>` (line 26).
  - **`heading-section` block** — `<h2 class="heading-section">` (line 28) + one `<p>` (lines 29-30).
  - **Screenshot grid** — `div.flex flex-wrap gap-6` (line 31) of three `<figure class="card m-0 flex-1 basis-[10rem] max-w-[14rem] p-3">` (lines 32, 37, 42), each an `<img class="w-full rounded-lg border border-neutral-700">` + `<figcaption class="text-muted text-sm mt-2 text-center">`.
  - **`heading-subsection` sections** (each `<h2 class="heading-subsection">` + body): "On your wrist" (lines 48-58, two `div.card max-w-[12rem] p-3`), "Why it helps" (lines 60-66, `<ul class="list-disc pl-6 marker:text-accent space-y-2">`), "Everything you need…" (lines 68-78, one `<p>` + 6-item `<ul>`), "Thoughtful by design" (lines 79-84, 3-item `<ul>`), "Built for quiet productivity" (lines 85-88, two `<p>`).
  - **Closing CTA** — `<p class="text-2xl text-accent text-center mt-12">` (line 89).
- Custom component classes are defined in `@layer components` of `css/site.css` (block starts line 108): `.card` (line 109), `.card:hover` (line 116), `.badge` (line 120), `.divider` (line 130), `.heading-hero` (line 150), `.heading-section` (line 155), `.heading-subsection` (line 162). Also shared `.btn` (line 137), `.form-input`/`.form-label` used by contact.
- `text-muted`/`text-accent` are Tailwind theme utilities derived from `@theme` vars (e.g. `--color-accent: #fb923c` css/site.css:27), not hand-written rules.
- `.hero` class (singlethread.html:5) has **no** CSS rule — bare grouping container.
- Tailwind utilities supply layout/spacing around custom classes: responsive `flex-col md:flex-row`, `gap-*`, arbitrary widths `basis-[10rem] max-w-[14rem]`, `max-w-[12rem]`, list styling `list-disc pl-6 marker:text-accent space-y-2`. Custom classes govern color/surfaces/typography.
- The `@layer components` block is global/shared — a single compiled `static/site.css` served via layout.html link; home uses Tailwind utilities only, contact uses the `.btn`/`.form-input`/`.form-label` components.

## Q2: Existing pattern for collapsible / expandable content?

### Findings
- **None exists.** Repo-wide search found `<details>`/`<summary>`, accordion, disclosure, tabs, modals, `onclick`, `addEventListener`, `<script>` — no matches anywhere (templates/, css/, static/, src/).
- **No client-side JavaScript at all.** No `.js` files; `static/` holds only images + compiled `site.css`. `templates/layout.html` head loads only a stylesheet (`asset_url('site.css')` line 8) — no `<script>`.
- All `fetch`/`json`/`script` matches in `src/` are server-side (sqlx/SQL fetches, reqwest calls to Unsplash/Resend upstream).
- Interactivity model is purely server-rendered minijinja served by axum handlers; e.g. `src/interfaces/handlers/singlethread/web.rs:1-20`. The only dynamic surface on the whole site is the contact form (`POST /contact`, honeypot) and wallpaper fetch — no JS behavior.
- Static content on SingleThread is laid out linearly and always fully expanded: hero → badges → divider → section heading + p → card grid → five subsections → closing CTA (see Q1). There is no reveal/expand mechanism anywhere on the site.

## Q3: How templates/singlethread.html inherits from layout.html and the context contract

### Findings
- `templates/layout.html` defines blocks `title` (line 7), `nav` (lines 23-27), `heading` (line 30), `content` (line 32), plus shared chrome: stylesheet `<link>` (line 8), wallpaper div (lines 11-12), Unsplash photo credit (lines 13-22), nav (lines 23-27), and wrapping `<main class="page"><div class="container">` (lines 28-34).
- Context contract is documented in a template comment `templates/layout.html:1-4`: every extending template must supply `wallpaper_url`, `photographer`, `photographer_url`; missing values are treated as empty (wallpaper hidden, credit suppressed).
- All three pages supply these via `picture::wallpaper_context(&state).await` plus `active_page`:
  - singlethread — `src/interfaces/handlers/singlethread/web.rs:13-15`, `active_page => "singlethread"`.
  - home — `src/interfaces/handlers/home/web.rs:10-13`, `active_page => "home"`.
  - contact — `src/interfaces/handlers/contact/web.rs:20-25`, `active_page => "contact"` (+ extra `submitted` flag).
- `active_page` drives the nav active highlight in layout.html:24-26 (`{% if active_page == "singlethread" %} class="active"{% endif %}`).
- `asset_url` is a registered minijinja function: `src/app/templates.rs:12-16` calls `assets::asset_url`, which returns `/static/<file>?v=<12-hex sha256 prefix>` (`src/app/assets.rs:44-50`, hashes from `hash_all("static")` lines 8-40); unknown files panic (`assets.rs:46-49`). Used in layout.html:9 and each `<img>` in singlethread.html (lines 12, 35, 40, 45, 53, 57).
- Wallpaper + credit chrome degrade gracefully: `wallpaper_context` returns `("", "", "")` on any failure (`src/app/picture.rs:15-21`); layout guards suppress the background-image (layout.html:11) and the credit line (layout.html:13).

## Q3: (this is the structure test question) — see Q4

### Findings
- Q3 in the input asked about tests *and* the test.sh gate; addressed in full below under Q4.

## Q4: How are the rendered SingleThread page contents asserted, and how test.sh verifies the page?

### Findings
- Test module at `src/interfaces/handlers/singlethread/web.rs:19`; helpers imported from `crate::test` (lines 21-24).
- `index_serves_ok_html` (web.rs:28-68): asserts `200 OK` (line 33), `text/html` content-type (35-38), `<title>SingleThread</title>` (41), `<h1>SingleThread</h1>` (42), hero tagline (43), first bullet `"One at a time."` (44), each section heading (45-48) and closing CTA (49), every asset URL via `body.contains(r#"<img src="/static/..."><v=")"` (50-55), nav Home + active SingleThread links (56-57), absence of legacy classes checked at boundaries `"st-"`, ` st-`, `section-heading`, `home-columns` (58-62), wallpaper URL escaping `url('https:&#x2f;&#x2f;example.com...')` (63), and photo credit with linked name (65-70).
- `index_still_renders_when_wallpaper_fetch_fails` (web.rs:71-85): 500s from Unsplash stub, clears `unsplash_pictures`, asserts page still 200 and has no `background-image` / `Photo by`.
- `index_shows_credit_as_text_when_no_photographer_url` (web.rs:87-97): seeds row with no URL, asserts plain-text credit and no link wrap.
- Test helpers in `src/test/mod.rs`: `start_app` (24-26), `start_app_with` (29-31), underlying `serve_app` (59-103, in-memory SQLite via `env DATABASE_URL="sqlite::memory:"`, runs `sqlx::migrate!`, `seed_wallpaper`, builds `AppState`, wraps `routes()` in rate-limiter, spawns axum), `test_client` (148-150), `seed_wallpaper` (151-158), `seed_wallpaper_no_url` (160-165), `start_unsplash_stub` (struct 170-173, builder ~178-198).
- Static asset versioned URLs are sanity-checked by `asset_url` unit tests (`src/app/assets.rs:44-52`) and routes.rs serves them with immutable `Cache-Control: public, max-age=31536000` (`src/interfaces/routes.rs`, `singlethread_screenshots_are_served_with_immutable_caching` test).
- `scripts/test.sh` gate (all chained with `&&`):
  - `cargo fmt --all` (line 6)
  - `cargo sqlx prepare -- --tests` (line 9) — refreshes offline `.sqlx/` SQL metadata
  - `cargo check --all-targets` (line 12)
  - CSS build + drift check: `./scripts/build-css.sh && git diff --exit-code -- static/site.css` (lines 15-16)
  - `cargo clippy --all-targets --all-features --locked -- -D warnings` (line 19)
  - `cargo nextest run` (line 22)
  - forgotten-TODO grep `! rg ... 'FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO\s|todo\s' src` (line 25)
- `scripts/build-css.sh` compiles `css/site.css` → `static/site.css` via pinned Tailwind v4.3.3 standalone (version line 6, cmd line 38 `"$bin" -i css/site.css -o static/site.css --minify`).

## Q5: How is `/singlethread` documented in ROUTES.md; pattern for template/CSS sync?

### Findings
- `ROUTES.md` `/singlethread` block spans `ROUTES.md:22-37`: `### GET /singlethread` (line 22); prose describing the hero icon/badge/tagline, platform badges, gradient divider, screenshot & watch cards with hover transitions, feature lists, closing CTA, and wallpaper/credit (lines 24-30); then `Response: 200 OK — text/html (minijinja templates/singlethread.html)` (32), `Errors: 500 via WebError` (33), rate-limit 429 with Retry-After/X-RateLimit-* (34-35); closing `---` (37).
- Convention: each endpoint is one self-contained `###` … `---` block (AGENTS.md:61-62). Verbatim: "each endpoint section (`###` through closing `---`) is a self-contained block — use `---` as the cut point when making batch edits".
- `ROUTES.md` does **not** document the CSS/Tailwind build — the only "css" hit is ROUTES.md:42 (contact honeypot). The CSS-sync convention is documented in `AGENTS.md:46-48` and enforced by `scripts/test.sh:15-16` (`build-css.sh` + `git diff --exit-code -- static/site.css`).
- Routes are defined centrally in `src/interfaces/routes.rs` (`fn routes()` line 15 aggregates; `/singlethread` bound at line 49; `/static` ServeDir with immutable caching in same file). `AGENTS.md:59-60` mandates that route/param changes be reflected in `ROUTES.md`.
- Documented sync pattern = two channels: behavior/templates tracked in `ROUTES.md` manual spec (per `###` block); stylesheet consistency enforced mechanically by build+drift gate in the test pipeline. Actions on CSS-drift failure: run the Tailwind build (`AGENTS.md:48`).

## Cross-Cutting Observations
- **No JS / no interactive widgets anywhere.** The site is entirely server-rendered minijinja + static CSS. Any collapsible/FAQ feature would be the first client- or template-level interaction on the site — there is no existing toggle pattern to reuse.
- **One shared stylesheet for all pages.** `css/site.css` `@layer components` is global; edits affect every page. New component classes must be added here and compiled via build-css.sh.
- **Shared 3-var chrome contract.** Every page handler calls `picture::wallpaper_context(&state)` + `active_page`; the 3 wallpaper/credit variables are legacy via `picture.rs` and degrade safely (empty strings). Any new page must repeat this contract.
- **Tests assert behavior + structure by substring** in `singlethread/web.rs`, including a "no legacy class names" boundary check — a new FAQ section's headings/bullets will likely be surfaced through the same `body.contains(...)` assertions.
- **Two documentation channels.** `ROUTES.md` (behavior spec, hand-maintained, per-endpoint `###` block) vs. `scripts/build-css.sh`/`scripts/test.sh` (CSS generation + drift gate, script-enforced).
- **ACS-driven CSS versioning**. `asset_url` content-hashes static files; new assets must exist under `static/` or `asset_url` panics.

## Open Areas
- `~/AGENTS.md` referenced by AGENTS.md error policy could not be verified on this host (file not present under `.pi`-home; the error-chokepoint rule noted separately).
- No evidence pinpoints how an *interactive* component would be tested (no prior art); the existing model is server-rendered substring assertions.
- The `.hero` class has no CSS rule — behavior depends on unstyled grouping only.