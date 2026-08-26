# Research Findings

Operative question set: the `/contact` and `/singlethread` pages. Focus: request
flow, styling, static assets, composition patterns, tests, and sync concerns.

## Q1: Request-to-response flow for `/contact` and `/singlethread`

### Route registration
- Router built in `src/interfaces/routes.rs:31-61`. `GET /singlethread` →
  `handlers::singlethread::web::index` (routes.rs:41); `GET /contact` →
  `handlers::contact::web::index` (routes.rs:42).
- `POST /contact` → `handlers::contact::web::create` inside a dedicated tier:
  `contact_tier = tiered_routes(Router::new().route("/contact", post(create)), CONTACT_TIER_PER_MS, CONTACT_TIER_BURST)`
  (routes.rs:33-38), merged via `.merge(contact_tier)` (routes.rs:54).
- Both routers carry `AppState`; `main.rs:52-53` applies `trace_layer()` and
  `main.rs:54-61` wraps the main router in the global GCRA limiter
  (`with_global_limit`), then serves on `:3000`. The metrics port `:9090` uses
  `metrics_router` (routes.rs:63-66), which is NOT rate-limited.

### Handlers
- `singlethread/web.rs:9-18` `index`: `inc_page_view("singlethread")` (line 10) →
  `picture::wallpaper_context(&state)` (line 12) → render `singlethread.html`
  with `context!{ wallpaper_url, photographer, photographer_url }` → `Html<String>`.
- `contact/web.rs`:
  - Shared `render(state, submitted)` helper (19-25): fetches `wallpaper_context`,
    renders `contact.html` with `submitted: bool` context flag. One render path for
    both GET and POST.
  - `index` GET (27-30): `inc_page_view("contact")` (line 28) then
    `render(&state, false)`.
  - `create` POST (32-47): extracts `Form<ContactForm>` (line 33). Honeypot branch
    (36-37): if `_website` non-empty → `render(&state, true)` and send nothing.
    Else builds subject/text (39-42), calls `contact::send(...)` (43) → Resend,
    then `render(&state, true)` on success.
- `ContactForm` (`src/app/contact.rs:5-14`): fields `name`, `email`, `message`,
  `_website: Option<String>` (honeypot). Parsed by axum `Form` (urlencoded).
- `contact::send` (contact.rs:20-35) → `resend::send_contact_email`
  (`src/infra/resend.rs`): `POST {base}/emails`, `bearer_auth`, JSON body; non-2xx →
  `ResendError` → `WebError::External` → 502.

### Template inheritance
- `layout.html:1-4` context contract: every extending template must supply
  `wallpaper_url`, `photographer`, `photographer_url` (empty tolerated).
- Blocks in layout.html: `title` (11), `heading` + `content` inside
  `main.page > div.container` (27-32). Nav at layout.html:25-27 lists Home /
  SingleThread / Contact.
- `singlethread.html:1-3` and `contact.html:1-3` override `title`, `heading`,
  `content`. `contact.html:5/9` branches on `{% if submitted %}` (thank-you
  vs form). `singlethread.html` does not use `submitted`.

### Error mapping
- `WebError` (src/app/error.rs:8-15): `Template`, `Database`, `NotFound`,
  `External`, `TooManyRequests`.
- `IntoResponse` (error.rs:41-66): `NotFound`→404, `Database`→500
  (tracing::error! + sentry::capture_error), `Template`→500 (same), `External`→502
  (logged, no Sentry), `TooManyRequests`→429 with `retry-after` + body
  `too many requests`.
- Decorative fallback: `picture::wallpaper_context` (src/app/picture.rs:15-21)
  returns `.unwrap_or_default()` on failure, so wallpaper/credit failures never
  fail the page; common non-200 for these GETs is a template error (500).

## Q2: Styling organization

- Tailwind v4 CSS-first source `css/site.css`. `@source not` excludes `.pi` and
  `static` (lines 8-9). Layer order `@layer theme, base, components, utilities`
  (10); imports only `theme.css` and `utilities.css`, NO preflight/base import
  (10-12) — base rules are hand-written.
