# Design Discussion

## Current State

The SingleThread page (`templates/singlethread.html:1-90`) is a fully-static,
server-rendered minijinja template extending `templates/layout.html`. It has
zero client-side JS and no interactive widgets anywhere on the site
(research.md Q2). The handler at `src/interfaces/handlers/singlethread/web.rs:8-18`
renders it with four context variables (the wallpaper/credit trio from
`picture::wallpaper_context` + `active_page`). All content is literal strings
in the template.

The page flows: Hero (icon + tagline + platform badges, lines 5-24) → Divider
(line 26) → Section heading + prose (lines 28-30) → Screenshot grid (lines
31-46) → "On your wrist" (lines 48-58) → "Why it helps" (lines 60-66) →
"Everything you need" (lines 68-78) → "Thoughtful by design" (lines 79-84) →
"Built for quiet productivity" (lines 85-88) → Closing CTA (line 89).

The shared design system is in `css/site.css` `@layer components` (lines
108-194): `.card`, `.badge`, `.divider`, `.btn`, `.heading-hero`,
`.heading-section`, `.heading-subsection`, `.form-input`, `.form-label`, and a
nav `.active` rule. `text-muted`/`text-accent` are Tailwind theme utilities
from `@theme` vars (site.css:27). All pages share one compiled
`static/site.css`; any new component class is global.

Tests (`web.rs:20-97`) assert 200 OK, text/html, title/h1, every section
heading, the closing CTA, all asset URLs, nav links, wallpaper URL escaping,
credit rendering, graceful wallpaper-failure degradation, and no legacy class
names (lines 58-62). The gate (`scripts/test.sh`) chains fmt, sqlx prepare,
check, CSS build + drift diff, clippy, nextest, and forgotten-TODO grep
(research.md Q4).

`ROUTES.md:22-37` documents `/singlethread` in a self-contained `###` …
`---` block describing the hero, badges, divider, cards, feature lists, CTA,
and wallpaper/credit behavior.

## Desired End State

An FAQ section rendered between "Built for quiet productivity" and the closing
CTA (`templates/singlethread.html`, after line 88, before line 89). Each FAQ
item is a collapsible `<details>/<summary>` pair. The section heading reuses
`heading-section`; the Q&A content is driven by a `Vec<FaqItem>` struct
defined in the handler and passed via minijinja render context.

Verification:
- All existing tests in `web.rs` continue to pass with updated assertions
- New tests assert FAQ heading, each question text, each answer text, the
  presence of `<details>`/`<summary>` markup
- The CSS drift gate passes after adding `.faq` component classes
- `ROUTES.md` `/singlethread` block updated to mention the FAQ section
- Page renders correctly in a browser (visual check via live-testing skill)

## Patterns to Follow

| Pattern | Source | Rationale |
|---------|--------|------------|
| Handler passes context variables dict to `templates.get_template(...)?.render(context!{...})` | `web.rs:13-15` | Single rendering chokepoint; new FAQ data goes here |
| `WebError` covers template render failures with `?` propagation | `web.rs:14`, `error.rs:60-64` | No bare status codes; template render errors → 500 via WebError::Template |
| Section headings use `.heading-section` / `.heading-subsection` component classes | `singlethread.html:28,48,60,68,79,85` | Visual consistency with existing sections |
| Custom component classes live in `@layer components` of `css/site.css` | `site.css:108-194` | One stylesheet, one component layer; no inline `<style>` |
| Tests assert structure via `body.contains(...)` substring checks | `web.rs:41-57` | String assertions on rendered HTML; no DOM parsing |
| Asset URLs reference `asset_url(...)` function for cache-busting | `singlethread.html:12,35,40,45,53,57` | Any new images pass through `asset_url` |
| Legacy class-name boundary checks in tests | `web.rs:58-62` | New component class names (`.faq-*`) must not trigger false-positives |
| `ROUTES.md` endpoint blocks are `###` … `---` self-contained | `ROUTES.md:22-37` | Update the `/singlethread` block to reference the new FAQ section |
| CSS build + drift gate enforces stylesheet consistency | `scripts/test.sh:15-16`, `AGENTS.md:46-48` | Any `.faq-*` classes must be compiled into `static/site.css`; drift diff catches mismatches |

**Do NOT follow:** There is no JS runtime, no `<script>` tags, no `onclick`
handlers anywhere in the codebase (research.md Q2). Do not introduce
JavaScript. Do not use CSS-only hacks (hidden checkboxes, `:target`) — they
add complexity for no gain when `<details>/<summary>` is native and supported.

