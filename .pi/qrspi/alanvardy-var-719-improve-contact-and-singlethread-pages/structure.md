# Structure Outline

## Approach

Bold template+CSS redesign of `/singlethread` and `/contact` plus upgraded shared chrome — no handler, route, or behavior changes. All work flows through `css/site.css` → Tailwind compile → `static/site.css` with zero JavaScript. Four vertical slices, each crossing CSS → template → tests.

---

## Phase 1: Design System + Chrome Upgrade

Establish the visual foundation and upgrade the nav bar / wallpaper treatment shared by all pages. Every subsequent phase builds on these CSS classes and the new nav structure.

**Files**: `css/site.css`, `templates/layout.html`, `templates/home.html`, `templates/singlethread.html`, `templates/contact.html`, `src/interfaces/handlers/home/web.rs`, `src/interfaces/handlers/singlethread/web.rs`, `src/interfaces/handlers/contact/web.rs`

**Key changes**:

- *CSS component classes* (`@layer components` in `css/site.css`):
  - `.hero { /* larger max-width, optional gradient bg, bigger heading scale */ }`
  - `.card { @apply bg-surface border border-border rounded-lg; transition: border-color 200ms; &:hover { border-color: var(--color-accent); } }`
  - `.badge { @apply inline-block px-2.5 py-0.5 rounded-full text-sm font-medium bg-accent/15 text-accent-strong; }`
  - `.divider { @apply w-full h-px my-8; background: linear-gradient(to right, var(--color-accent), var(--color-accent-strong), transparent); }`
  - `.container-wide { max-width: 64rem; }` (wider than the default 48rem `.container`)
  - `.btn { @apply rounded bg-accent text-bg font-semibold px-5 py-2.5; transition: background-color 200ms; &:hover { background: var(--color-accent-strong); } }` — extracted from the contact form's submit button for reuse
  - Gradient utility: `.bg-gradient-accent { background: linear-gradient(to right, var(--color-accent), var(--color-accent-strong)); }` — explicit CSS so Tailwind doesn't tree-shake it
  - Heading scale: `.heading-hero { @apply text-accent; font-size: var(--text-3xl); }`, `.heading-section { @apply text-muted mt-10 mb-4; font-size: var(--text-2xl); }`, `.heading-subsection { @apply text-muted mt-8 mb-3; font-size: var(--text-xl); }` — the old bare `text-xl` h2 becomes `.heading-subsection` or `.heading-section` depending on hierarchy
  - Transition utilities: `.transition-interactive { transition: color 200ms, background-color 200ms, border-color 200ms; }` — applied to links and buttons

- *`templates/layout.html`*:
  - Extract nav into a `{% block nav %}` with a default (no active indicator). Each page template overrides it to mark its active link with `class="active"` (a subtle border-bottom or background highlight on `nav a.active`).
  - Upgrade nav bar styling: wider padding, slightly transparent/blurred background (`backdrop-filter`), active-page indicator via `nav a.active` class.
  - Add a subtle gradient overlay on `.wallpaper` via a `::after` pseudo-element in CSS (dark gradient at bottom edge to ground the photographer credit bubble).
  - The `nav a` base styles get `.transition-interactive` for hover polish.

- *`templates/home.html`*: Override `{% block nav %}` with Home as active. Content unchanged.

- *`templates/singlethread.html`*: Override `{% block nav %}` with SingleThread as active. Content unchanged (Phase 2 rewrites it).

- *`templates/contact.html`*: Override `{% block nav %}` with Contact as active. Content unchanged (Phase 3 rewrites it).

- *Test updates — nav assertions across 3 test files*:
  - `home/web.rs:50-52`: `"<a href=\"/\">Home</a>"` → include active class marker (e.g. `"<a href=\"/\" class=\"active\">Home</a>"` or whatever the rendered output is)
  - `singlethread/web.rs:52-53`: same pattern, SingleThread link active
  - `contact/web.rs:83-85`: same pattern, Contact link active
  - Home still has NO Contact nav assertion (current behavior).

**Verify**: `./scripts/test.sh` passes (includes CSS drift check). All tests pass with updated nav assertions. Visual check: nav bar is wider, has subtle backdrop blur, active page is highlighted, wallpaper has a subtle bottom gradient overlay.

---

## Phase 2: SingleThread Page Redesign

Rebuild `singlethread.html` using the Phase 1 component classes. Icon hero with `singlethread-icon.png`, card layouts for screenshot grids, platform badges, decorative dividers, and the new heading scale.

**Files**: `templates/singlethread.html`, `src/interfaces/handlers/singlethread/web.rs`

**Key changes**:

- *`templates/singlethread.html`* — structural rewrite, sections from top to bottom:
  - **Nav block**: override `{% block nav %}` with SingleThread active (carried forward from Phase 1).
  - **Hero section** (`.hero` container): Two-column asymmetric flex — left: `singlethread-icon.png` as a large app-icon badge (`w-24 h-24` or similar, explicit dimensions per pattern at `home.html:4`), right: tagline `text-3xl` + two-sentence explanation. Uses the existing `flex flex-col md:flex-row gap-8 items-center` pattern.
  - **Platform badges row**: `.badge` pills for "iPhone", "iPad", "Mac", "Watch" — centred below the hero.
  - **First `.divider`** — decorative gradient separator before the content sections.
  - **"Why" section**: `.heading-section` heading + paragraph. Then three `<figure>` screenshots wrapped in `.card` containers in a `flex flex-wrap gap-6` grid — replaces the raw `<figure>` grid. Each card has transition hover (border lights up accent).
  - **Watch section**: `.heading-subsection` heading + centred watch images in `.card` containers.
  - **Feature lists**: `.heading-subsection` headings, `list-disc pl-6 marker:text-accent space-y-2` lists (unchanged pattern, just new heading class).
  - **Closing CTA**: accent line with `.text-2xl text-accent text-center mt-12` (slightly larger).
  - All `<img>` references preserved; add `singlethread-icon.png` as `{{ asset_url('singlethread-icon.png') }}`.