- `@theme` tokens (css/site.css:14-24): `--color-bg #1a1a1a`, `--color-surface
  #262626`, `--color-text #ece7e2`, `--color-muted #a8a29e`, `--color-accent
  #fb923c`, `--color-accent-strong #fdba74`, `--color-border #3d3833`. Token
  naming gives utilities `bg-bg`, `text-muted`, `text-accent`, `border-border`.
- `@layer base` hand-written rules (css/site.css:26-93): box-sizing reset (29-31),
  `body` (33-40), `a`/`a:visited`/`a:hover`/`a:focus-visible` (42-59), `.wallpaper`
  (61-68), `.page` (70-71), `.container` (73-81), `nav`/`nav a` (83-92).
- `.container` (73-81): `max-width:48rem`, bordered, rounded (var(--radius-lg)),
  opaque panel.
- Compile: `scripts/build-css.sh:34` runs pinned standalone CLI v4.3.3
  `tailwindcss -i css/site.css -o static/site.css --minify` (binary cached under
  `target/tailwindcss-cli/`, build-css.sh:6-29).
- Generated `static/site.css` is the minified committed artifact; never served
  directly (css/site.css:1-3).
- `asset_url` for CSS: `layout.html:12` `{{ asset_url('site.css') }}`. Registered
  in `src/app/templates.rs:13-17` as a safe (unescaped) template function →
  `assets::asset_url`.
- No authored media queries in source; `md:` variants used in templates
  (`home.html`, `singlethread.html`) emit Tailwind default `@media (min-width:…)`
  breakpoints in output.

## Q3: Static image assets, serving, and cache-busting

- 11 files in `static/`: `alanvardy.jpg`, `github.svg`, `linkedin.svg`,
  `singlethread-icon.png`, `singlethread-shot-main.jpg`,
  `singlethread-shot-settings.jpg`, `singlethread-shot-swipe.jpg`,
  `singlethread-watch-detail.png`, `singlethread-watch-list.png`, `site.css`,
  `wave.svg`.
- Referenced by template:
  - `layout.html`: `site.css` (12).
  - `home.html`: `wave.svg` (4), `github.svg` (25), `linkedin.svg` (33),
    `alanvardy.jpg` (41).
  - `singlethread.html`: `singlethread-shot-main.jpg` (11,22),
    `singlethread-shot-settings.jpg` (26), `singlethread-shot-swipe.jpg` (30),
    `singlethread-watch-list.png` (37), `singlethread-watch-detail.png` (39).
  - `contact.html`: **no** static asset references (pure form).
  - `static/singlethread-icon.png` is referenced by no template (only tests).
- Serving: `/static` via `nest_service("/static", SetResponseHeader::overriding(
  ServeDir::new("static"), CACHE_CONTROL, "public, max-age=31536000, immutable"))`
  (routes.rs:53-61). Immutable 1-year cache is safe because URLs are content-hashed.
- `asset_url` (`src/app/assets.rs`): `ASSET_HASHES` `OnceLock<HashMap>` lazily
  computed on first call (hash_all/hash_dir, assets.rs:15-34); each file's
  SHA-256 first 12 hex chars as `?v=` (assets.rs:37-43). Panics on unknown asset
  name — template referencing a missing file fails fast.

## Q4: Existing composition/visual patterns

- **Theme tokens** (css/site.css:14-24) drive all utilities; accent for links,
  CTA button, success banner; muted for labels, section h2, lead copy.
- **Heading with inline icon**: `home.html:3-6` `{% block heading %}` with inline
  `wave.svg`.
- **Section h2 pattern**: `class="text-muted text-xl mt-8 mb-3"` — home.html:20;
  singlethread.html:17,35,43,50,61,68. `text-xl` is the only display size; no
  large hero-heading scale.
- **Hero (asymmetric two-column)**: `flex flex-col md:flex-row gap-8` +
  image `w-full max-w-[X] order-first md:order-none` — home.html:8-44 (3:1 flex
  ratio, `md:flex-[3]` text vs `md:flex-1` image); singlethread.html:5-15.
- **Image treatment**: `class="w-full rounded-lg border border-neutral-700"`
  throughout (home.html:42; singlethread.html:13,23,27,31,38,40).
