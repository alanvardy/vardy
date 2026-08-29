# Research Findings

Repo: alanvardy.com (Rust/axum + minijinja + Tailwind v4 CSS-first). Four templates
(`templates/{home,contact,singlethread,layout}.html`), one CSS source
(`css/site.css`, 235 lines), one compiled artifact (`static/site.css`, minified 1
line, v4.3.3). No JS anywhere in the repo.

## Q1: Page width constraints — `.container`, `@layer base`, Tailwind v4 `.container` utility

### Facts

- Layout chrome: `<meta name="viewport" content="width=device-width, initial-scale=1">`
  at `templates/layout.html:9`; `<main class="page"><div class="container">` at
  `templates/layout.html:29-30`. This is the only use of the `container` class in
  `templates/` (verified by grep; zero in `src/`).
- The `.container` element matches **two** competing rules, one from each layer:
  - Hand-authored, in `@layer base` at `css/site.css:81-89`:
    `max-width: 48rem; margin: 3rem auto; padding: 3rem 2rem;` plus opaque panel
    background, 1px border, `border-radius: var(--radius-lg)` (border-radius token is
    Tailwind's default: `static/site.css`, `@layer theme` `--radius-lg:.5rem`).
  - Tailwind v4 core utility emitted in `@layer utilities` of `static/site.css`:
    `.container{width:100%}` plus stepped responsive max-widths
    `@media (min-width:40rem){max-width:40rem}`, `48rem`, `64rem`, `80rem`, `96rem` (all in
    the minified line, `static/site.css:2`).
- Layer ordering: source declares `@layer theme, base, components, utilities;`
  (`css/site.css:10`) and imports only `tailwindcss/theme.css` and
  `tailwindcss/utilities.css` (`css/site.css:11-12`); Preflight is intentionally NOT
  imported (header comment `css/site.css:3-4`). Compiled order is
  properties → theme → base → components → utilities, so **utilities outrank base** on
  equal specificity (`static/site.css:2`).
- `box-sizing: border-box` on `*,::before,::after` (`css/site.css:31`) — widths include
  padding. `body{margin:0}` (`css/site.css:35`), `.page{min-height:100vh}` with no width
  rule (`css/site.css:77-79`), so the container's containing block is the viewport.
- **No rule sets font-size on `html`/`:root`** (verified in both source and compiled
  output) — rem resolves against the 16px UA default; Tailwind's `--spacing:.25rem` is
  just a custom property. No `--breakpoint-*` overrides in `@theme` (`css/site.css:14-23`),
  so Tailwind defaults apply (sm=40rem, md=48rem, lg=64rem, xl=80rem, 2xl=96rem).
- `@source not "../.pi"` / `@source not "../static"` (`css/site.css:8-9`) keep the utility
  scan deterministic.

### Effective widths per viewport (border-box, 16px rem)

| Viewport | Matching rule | Panel width | Content (minus 4rem padding) |
|---|---|---|---|
| < 40rem (< 640px) | none (base `max-width:48rem` not binding) | 100% of viewport, no gutters | viewport − 2rem sides |
| 40–48rem (640–767px) | `min-width:40rem` → max-width 40rem | 40rem (640px) — caps below the base rule's 48rem | 36rem (576px) |
| 48–64rem (768–1023px) | `min-width:48rem` → max-width 48rem | 48rem (768px) | 44rem (704px) |
| 64–80rem | cap 64rem | 64rem (1024px) | 60rem (960px) |
| 80–96rem | cap 80rem | 80rem (1280px) | 76rem (1216px) |
| ≥ 96rem | cap 96rem | 96rem (1536px) | 92rem (1472px) |

Horizontal auto margins (`margin:3rem auto`, `css/site.css:83`) center the panel whenever
the viewport exceeds the cap. Padding (`3rem` top/bottom, `2rem` sides) is constant at
every viewport — no utility rule touches it. The utility layer's stepped caps replace the
base rule's `max-width:48rem`; at 40–48rem that means a **narrower** panel than the base
rule alone would produce.

## Q2: Contact page two-column layout

### Facts

- Wrapper: `<div class="flex flex-col md:flex-row gap-8 items-start">` at
  `templates/contact.html:5`. Left column `<div class="space-y-4 md:flex-1">` at
  `templates/contact.html:7`; right column `<div class="md:flex-1">` at
  `templates/contact.html:18` (form or thank-you branch, `{% if submitted %}`).
- Submit button at `templates/contact.html:39` with classes spanning `:40-45`:
  `btn self-start rounded-full inline-flex items-center justify-center gap-2
  cursor-pointer transition-all ... focus-visible:outline-accent-strong`.
- Component classes in `@layer components` of `css/site.css`:
  - `.btn` `css/site.css:137-144` — `border-radius: var(--radius-DEFAULT)`, accent
    background, weight 600, `padding: 0.625rem 1.25rem`; `.btn:hover` `:146-148`. **No width
    property** — content/padding sized.
  - `.form-input` `css/site.css:169-177` — `width: 100%`, radius, 1px border, surface
    background, `padding: 0.75rem`; focus state `:179-183`.
  - `.form-label` `css/site.css:185-189` — `color: var(--color-muted); font-size:
    0.875rem; font-weight: 500` (no width).
- Compiled flex utilities (`static/site.css:2`): `.flex{display:flex}`,
  `.flex-col{flex-direction:column}`, `.self-start{align-self:flex-start}`,
  `.items-start{align-items:flex-start}`, `.space-y-4>:not(:last-child)` margins,
  `.gap-8{gap:calc(var(--spacing) * 8)}` (2rem), and the only `md:` variant group
  `@media (min-width:48rem){.md\:order-none{order:0}.md\:flex-1{flex:1}.md\:flex-\[3\]{flex:3}.md\:flex-none{flex:none}.md\:flex-row{flex-direction:row}}`
  – so `md` = 48rem = **768px** at the 16px default root.

### Column width per breakpoint

- **< 768px** (wrapper-direction column): only `md:flex-1` is present on the right column
  (no base `flex-1`), and wrapper-level `items-start` prevents cross-axis stretching, so
  the form column is **fit-content, not full width**. Empirically measured (headless
  Chrome, compiled CSS inlined, device-scale 1) it stays ≈ **202px** at every sub-768
  viewport — sized by the form's intrinsic max-content; `.form-input{width:100%}`
  resolves within the form's own content box. The intro column (unconstrained prose) fills
  the wrapper. The button stays ≈ 137px everywhere (`self-start` + intrinsic + padding).
- **≥ 768px** (row + `md:flex-1` on both columns): each column =
  (container content width − 2rem gap) / 2, since `flex:1` means flex-basis 0% with equal
  growth; container content width = container border-box − 2px border − 4rem padding.
  Measured: ≈ 327px at 768vw, 335px at 820vw (48rem cap reached), 455px at 1024vw, 463px
  at 1100–1279vw (64rem cap), 583px at 1280vw. (Q1's arithmetic and Q2's measurements
  differ only by the ~15px scrollbar consumed in the empirical run.)

### Test assertions on layout

`src/interfaces/handlers/contact/web.rs` tests (`#[cfg(test)] mod tests` starts `:55`):
- `get_contact_returns_200_with_form` (`:64-88`) asserts HTTP 200, `text/html`
  content-type, `<title>Contact</title>`, `name="name"/email/message/_website"`,
  `action="/contact"`, intro copy `"I'm Alan"`, and exact nav anchors including
  `<a href="/contact" class="active">Contact</a>` (`:88`).
- **There are no assertions on any CSS class, flex utility, or layout attribute** —
  `form-input`, `form-label`, `btn`, `flex-col`, `md:flex-1`, `self-start` appear nowhere
  in the test module. Layout tests are content/status only (POST success `:101-104`,
  honeypot `:122-123`, resend 502 `:136-137`, 429 rate limit `:157-158`).

## Q3: Wallpaper background and photographer credit

### Markup in `templates/layout.html`

- `.wallpaper` div always emitted, inline style conditional:
  `<div class="wallpaper" aria-hidden="true" {% if wallpaper_url %}style="background-image: url('{{ wallpaper_url }}')"{% endif %}>`
  at `templates/layout.html:14`. Empty `wallpaper_url` → no `style` attribute at all.
- Credit bubble guarded by `{% if photographer %}` (`:15`, closed `:21`):
  `<div class="fixed bottom-3 right-3 px-3 py-1.5 rounded bg-black/50 text-sm">` (`:18`)
  containing `Photo by {% if photographer_url %}<a href="..." target="_blank"
  rel="noopener noreferrer" class="underline">{{ photographer }}</a>{% else %}{{
  photographer }}{% endif %} on Unsplash` (`:19`). Linked name when
  `photographer_url` is set; plain text otherwise ("legacy rows", comment `:16-17`).
- Context contract documented at `templates/layout.html:1-4`: every extending page must
  supply `wallpaper_url`, `photographer`, `photographer_url`; missing values are treated
  as empty.

### Supply chain

- `picture::wallpaper_context` `src/app/picture.rs:15-21`: `(String, String, String)` =
  `(url, photographer, photographer_url)` from `current(state).await.ok()
  .map(...).unwrap_or_default()` — **any failure yields `("","","")`**, suppressing both
  wallpaper style and credit bubble (doc comment `picture.rs:10-14`).
- `current` `picture.rs:25-31`: returns `latest()` (`:33-40`, `SELECT url, photographer,
  photographer_url, created_at FROM unsplash_pictures ORDER BY id DESC LIMIT 1`) if
  not stale (`Picture::is_stale`, `src/domain/picture.rs:17`, `MAX_AGE_HOURS = 6` at
  `:4`), else `fetch_and_insert` (`picture.rs:60-64`) → `fetch_random` +
  `create` (`:42-51`).
- Handler call sites, all with the same "decorative fallback" comment (`home/web.rs:10-12`,
  `contact/web.rs:18-20`, `singlethread/web.rs:62-64`):
  - home: `src/interfaces/handlers/home/web.rs:13` (call), `:15` (`context!` with
    `active_page => "home"`).
  - contact: shared render helper `src/interfaces/handlers/contact/web.rs:17-27`, call at
    `:21`, context at `:25`.
  - singlethread: `src/interfaces/handlers/singlethread/web.rs:65` (call), `:74` (context
    with `faq_items`).

### CSS

`.wallpaper` at `css/site.css:60-67` (`position: fixed; inset: 0; z-index: -1;
background-color: var(--color-bg); background-size: cover; background-position: center`),
plus gradient overlay `.wallpaper::after` `:69-75`. Compiled identically in
`static/site.css` `@layer base`. Credit-bubble utilities (`fixed`, `bottom-3`, `right-3`,
`px-3`, `py-1\.5`, `rounded`, `bg-black/50`, `text-sm`, `underline`) all emit in
`static/site.css` `@layer utilities`.

### Test assertions on that markup

- home `src/interfaces/handlers/home/web.rs`:
  - `:64-68` credit with linked name — `"Photo by"`, `"Wallpaper Photographer"`,
    `href="https:&#x2f;&#x2f;unsplash.com&#x2f;@test"` (minijinja HTML-escapes `/` in
    attribute context), `"on Unsplash"`.
  - `:82-83` inline style — `body.contains("url('https:&#x2f;&#x2f;example.com&#x2f;wallpaper.jpg')")`.
  - `:102-104` no-URL variant — `"Photo by NoLink Photographer on Unsplash"` and
    `!body.contains("NoLink Photographer</a>")` (no anchor).
  - `:123-124` fetch failure — `!body.contains("background-image")` and
    `!body.contains("Photo by")`.
- singlethread `src/interfaces/handlers/singlethread/web.rs`: same three scenarios at
  `:132-139` (wallpaper + linked credit), `:156-159` (suppressed), `:178-180` (plain-text).
- contact `src/interfaces/handlers/contact/web.rs`: the render path calls
  `wallpaper_context` (`:21`) but **no contact test asserts wallpaper or credit markup at
  all**.
- Test data: every default test app seeds one cache row — `serve_app` calls
  `seed_wallpaper(&db)` at `src/test/mod.rs:76`; `seed_wallpaper` at `:167-175` inserts
  `https://example.com/wallpaper.jpg` / `Wallpaper Photographer` /
  `https://unsplash.com/@test`; `seed_wallpaper_no_url` at `:179-187` omits
  `photographer_url` (column DEFAULTS `''`).

| Condition | `.wallpaper` div | inline `style` | credit bubble |
|---|---|---|---|
| url + photographer + url all set | present | present (`layout.html:14`) | present, linked name (`:18-19`) |
| photographer set, `photographer_url` empty | present | present | present, plain text |
| all empty (fetch failure/empty cache) | present | **suppressed** | **suppressed** (`:15`) |

## Q4: Responsive/mobile conventions and `@media` audit

### Template conventions (all verified against source)

- **Mobile-first `flex flex-col md:flex-row` stacking**: `templates/home.html:8`,
  `templates/contact.html:5`, `templates/singlethread.html:7` (`items-center` here vs
  `items-start` on the other two). Non-breakpoint `flex-col`: form and field groups
  `contact.html:22,23,27,31`.
- **`order-first md:order-none`** (image above text on mobile): `templates/home.html:40`
  (portrait), `templates/singlethread.html:12` (app icon). Compiled
  `.order-first{order:-9999}` / `.md\:order-none{order:0}` (`static/site.css:2`).
- **`md:`-only column allocation**: `md:flex-[3]` `home.html:9` (intro = 3× the portrait
  column), `md:flex-1` `home.html:40`, `contact.html:7,18`, `singlethread.html:8`,
  `md:flex-none` `singlethread.html:12`.
- **Arbitrary-value utilities** (only `max-w-[...]`, `basis-[...]`, `flex-[...]`; no
  `w-[...]`): `max-w-[200px]` `home.html:40`; `md:flex-[3]` `home.html:9`; `basis-[10rem]
  max-w-[14rem]` `singlethread.html:33,38,43`; `max-w-[12rem]` `singlethread.html:52,56`.
  Compiled: `.max-w-\[200px\]`, `.basis-\[10rem\]`, `.md\:flex-\[3\]` etc.
  (`static/site.css:2`).
- **Wrap-around rows**: `flex flex-wrap gap-2 justify-center` badges
  `singlethread.html:19`; `flex flex-wrap gap-6` screenshot grid `singlethread.html:32`
  (cards are `flex-1 basis-[10rem] max-w-[14rem]` so they wrap below ~10rem each); watch
  row `flex justify-center gap-6` `singlethread.html:51` (fixed 2-up).
- **Full-width images inside columns**: `w-full rounded-lg border border-neutral-700` —
  `home.html:41`; `singlethread.html:34,39,44,53,57`.
- **`sm:`/`lg:`/`xl:`/`2xl:` — zero occurrences** in `templates/` or `css/` (grep, exit 1).
  The only breakpoint in use is `md` (48rem).
- `layout.html` chrome (nav `:23-27`, credit bubble `:18`, container `:30`) has **no
  breakpoint variants**; nav is `display:flex; gap:1.5rem` (`css/site.css:91-96`) with no
  wrap/mobile styling.

### `@media` audit

- Hand-written `@media` in source CSS, templates, JS: **none**. Repo-wide grep excluding
  `.pi/` and compiled output returns zero hits; there are no `.js` files at all.
- All 7 `@media` blocks in the repo are in the minified `static/site.css:2`, generated by
  the Tailwind v4.3.3 CLI: five `min-width` container steps (40/48/64/80/96rem), the
  `min-width:48rem` `md:` variant group, and `@media (hover:hover)` for
  `.hover\:-translate-y-0\.5` / `.hover\:shadow-md`.
- Prior design docs describe the *pre-VAR-682* state: the old hand-written
  `@media (max-width: 48rem)` was removed when Tailwind v4 was adopted
  (`.pi/qrspi/alanvardy-var-682-adopt-tailwind-css-v4-via-standalone-cli-replace-hand-rolled/research.md:62`,
  `structure.md:97,101`, `plan.md:380,459`).

### Prior design-doc decisions (VAR-719 and VAR-726)

- `VAR-719 design.md` (`.pi/qrspi/alanvardy-var-719-improve-contact-and-singlethread-pages/design.md`):
  - `:61` codifies the canonical pattern: "Asymmetrical two-column hero —
    `flex flex-col md:flex-row`, image column with `max-w-[X] order-first md:order-none`".
  - `:166` explicit boundary: **"Not introducing media queries beyond Tailwind's built-in
    breakpoints"**.
  - `:111-114` Design Decision 4: two-column contact form reuses the asymmetrical pattern.
  - `:104,130` planned `.container-wide` variant (max-width 64rem "or so") — note: no such
    class exists in the current `css/site.css` (audited; only `.container` is defined).
  - `:187` open risk: 64rem container on mobile-first is "fine".
- `VAR-726 design.md` (`.pi/qrspi/alanvardy-var-726-add-an-faq-to-singlethread-page/design.md`):
  zero responsive/breakpoint/media-query mentions (grep empty). Its constraints table
  (`.pi/qrspi/alanvardy-var-726-add-an-faq-to-singlethread-page/design.md:64-66`) records:
  component classes must live in `@layer components` of `css/site.css` ("one stylesheet,
  one component layer; no inline `<style>`"), tests assert structure via `body.contains(...)`
  substring checks, assets go through `asset_url`, legacy class-name boundary checks, and
  "CSS build + drift gate enforces stylesheet consistency". It also bans JS and CSS-only
  hacks (hidden checkboxes, `:target`) in favor of native `<details>` (`design.md:77-78,
  120-122`).

## Q5: CSS build pipeline, drift gate, static serving

### Pipeline

- Source `css/site.css` → compiled `static/site.css` via `scripts/build-css.sh`
  (40 lines): pins `TAILWIND_VERSION="v4.3.3"` (`:6`) with hard-coded SHA-256 checksums
  for macos-arm64/linux-x64 (`:7-8`), platform-specific download (`:10-21`), caches the
  binary under `target/tailwindcss-cli` (`:23-25`, gitignored via `.gitignore:1`),
  re-downloads only when missing or checksum-mismatched (`:27-35`), post-verifies
  (`:37`), then `"$bin" -i css/site.css -o static/site.css --minify` (`:38`).
- CI: `.github/workflows/ci.yml` `css-drift` job `:128-146` — caches
  `target/tailwindcss-cli` keyed on `hashFiles('scripts/build-css.sh')` (`:138-141`),
  `./scripts/build-css.sh` (`:143-144`), then `git diff --exit-code -- static/site.css`
  (`:145-146`). Deployment (`fly-deploy.yml`) triggers on CI workflow_run success, so the
  drift gate gates deploys. Other workflows (ci-secure, rust-version-bump,
  dependabot_auto_merge) have no CSS steps.
- Dockerfile rebuilds CSS inside the image (scripts/ is excluded by `.dockerignore:14`):
  `ARG TAILWIND_VERSION=v4.3.3` `Dockerfile:9`, arch-specific checksums `:11-12`,
  `case "$TARGETARCH"` curl + `sha256sum -c` `:28-36`, compile `tailwindcss -i
  css/site.css -o static/site.css --minify` `:37`, `COPY --from=builder /app/static`
  `:44`.
- Local gate `scripts/test.sh`: `./scripts/build-css.sh &&` (`:12`) then
  `git diff --exit-code -- static/site.css` (`:13`), chained between `cargo check` and
  `cargo clippy`; documented at project `AGENTS.md:46-47`. Any change to generated CSS
  (new utility in a template, new `@theme` token, new layer rule) fails CI/test.sh unless
  the regenerated `static/site.css` is committed; conversely the committed file cannot be
  hand-edited without matching regeneration.

### Serving, caching, hashing

- `/static` mounted at `src/interfaces/routes.rs:56-63`:
  `SetResponseHeader::overriding(ServeDir::new("static"), CACHE_CONTROL,
  "public, max-age=31536000, immutable")` — one-year immutable cache on every static file.
  Endpoint block documented at `ROUTES.md:108-116`.
- `asset_url` `src/app/assets.rs:37-42`: SHA-256 of file bytes truncated to 12 hex chars
  (`hash_all` `:12-16`, `hash_dir` `:18-35`), keyed by path relative to `static/`,
  producing `/static/<file>?v=<hash>`; **panics on unknown files** (`None` arm `:41`) and
  on unreadable dirs ("fail fast on broken deploys", comment `:10-11`). Hashes are lazy in
  a `OnceLock` (`:8`) — content-dependent, so a rebuilt `static/site.css` gets a new `?v=`
  automatically.
- Registered as a minijinja function in `templates::init()` `src/app/templates.rs:13-16`
  (`Value::from_safe_string`), which also sets `path_loader("templates")` (`:5`) and HTML
  auto-escape for `.html` (`:6-10`). Only stylesheet reference:
  `<link rel="stylesheet" href="{{ asset_url('site.css') }}">` at
  `templates/layout.html:11`; all other assets also go through `asset_url`
  (`home.html:4,25,33,41`; `singlethread.html:13,34,39,44,53,57`).

### Constraints on CSS/markup changes (from tests + gate)

- Drift gate: any new utility class used in a template only appears in `static/site.css`
  after `build-css.sh` runs — regenerated output must be committed (`test.sh:12-13`,
  `ci.yml:145-146`). Utility detection is template-driven (`@source` exclusions at
  `css/site.css:8-9`); hand-written rules must go in `@layer base` (`:29`) or
  `@layer components` (`:108`).
- HTML-substring assertions pin template structure (all `body.contains(...)`):
  - home `web.rs:43-63`: `<title>Home</title>`, `Hi!`, intro prose, GitHub/LinkedIn hrefs,
    every asset `src="/static/<file>?v=`, **negative** checks `home-columns`/`invite-list`
    absent (`:57-58`), exact nav anchors (`:60-61`), `/static/site.css?v=` present (`:62`),
    and **no `<style>` tag** (`:63`).
  - singlethread `web.rs:112-131`: hero tagline, section headings, FAQ `<details>`/
    `<summary><span class="faq-chevron"` and `</span>{question}</summary>` (`:114-115`),
    versioned image srcs (`:118-123`), exact nav (`:125`), negative class checks
    `"st-`, ` st-`, `section-heading`, `home-columns` at class-name boundaries (`:126-131`),
    no `<script`/`onclick` (`:265-266`), FAQ-before-CTA ordering (`:250`).
  - contact `web.rs:64-88`: title, field names, action, nav anchor with `class="active"`.
- Serving contract: `/static/site.css` must keep `Cache-Control: max-age=31536000`
  (`routes.rs:61`) or `static_stylesheet_is_served` (`routes.rs:265-283`, also checks
  `text/css`) fails; per-asset cache tests at `routes.rs:131-144` and `:215-244`.
- `asset_url` tests: `known_file_yields_versioned_url` (`assets.rs:50-56`, 12 hex chars),
  `unknown_file_panics` (`assets.rs:72-78`), deterministic hashing (`assets.rs:59-66`),
  `asset_url_function_resolves_in_templates` (`templates.rs:51-61`).
- Net effect: adding a new utility class requires (1) use in a template, (2) regenerated
  committed `static/site.css`, (3) no collision with negative assertions (legacy class
  names) or exact-substring assertions.

## Cross-Cutting Observations

- One source of truth for width control: `.container` is styled twice (base panel style +
  Tailwind utility caps) and the utility caps **shrink** the panel below the base rule's
  48rem in the 40–48rem band — the effective layout is a stepped panel at Tailwind's
  default min-width breakpoints, sitting on a full-viewport fixed wallpaper
  (`layout.html:14`, `css/site.css:60-67`).
- The entire responsive strategy is five utilities at one breakpoint (`md`, 48rem/768px):
  `flex-col`/`md:flex-row`, `order-first`/`md:order-none`, `md:flex-[N]`/`md:flex-1`/
  `md:flex-none`, plus `flex-wrap`. No hand-written media queries anywhere; all `@media`
  blocks are Tailwind-generated.
- Mobile behavior contrasts with desktop: on the contact page the form column is
  fit-content (~202px) below 768px rather than full-width — the only responsive
  full-width-vs-column behavior comes from the wrapper direction flip, not from width
  utilities on the columns.
- Decorative fallback pattern is uniform: `wallpaper_context`'s empty-default tuple and
  the template's `{% if %}` guards pair so Unsplash failure degrades to a plain
  background with no credit bubble, and all three handlers carry the identical
  "decorative fallbacks" comment.
- Every test gate is content-string based — no DOM parsing, no layout assertions, no
  computed-style checks; the closest things to layout pins are the nav-anchor exact
  strings, the negative legacy-class checks, and `/static/site.css?v=` presence.
- `asset_url` + immutable cache header are a matched pair: 12-hex content hashes make the
  1-year `max-age=31536000, immutable` safe; changing any static file changes its `?v=`
  automatically.

## Open Areas

- `.container-wide` appears in the VAR-719 design doc (`design.md:104,130`) as a planned
  variant but does not exist in `css/site.css` or any template as of this research —
  whether it was consciously dropped or deferred is not recorded in the current codebase.
- Q2's empirical column-width measurements involve a ~15px vertical-scrollbar offset in
  the headless-browser environment; sub-768 form-column width (~202px) is intrinsic
  content sizing, not a written rule, and would shift if form content or font/text-size
  tokens change.
- `ROUTES.md` documents `/static` with a note about the global rate limiter
  (`ROUTES.md:114-115`); whether static files actually pass through the GCRA layer was
  not traced in this pass (the route is nested on the same router in `routes.rs:56-63`).
- Prior-doc references (`VAR-682`, `VAR-664`, `VAR-668`, `VAR-704` research/plan files)
  describe the removed hand-written `@media (max-width: 48rem)` era; current behavior is
  fully captured by the compiled `static/site.css` `@media` audit above.