- *Test updates — `singlethread/web.rs`*:
  - `assert!(body.contains("singlethread-icon.png?v="))` — new assertion for the icon asset (finally rendered in a template).
  - Content assertions (hero tagline, section headings, image URLs, CTA text) stay or shift slightly as markup rearranges; all remain `.contains()` substring checks.
  - Nav assertions already updated in Phase 1.
  - Negative assertions: `!body.contains("\"st-")`, `!body.contains(" st-")`, `!body.contains("section-heading")` stay. Add `!body.contains("home-columns")` (belt-and-suspenders).

**Verify**: `./scripts/test.sh` passes. SingleThread page tests pass with icon assert added. Visual check: icon badge in hero, cards with hover transitions, gradient dividers, platform badges.

---

## Phase 3: Contact Page Redesign

Rebuild `contact.html` as a two-column layout with introductory copy and a polished form, using Phase 1 classes.

**Files**: `templates/contact.html`, `src/interfaces/handlers/contact/web.rs`

**Key changes**:

- *`templates/contact.html`* — structural rewrite:
  - **Nav block**: override `{% block nav %}` with Contact active (carried forward).
  - **Two-column layout** (asymmetric flex, pattern from `home.html:8-44`):
    - **Left/top column** (`md:flex-1`): introductory copy — brief bio line about who Alan is, what he works on, and why someone should reach out. Maybe re-use the "AI, backend Rust services and Swift applications" framing from the home page. Uses `.text-muted` for body text.
    - **Right/bottom column** (`md:flex-1`): the contact form — `{% if submitted %}` branches here. Form side: label+input pairs in `flex flex-col gap-4`, inputs gain the `.card`-like `bg-surface border border-border rounded p-3` (extracted from inline classes to `.form-input` CSS class). Submit button uses `.btn` from Phase 1. Honeypot unchanged (CSS-hidden, same markup).
    - **`{% if submitted %}` branch**: Two-column thank-you — intro copy stays in left column, right column shows `.text-2xl text-accent` confirmation message ("Thank you — I'll get back to you soon.").
  - *CSS additions* (`css/site.css`): `.form-input { @apply w-full rounded border border-border bg-surface text-text p-3; transition: border-color 200ms; &:focus { border-color: var(--color-accent); outline: none; } }` — extracted from the current inline field classes, adds focus ring via border-color change.
  - *CSS additions*: `.form-label { @apply text-muted text-sm font-medium; }` — label styling.

- *Test updates — `contact/web.rs`*:
  - `get_contact_returns_200_with_form` (line 64): form field name assertions (`name="name"`, `name="email"`, `name="message"`, `name="_website"`, `action="/contact"`) stay. Nav assertions already updated in Phase 1. Add assertion for intro copy text (e.g. `body.contains("Alan")` or whatever the intro line says). Title assertion (`<title>Contact</title>`) stays.
  - Submit, honeypot, 502, 429 tests (`post_*.rs`): these assert `stub.call_count`, status codes, and `"bad gateway"` / `"too many requests"` body text. No markup assertions to update — these tests don't inspect the thank-you page content beyond status.
  - Thank-you content assertion: POST success renders the two-column thank-you; the existing tests don't check the thank-you body text (they only check `StatusCode::OK` and `stub.call_count`), but we could add a `body.contains("Thank you")` check in `post_valid_form_sends_email` for coverage of the submitted branch.

**Verify**: `./scripts/test.sh` passes. Contact page tests pass. Visual check: two-column layout, intro copy present, form fields have focus ring, submit button uses `.btn`, thank-you is two-column.

---

## Phase 4: ROUTES.md Sync

Update the `/singlethread` and `/contact` route descriptions to reflect the new page structure. No route or parameter changes — markup descriptions only.

**Files**: `ROUTES.md`

**Key changes**:

- `### GET /singlethread` block (lines 13-21): update description to mention icon hero, card layout, platform badges, and decorative dividers.
- `### GET /contact` block (lines 26-34): update description to mention two-column layout with introductory copy alongside the form.
- `### POST /contact` block (lines 38-49): update description to mention the two-column thank-you page.
- Each block is a self-contained `###`…`---` region — use `---` as the cut point for batch edits per AGENTS.md.

**Verify**: `./scripts/test.sh` passes (no code changes to break). Manually review ROUTES.md for accuracy against the rendered pages.

---

## Testing Checkpoints

| After Phase | What must be true |
|---|---|
| **1** | `./scripts/test.sh` passes. All 3 page tests have updated nav assertions. CSS compiles without drift. Home, SingleThread, and Contact pages all render with the new nav chrome. |
| **2** | `./scripts/test.sh` passes. SingleThread tests assert `singlethread-icon.png` in body. Cards, badges, dividers render. Other two pages unaffected. |
| **3** | `./scripts/test.sh` passes. Contact tests have intro-copy assertion. Two-column layout renders for both form and thank-you states. Home and SingleThread unaffected. |
| **4** | ROUTES.md sections accurate. No code changes — no test gate impact. |