- **Card-ish grid**: `flex flex-wrap gap-6` of `<figure class="m-0 flex-1
  basis-[10rem] max-w-[14rem]">` — singlethread.html:20-33; centered watch row
  `flex justify-center gap-6` with `max-w-[12rem]` images — singlethread.html:36-41.
  Closest thing to a reusable card; no named `.card` class.
- **CTA**: closing accent line `text-xl text-accent text-center mt-12`
  (singlethread.html:75); accent left-border link list `list-none ml-0 py-0 pl-4
  border-l-4 border-accent` (home.html:21).
- **Forms** (contact.html:8-34): `flex flex-col gap-4` form; fields
  `w-full rounded border border-border bg-surface text-text p-2` (12,17,24);
  submit button `rounded bg-accent text-bg font-semibold px-4 py-2 self-start`
  (29-31); success `<p class="text-xl text-accent">` (6).
- **Bullet lists**: `list-disc pl-6 marker:text-accent space-y-2`
  (singlethread.html:44,52,62).
- **Metric badges / stat components**: NOT present anywhere (grep for
  badge|stats|metric|kpi|pill|stat- across templates and css/site.css — no
  matches). Only "pill-like" element is the photographer credit bubble
  (layout.html:18).

## Q5: Existing tests asserting `/contact` and `/singlethread`

### Test harness (`src/test/mod.rs`)
- `start_app()` (mod.rs:20) → `start_app_with("https://api.unsplash.com")` (22)
  returning `(SocketAddr, SqlitePool)`.
- `start_app_with(unsplash_base_url)` (26), `start_app_with_resend(...)` (42),
  `start_app_with_resend_and_rate_limits(...)` (48).
- Real boot `serve_app` (56-105): in-memory SQLite, `enable_sentry:false`, real
  TCP `TcpListener::bind("127.0.0.1:0")` (89-91), router wrapped in
  `with_global_limit` (96-98), `axum::serve(... with_connect_info)` (99-105).
- **Boot is TCP-only** — no oneshot boot path in src/test/mod.rs. Requests go
  over a real socket via `test_client()` = `reqwest::Client` (mod.rs:161).
- External stubs: `start_resend_stub` (mod.rs:190), `start_unsplash_stub`
  (mod.rs:231) are local TCP micro-servers.

### `/contact` tests (`src/interfaces/handlers/contact/web.rs`)
- `get_contact_returns_200_with_form` (64): GET `/contact`, assert `OK`, content
  `text/html`, body contains `<title>Contact</title>`, `name="name"`,
  `name="email"`, `name="message"`, `name="_website"`, `action="/contact"`, and
  nav chrome `<a href="/">Home</a>`, `<a href="/singlethread">SingleThread</a>`,
  `<a href="/contact">Contact</a>`.
- `post_valid_form_sends_email` (91): stub 200, POST form, assert `OK` +
  `stub.call_count==1`.
- `post_honeypot_filled_skips_email` (105): POST with `_website=http://spam`, assert
  `OK` + `call_count==0`.
- `post_resend_failure_returns_502` (124): stub 500, assert `BAD_GATEWAY` + body
  exactly `"bad gateway"`.
- `post_too_many_requests_returns_429` (138): 10 rapid POSTs against contact tier,
  expect ≥1 `TOO_MANY_REQUESTS` with `retry-after` header + body
  `"too many requests"` (accepts others as OK).

### `/singlethread` tests (`src/interfaces/handlers/singlethread/web.rs`)
- `index_serves_ok_html` (29): GET `/singlethread`, assert `OK` + `text/html`;
  body contains `<title>SingleThread</title>`, `<h1>SingleThread</h1>`, hero
  `Your brain does one thing at a time`, `One at a time.`, `Why it helps`,
  `Everything you need, nothing you don't`, `Thoughtful by design`,
  `Built for quiet productivity`, `Your reminders. One at a time. In order. At
  your pace.`, all five `<img src="/static/….jpg|png?v=…` versioned URLs, nav
  Home + SingleThread links. Negative: NOT contains `"st-`, ` st-`,
  `section-heading` (no legacy component classes). Wallpaper/credit asserts:
  `url('https:&#x2f;&#x2f;example.com…')` (escaped `/`), `Photo by`,
  `Wallpaper Photographer`, `href="https:&#x2f;&#x2f;unsplash.com&#x2f;@test"`,
  `on Unsplash`.