## Design Decisions

1. **Collapsible via `<details>/<summary>`** — Native HTML disclosure widget.
   Zero JS. Zero new CSS interaction states beyond styling the default
   triangle and cursor. Every major browser supports it; the only cross-browser
   concern is the `::marker` pseudo-element for the disclosure triangle, which
   we normalize with a single CSS rule. This is the idiomatic HTML FAQ pattern
   and matches the site's "no interactivity unless the browser gives it to us
   for free" philosophy.

2. **FAQ content as `Vec<FaqItem>` struct in handler** — Define in
   `web.rs`:

   ```rust
   struct FaqItem {
       question: &'static str,
       answer: &'static str,
   }
   ```

   Construct a `Vec<FaqItem>` in the handler and pass it in the render
   context. This keeps content out of the template (cleaner than 9 hardcoded
   `<details>` blocks), makes it trivial to test (iterate the vec in Rust to
   assert all questions/answers appear), and adds zero allocations (all
   `&'static str`). It is not DB-backed — a migration for static marketing
   content is overengineering given nothing else on the page is DB-driven.

3. **New `.faq` component class in `@layer components`** — Adds
   `.faq-item` for the `<details>` wrapper and minimal `<details>`+
   `<summary>` normalization. Reuses `.heading-section` for the FAQ section
   title and `.text-muted` for answer text. Keeps the CSS surface small (~8
   lines).

4. **Placement before the closing CTA** — Insert after "Built for quiet
   productivity" (line 88) and before the CTA `<p class="text-2xl
   text-accent text-center mt-12">` (line 89). This is the standard web
   convention ("Still have questions? → FAQ → CTA"), minimizes disruption to
   existing test assertions (all existing section headings still appear in
   the same order before the FAQ), and puts answers right where a visitor who
   scrolled the whole page would look.

5. **FAQ questions — 9 provided + 2 suggested**:
   1. Where is my data stored?
   2. Why did you choose Apple Reminders?
   3. Are you going to create an Android version?
   4. Are you planning on supporting other task managers?
   5. Where are the wallpapers from and how do you select them?
   6. Pulp or no pulp?
   7. What network requests does this app make?
   8. Does this app work off-line?
   9. Can I contact you with questions, bug reports, or feature requests?
   10. **Is SingleThread free?** (suggested — pricing is the #1 product FAQ)
   11. **How do I get started?** (suggested — onboarding is the #2 product FAQ)

## What We're NOT Doing

- **No JavaScript.** No `<script>`, no `addEventListener`, no htmx, no
  Alpine.js. The site has zero JS; the FAQ stays that way.
- **No database migration.** FAQ content is static and lives in the handler
  struct. No new tables, no `sqlx migrate add`.
- **No new route.** The FAQ is part of the existing `/singlethread` GET
  handler; no new endpoint.
- **No dynamic FAQ management.** No admin UI, no edit-in-place. Content
  changes happen in a PR like any other code change.
- **No CSS-only toggle hacks.** No hidden checkboxes, no `:target` tricks.
  `<details>/<summary>` is the one and only collapse mechanism.
- **No animated transitions for expand/collapse.** The browser's native
  `<details>` open/close behavior is sufficient.
- **No section reordering.** The FAQ is appended before the CTA; existing
  sections do not move.

## Open Risks

- **`<details>` styling across browsers.** Safari, Firefox, and Chrome all
  render `<details>` natively, but `::marker` and `summary::-webkit-details-marker`
  differ. Risk is low — a single `list-style: none` + custom `::marker` rule
  normalizes this. Mitigated by visual verification in Safari (primary target
  platform for an Apple-focused product page).
- **Test boundary: no legacy class-name check.** The existing
  `!body.contains("st-")` etc. checks at `web.rs:58-62` look for old
  patterns. `.faq-item` is a new class name prefix — verify it doesn't
  accidentally match a substring of `"faq-"` inside other content. Risk is
  negligible; the check only tests class-name boundaries (`"st-"` with
  leading quote/space).
- **minijinja `<details>` rendering.** minijinja has no issue with raw HTML5
  elements — the template is already full of `<div>`, `<h2>`, `<p>`,
  `<figure>`, `<figcaption>`. `<details>` and `<summary>` are standard HTML5
  elements minijinja treats as passthrough text. Zero risk.
- **FAQ content drift from product reality.** The Qs & As are hardcoded and
  will go stale if the product changes (e.g. pricing changes, Android
  version ships). Same risk as the rest of the page content. Mitigated by
  the same process: PR review catches contradictions with product changes.