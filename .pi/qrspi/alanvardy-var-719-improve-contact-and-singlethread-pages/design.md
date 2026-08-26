# Design Discussion

## Current State

The site is a three-page personal/portfolio site with a dark aesthetic. All
pages share `templates/layout.html` chrome: a simple horizontal nav bar, a
full-bleed Unsplash wallpaper behind a centered opaque container panel, and
an unsplash photographer credit bubble. No preflight, no JavaScript — pure
server-rendered HTML served from an axum app.

**SingleThread page** (`templates/singlethread.html`): a hero blurbs (one-line
tagline + two-sentence explanation) with a screenshot image in the same flex
row (`home.html:8-44` pattern reused at `singlethread.html:5-15`), followed by
a series of `<h2 class="text-muted text-xl mt-8 mb-3">` section headings
paired with paragraphs, a three-figure screenshot row (`singlethread.html:20-33`),
a centered two-watch-image row (`singlethread.html:36-41`), three
`list-disc marker:text-accent` feature lists, and a closing accent CTA line
(`singlethread.html:75`). The orphaned `singlethread-icon.png` in `static/`
is tested but never rendered (`research.md §Open Areas`).

**Contact page** (`templates/contact.html`): a bare single-column form
(label+input pairs in `flex flex-col gap-4`, submit button, CSS-hidden
honeypot) with a `{% if submitted %}` branch that swaps the form for a bland
thank-you line. No static assets referenced. No introductory content.

**Styling** (`css/site.css`): Tailwind v4 CSS-first with a dark `@theme` palette
(bg `#1a1a1a`, surface `#262626`, text `#ece7e2`, muted `#a8a29e`, accent
`#fb923c`, accent-strong `#fdba74`, border `#3d3833`), hand-written `@layer base`
rules for body, links, `.wallpaper`, `.page`, `.container` (max-width 48rem
opaque panel), and nav. No component classes, no gradient utilities, no
hero-heading scale beyond `text-xl`.

**Handler & testing pattern**: Each page handler fetches wallpaper context →
renders template with context → returns `Html<String>`. Tests are TCP
integration tests using `start_app()` (in-memory SQLite) + `reqwest::Client`,
asserting `.contains()` on `res.text()` for nav chrome strings, heading text,
image URLs, and wallpaper credit lines. Contact has additional form-post,
honeypot, 502, and 429 tests.

## Desired End State

A notably more visually appealing site where each page has a distinct
personality while sharing a cohesive upgraded chrome. Verify correctness by:

1. **All existing tests pass** with minimal assertion updates (nav chrome
   strings are the main cross-page change; page-specific content assertions
   evolve but remain `.contains()` substring checks)
2. **`./scripts/test.sh` gate passes** — format, sqlx, type-check, clippy,
   tests, CSS drift check
3. **Visual inspection**: SingleThread is richer (icon hero, card layouts,
   badge accents); Contact is inviting (two-column, intro copy, polished form)
4. **No regressions**: home page, `/dump`, `/health`, wallpaper/credit, and
   email delivery are untouched or only trivially adapted to chrome changes

## Patterns to Follow

### Good patterns (must match)

| Pattern | Reference | Notes |
|---|---|---|
| Asymmetrical two-column hero | `home.html:8-44`, `singlethread.html:5-15` | `flex flex-col md:flex-row`, image column with `max-w-[X] order-first md:order-none` |
| Section h2 heading | `singlethread.html:17,35,43,50,61,68` | `class="text-muted text-xl mt-8 mb-3"` — evolve to larger hero scale per Q5 |
| Accent marker bullets | `singlethread.html:44,52,62` | `list-disc pl-6 marker:text-accent space-y-2` |
| Accent left-border link list | `home.html:21` | `list-none ml-0 py-0 pl-4 border-l-4 border-accent` |
| Image treatment | `home.html:42`, `singlethread.html:13` | `rounded-lg border border-neutral-700` — keep for screenshots, not icons |
| Heading inline icon | `home.html:3-6` | `{% block heading %}` with inline `<img>` — reuse for SingleThread heading |
| Single render path + submitted flag | `contact/web.rs:19-25` | shared `render(state, submitted)` helper — keep |
| `wallpaper_context().unwrap_or_default()` | `picture.rs:15-21` | decorative fallback, never fails the page |
| `asset_url` for all static refs | `layout.html:12`, all page templates | content-hashed immutable cache, panics on missing file |
| `inc_page_view` per page GET | `singlethread/web.rs:10`, `contact/web.rs:28` | keep label strings unchanged |

### Patterns to avoid

- **Inline `<style>` tags**: All CSS goes through `css/site.css` → Tailwind
  compile → committed `static/site.css`. The home page test asserts
  `!body.contains("<style>")` (`home/web.rs:56`). Do not reintroduce.
- **Legacy component class names**: Tests assert no `st-`, `section-heading`
  (`singlethread/web.rs:56-58`) or `home-columns`/`invite-list`
  (`home/web.rs:53-54`). New component classes follow the `.card`, `.badge`,
  `.hero` naming per Q5, distinct from legacy names.
- **New handler code**: Route registration, wallpaper fetching, form
  processing, and error handling are all working correctly. Template and CSS
  changes only.
- **Template-only `<script>` tags**: No JavaScript. Keep it zero-JS.

## Design Decisions