- `index_still_renders_when_wallpaper_fetch_fails` (76): stub 500, clear rows,
  assert `OK` and NOT contains `background-image` or `Photo by`.
- `index_shows_credit_as_text_when_no_photographer_url` (96): `seed_wallpaper_no_url`,
  assert `Photo by NoLink Photographer on Unsplash`, NOT `NoLink Photographer</a>`.
- Seeding: `seed_wallpaper` (mod.rs) inserts `https://example.com/wallpaper.jpg`,
  `Wallpaper Photographer`, `https://unsplash.com/@test`; `seed_wallpaper_no_url`
  with NULL photographer_url.
- All assertions are substring `.contains()` on `res.text()` + status/header;
  no DOM-selector/query API.

## Q6: Non-template concerns in sync with page markup

### ROUTES.md
- Flat per-route sections, each a self-contained block from a `###` heading to a
  closing `---`. AGENTS.md ("Routes" section): each `###`↔`---` block is the cut
  point for batch edits.
- `### GET /singlethread` (ROUTES.md:13-21), `### GET /contact` (26-34),
  `### POST /contact` (38-49). POST/contact adds honeypot + Resend behavior and
  the dedicated `CONTACT_TIER_*` nested tier bullets.
- Header (ROUTES.md:6-8): `:3000` global GCRA limit; `/metrics` on `:9090` is
  not rate-limited. `/metrics` is intentionally undocumented in ROUTES.md.

### Metric labels
- Family `page_views_total{page}` (IntCounterVec) — src/infra/metrics.rs:15-25;
  `inc_page_view` (metrics.rs:26-28).
- `/singlethread` label `"singlethread"` — singlethread/web.rs:10.
- `/contact` label `"contact"` — contact/web.rs:28, GET only; the POST `create`
  does NOT increment a page view.
- Metric parsing test asserts `page_views_total` labels — src/test/mod.rs:276-297.
- Prior design note (var-668 design.md) records: keep `inc_page_view("singlethread")`.

### Other tied references
- Nav strings in `templates/layout.html:25-27` render on every page; asserted by
  contact/web.rs, singlethread/web.rs, home/web.rs tests.
- Route registrations: routes.rs:41-42 (GET), 38 (POST contact).
- AGENTS.md conventions: handlers in `src/interfaces/handlers/<domain>/` (`web.rs`
  HTML, `json.rs` JSON); routes only in routes.rs; route/param changes update
  ROUTES.md.

## Cross-Cutting Observations
- **Layered I/O boundary**: `interfaces` handlers reach external HTTP only through
  `app` (contact.rs → resend.rs; picture.rs → unsplash.rs) and read state from
  `AppState` (`src/app/state.rs:8-20`).
- Single render path for GET+POST contact via a `submitted` bool; the bool drives
  the `{% if submitted %}` template branch, and only GET increments page views.
- Decorative wallpaper/credit failures are swallowed at the app layer so pages
  always render at 200; the common error is template render (500) or, on POST,
  Resend failure (502).
- All CSS lives in one committed generated artifact `static/site.css`, referenced
  only through content-hashed `asset_url` (immutable caching). Adding any new
  static file gates both the file existing in `static/` AND a template reference
  (else panic), plus a ROUTES.md/static-serve test path.
- Tests are substring-based and TCP-integration style; nav chrome and exact
  heading strings are load-bearing for `/contact` and `/singlethread`.

## Open Areas
- `static/singlethread-icon.png` is served/tested but unreferenced by any template
  (its purpose / whether markup should use it is unresolved).
- No metric-badge/stat component exists; the questions called out "metric/count
  badges" as a possible pattern but none is present in this codebase.
- ROUTES.md does not document `/metrics` (dedicated `:9090` port) — an observed
  gap, not a defect to fix per this research.
