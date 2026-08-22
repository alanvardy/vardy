# Design Discussion

## Current State
- `/singlethread` is a minimal static page: `templates/singlethread.html` renders an icon,
  one `<p>` of copy ("single line of work"), and a 3-item `<ul>` inside a `.card`.
  Handler passes an empty context (`src/interfaces/handlers/singlethread/web.rs:12`) — all
  content is template literals.
- Shared chrome comes from `templates/layout.html`: title block, stylesheet via
  `{{ asset_url('site.css') }}` (line 8), bare nav with `/` and `/singlethread` links
  (lines 10–13), `div.container > h1 + block content`.
- Styling is one dark-theme stylesheet (`static/site.css`) with `:root` design tokens
  (lines 1–7), `.container`, `.card`, and homepage-only classes (`.home-columns` flex at
  57–62, media-query stacking at ~107–116). No grid, no screenshot layout exists anywhere.
- Static serving: `/static` nests `ServeDir` with `Cache-Control: public, max-age=31536000,
  immutable` (`src/interfaces/routes.rs:21-28`); cache busting via `asset_url()` returning
  `/static/<file>?v=<12-hex>` (`src/app/assets.rs:36-43`). `asset_url` **panics on unknown
  filenames** (`assets.rs:42`) — new assets must exist under `static/` before templates
  reference them.
- Tests are `body.contains(...)` string checks: `singlethread/web.rs:37-42` asserts title,
  `<h1>`, the phrase "single line of work", the versioned icon URL, and both nav links;
  `home/web.rs:49` also asserts the nav link to `/singlethread`.

## Desired End State
A richer product marketing page for SingleThread using the user's real copy and five
screenshots:
1. Copy assets into `static/` with clean lowercase names:
   - `singlethread-shot-main.png` (IMG_5426, 1284×2778 — hero, one-reminder screen)
   - `singlethread-shot-settings.png` (IMG_5427, 1284×2778 — settings sheet)
   - `singlethread-shot-swipe.png` (IMG_5429, 1284×2778 — light-mode Complete swipe)
   - `singlethread-watch-list.png` (410×502 — watch reminder w/ Complete/Skip)
   - `singlethread-watch-detail.png` (410×502 — watch Refresh/Delete)
   All referenced via `{{ asset_url(...) }}` with descriptive `alt` text.
2. Page structure (new `st-*` classes in `site.css`, dark-theme tokens reused):
   - Hero: h1 + promotional tagline ("Your brain does one thing at a time — your list
     should too.") beside the main screenshot; stacks on mobile like `.home-columns` does.
   - Description paragraph, then three phone screenshots in a responsive flex row
     (main / settings / swipe) with rounded corners and subtle borders (portrait style).
   - Feature prose sections from the provided copy: "Why it helps" (3 bullets),
     "Everything you need, nothing you don't" (6 bullets), "Thoughtful by design"
     (3 bullets), closing "Built for quiet productivity" paragraph + tagline line
     ("Your reminders. One at a time. In order. At your pace.").
   - Apple Watch subsection pairing the two small watch screenshots side by side.
3. Verification: updated handler test asserts new key phrases and versioned image URLs;
   existing static-serving test patterns extended for the new PNGs; `./scripts/test.sh`
   passes; visual check against the live server.

## Patterns to Follow
- **Versioned asset references** — `{{ asset_url('...') }}` (`templates/layout.html:8`,
  `singlethread.html:5`); pairs correctly with immutable caching. Do NOT follow the
  hardcoded `/static/...` style in `home.html:4,24,30,37`.
- **Design tokens** — use existing `--bg/--surface/--text/--muted/--accent` vars
  (`site.css:1-7`); no new color literals.
- **Responsive stacking precedent** — media query at `max-width: 48rem` reorders/stacks
  columns (`site.css:~107-116`); mirror this breakpoint for the hero and screenshot rows.
- **Image treatment** — radius + 1px border like `.portrait` (`site.css:64-68`).
- **Section headings** — muted-color `.section-heading` pattern (`home.html:12`,
  `site.css:70-74`); reuse or clone for feature-section titles.
- **Test harness** — boot real router via `start_app()` / assert with `test_client()`
  (`src/test/mod.rs:11-40`); status AND body asserted together (project rule).
- **Metrics unchanged** — keep `inc_page_view("singlethread")` (`web.rs:8`).

## Design Decisions
1. **Page-specific CSS classes (`st-*`)** rather than generalizing home classes — chosen
   per user (Q2=B). Product-page layouts (screenshot rows, watch pair) have different
   needs than the homepage's text/portrait split; keeps home CSS untouched. Trade-off:
   some duplication of flex/media-query logic.
2. **New copy replaces "single line of work"** (Q3=B) — handler test assertions rewritten
   around stable new phrases (tagline, "One at a time", section headings).
3. **All images via `asset_url()`** (Q4=A) — required for correct long-lived caching;
   no hardcoded `/static/` URLs on this page.
4. **Layout/nav changes permitted** (Q5=B) — e.g., adding a footer or minor heading-block
   adjustments if needed; any nav change must update both `singlethread/web.rs` and
   `home/web.rs` assertions together.
5. **Assets renamed to lowercase kebab-case** before commit (the two Downloads PNGs are
   uppercase `.PNG` with UUID names); files land in `static/` first because `asset_url`
   panics otherwise.
6. **No handler/context change** — page stays fully static; empty render context stays.

## What We're NOT Doing
- No dynamic data, DB access, or render-context values on this page.
- No App Store link/badges (none provided; can be added later).
- No refactoring `home.html`'s unversioned asset refs (separate concern).
- No new routes, metrics labels, or ROUTES.md semantics changes (content-only markup;
  research confirms ROUTES.md needs no edit — `ROUTES.md:14-19`).
- No light/dark toggle on the marketing site itself; page stays on the site's dark theme.
- No image optimization pipeline (screenshots committed as-is; file size acceptable).

## Open Risks
- Three 1284×2778 JPEGs plus two PNGs may total several MB — page weight could hurt load
  time; may need downscaled exports if it feels heavy (user decision at review).
- Screenshots contain status-bar times/battery; acceptable per user provision, but easy
  to swap later thanks to `asset_url` cache busting.
- `body.contains` assertions are brittle to copy edits — final copy wording should be
  frozen before implementation finishes.