1. **Ambition level — Bold redesign**: Chose Option C across all five
   questions. Gradient accents on key elements, stat/metric-like badges in
   the SingleThread page, decorative dividers between sections, subtle
   CSS transitions on interactive elements (links, buttons), and a
   distinct visual identity per page while sharing an upgraded chrome.

2. **`singlethread-icon.png` as hero app-icon badge**: Place it prominently
   in the SingleThread hero section — either as a large standalone element
   beside the tagline or as an app-icon badge next to the heading. This
   resolves the orphaned-asset gap (tested but unused) and gives the page
   immediate visual identity before any text is read. Option B (hero
   placement) rather than inline `<h1>` icon (too small) or deletion
   (scope creep).

3. **Rework shared chrome**: Upgrade `layout.html` nav bar (wider, subtle
   active-page indicator, possibly slightly transparent/blurred), add a
   `.container-wide` variant for pages that benefit from a broader layout,
   and soften the wallpaper treatment (optional subtle gradient overlay).
   This touches every page's nav string assertions but is a one-time
   coordinated update across all three handler test files. Option C rather
   than page-only or light changes because the current chrome is the
   limiting ceiling on visual quality.

4. **Two-column contact form**: Split into form (right/top) + introductory
   copy (left/bottom). The intro side explains who Alan is and why someone
   should reach out — a warm lead-in before the form fields. Uses the
   existing asymmetrical flex pattern from `home.html:8-44`. The
   `{% if submitted %}` branch becomes a two-column thank-you (intro copy
   stays, form replaced by confirmation). Option B (two-column) rather than
   single-column polish or over-engineered multi-step.

5. **Mini design system**: New component classes in `css/site.css` under
   `@layer components`:
   - `.hero` — larger hero section with optional gradient background, wider
     max-width, larger heading scale (text-3xl+)
   - `.card` — reusable card container with surface background, border,
     rounded corners, optional hover transition
   - `.badge` — small inline accent-colored pill for metric/stat display
     (e.g., "iPhone, iPad, Mac, Watch" platform badges)
   - Gradient utilities — `bg-gradient-to-r from-accent to-accent-strong`
     using theme tokens for button and divider accents
   - `.divider` — horizontal decorative divider between major sections
   - `.container-wide` — wider container variant (max-width: 64rem or so)
     for the SingleThread page's screenshot grids
   - Transition utilities — `transition-colors duration-200` on links and
     buttons for subtle polish
   - A new heading scale: `text-3xl` hero tagline, `text-2xl` major section
     heads, `text-xl` minor section heads (current h2 is the ceiling)

6. **No handler/route/behavior changes**: All changes are in templates and
   CSS. Route registrations (`routes.rs:41-42`), handler logic
   (`singlethread/web.rs:9-18`, `contact/web.rs:19-47`), form processing,
   honeypot, Resend integration, rate limiting, and wallpaper fetching are
   untouched. The `inc_page_view` label strings stay `"singlethread"` and
   `"contact"`.

7. **Test strategy — update in place**: Every `.contains()` assertion that
   references changed markup gets updated. Nav chrome strings are a
   cross-cutting change across all three handler test files. Content
   assertions (heading text, hero taglines, image URLs) stay or shift as
   the markup moves. No new test framework or assertion style — keep the
   TCP integration + .contains() pattern.

## What We're NOT Doing

- **Not touching handler code, route registration, or AppState** — purely
  template + CSS work
- **Not adding JavaScript** — zero-JS remains a site property
- **Not changing Tailwind version, build process, or `asset_url` hashing**
- **Not touching `/dump`, `/health`, `/unsplash` routes or templates**
- **Not removing `singlethread-icon.png` or its test assertions** — we're
  finally using it
- **Not changing wallpaper/photographer credit mechanism** — stays as is
- **Not adding new static assets** beyond what already exists in `static/`
- **Not changing the contact form's behavior** — honeypot, Resend delivery,
  rate limiting, 429/502 error paths all stay identical
- **Not redesigning the home page** — it gets chrome updates only
- **Not adding a new font or webfont import** — system font stack stays
- **Not introducing media queries beyond Tailwind's built-in breakpoints**

## Open Risks

1. **Test assertion churn**: Nav chrome changes cascade to every page's test
   file. The `contain "<a href="..."` assertions in
   `home/web.rs:50-52`, `singlethread/web.rs:52-53`, and
   `contact/web.rs:83-85` must all be updated in lockstep. Mitigation:
   update all three in one editing pass before running tests.

2. **`singlethread-icon.png` aspect ratio**: It's a 1024×1024 app icon (from
   `file` inspection). Using it as a hero badge needs explicit `width`/`height`
   to prevent layout thrash — the research shows the home page pattern of
   explicit dimensions on the `wave.svg` (`home.html:4`).

3. **Gradient + dark theme contrast**: `from-accent to-accent-strong` is
   orange-on-orange — fine for buttons/accents but could clash with the
   wallpaper. Keep gradients on solid-surface elements only, never on the
   wallpaper overlay.

4. **`.container-wide` vs wallpaper parallax**: The wider container still
   sits on the opaque `.container` background. At 64rem on mobile-first,
   it's fine. At extreme viewports the panel may feel floaty — the current
   48rem cap already has this and it's accepted.

5. **`ROUTES.md` sync**: No route or parameter changes, but the markup
   description blurbs in ROUTES.md are now stale. Update the `GET /contact`
   and `GET /singlethread` sections to reflect the new page structure.