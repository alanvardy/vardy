# Design Discussion

## Current State

The home page (`templates/home.html`) renders a full-screen Unsplash wallpaper
via `templates/layout.html:10`. The handler at
`src/interfaces/handlers/home/web.rs:16` calls `picture::current()` but
discards everything except `.url`:

```rust
let wallpaper_url = picture::current(&state).await.ok().map(|p| p.url);
```

`Picture` (`src/domain/picture.rs:7-13`) carries `photographer` and
`photographer_url` fields — populated from the Unsplash API response
(`src/infra/unsplash.rs:33-34`) and persisted in the `unsplash_pictures` table
— but they never reach any template. The `/unsplash` JSON endpoint exposes
them (`src/interfaces/handlers/unsplash/json.rs`), but page renders do not.

The wallpaper `<div>` is marked `aria-hidden="true"` (`layout.html:10`) as
pure decoration. The site has no photo attribution visible to users.

## Desired End State

A credit line reading "Photo by [photographer name] on Unsplash" appears
in the **bottom-right corner of the viewport** as a fixed overlay, on every
page that extends `layout.html` (Home + SingleThread). The photographer name
links to their Unsplash profile page (`photographer_url`).

**Verification**:
- GET `/` and `/singlethread` return 200 with the credit line present in the
  response body (photographer name visible)
- When `photographer_url` is non-empty, the name is a link with
  `target="_blank" rel="noopener noreferrer"` pointing to that URL
- When `photographer_url` is empty (legacy rows, test seed), the name renders
  as plain text — no broken link
- When wallpaper fetch fails (`.ok()` swallows the error), no credit line
  appears (no photographer data to credit)
- The credit is marked `aria-hidden="true"` since it is decorative chrome,
  not interactive page content
- The credit link uses `underline` as its non-color differentiator (WCAG
  1.4.1), with a semi-transparent dark backdrop for contrast over variable
  photos

## Patterns to Follow

- **Handler context pattern** (`src/interfaces/handlers/home/web.rs:18-20`):
  single `context!` call per handler, all context values named inline.
  Extend to include `photographer` and `photographer_url` alongside
  `wallpaper_url`. Apply identically to both handlers (home + singlethread).

- **`{% if %}` conditional rendering** (`templates/layout.html:10`):
  gate the entire credit block on `photographer` being non-empty. Degrade
  gracefully when data is absent.

- **External link idiom** (`templates/home.html:23,31`):
  `<a href="..." target="_blank" rel="noopener noreferrer">`. Use for the
  photographer link when `photographer_url` is non-empty.

- **`aria-hidden` on decorative elements** (`templates/layout.html:10`):
  the credit overlay is decorative chrome (not interactive page content),
  so mark it `aria-hidden="true"` as the wallpaper already is.

- **Tailwind utility classes in templates**: matching the content markup in
  `home.html:8-46` (not hand-written `@layer base` rules). Tailwind v4 scans
  templates for used classes and includes them in the compiled output
  (`scripts/build-css.sh:43`). No changes to `css/site.css`.

- **`asset_url()` for static files** (`src/app/templates.rs`): any icon
  assets (e.g. a camera icon) referenced from the template must use
  `asset_url()`.

- **Test assertions on status + body** (`src/interfaces/handlers/home/web.rs`
  `mod tests`): every test asserts both HTTP status and body tokens. New
  tests for the credit line follow this convention.

- **`seed_wallpaper` test harness** (`src/test/mod.rs:135-141`): insert a row
  with `photographer_url` populated (`'https://unsplash.com/@test'`) for
  positive-path tests. Add a second seed helper or direct INSERT for
  empty-`photographer_url` tests.

### Patterns to Avoid

- Do **not** add hand-written CSS to `@layer base` in `css/site.css` — use
  Tailwind utilities in the template instead.
- Do **not** add `role` attributes to the credit — it's decorative, not a
  content landmark.
- Do **not** create a new handler or change route wiring — the credit is
  template-only, driven by context data the handler already has access to.

## Design Decisions

1. **Placement: fixed viewport overlay, bottom-right** — The credit relates
   to the wallpaper, which is `position: fixed; inset: 0`. A fixed overlay in
   the same coordinate space makes the relationship clear. Tailwind:
   `fixed bottom-0 right-0` (or `bottom-3 right-3` for padding).

2. **Scope: `layout.html` (all pages)** — The wallpaper div lives in
   `layout.html:10` and every extending page inherits it. The credit for that
   wallpaper belongs in the same shared chrome. Both handlers already pass
   `wallpaper_url` from the same `picture::current` call; extending context
   to include photographer fields is a one-line change in each.

3. **Degraded data: show name without link** — When `photographer_url` is
   `""` (legacy rows from `0005_add_photographer_url.sql`, test seed rows),
   render the photographer name as plain text. No broken `<a href="">`. The
   template uses `{% if photographer_url %}` to decide link vs. text.

4. **Link contrast: semi-transparent backdrop + underline** — `bg-black/50
   rounded` creates a dark pill behind the text, ensuring the accent-colored
   link is readable over any photo. `underline` satisfies WCAG 1.4.1 (the
   non-color differentiator the codebase currently lacks on all other links,
   as noted at `css/site.css:22-26`).

5. **CSS: Tailwind utilities only** — All styling via utility classes in
   `layout.html`. No changes to `css/site.css` or the `@layer base` block.
   Tailwind v4 content scanning picks up new classes automatically on
   `scripts/build-css.sh` rebuild.

## What We're NOT Doing

- **Not** changing the `/unsplash` JSON endpoint — it already returns
  `photographer` and `photographer_url`.
- **Not** adding credit to the `/dump` or `/health` pages — they don't extend
  `layout.html`.
- **Not** changing the `Picture` struct, the Unsplash API client, or the DB
  schema — all the data already exists.
- **Not** making the credit interactive (no hover effects beyond the global
  `a:hover` rule, no JS).
- **Not** changing the `aria-hidden` treatment of the wallpaper div.
- **Not** adding an Unsplash logo/brand mark — text-only credit as specified
  ("give credit to unsplash artist").

## Open Risks

- **Empty `photographer` field**: `photographer` is `NOT NULL DEFAULT ''`
  (`0003_unsplash_pictures.sql`). If both `photographer` and
  `photographer_url` are `""`, the `{% if photographer %}` gate hides the
  entire credit block. This is correct behavior (no data → no credit), but
  means the credit may be absent for ~6 hours if the DB cache has a
  pre-migration row with empty photographer. Risk is low — rows rotate every
  6 hours max.
- **SingleThread page**: the task mentions only the home page. If credit on
  SingleThread is undesirable, scope can be narrowed to `home.html` by moving
  the credit block out of `layout.html`. This is a one-template change and
  doesn't affect the handler or data decisions above.
- **`static/site.css` must be regenerated**: any Tailwind utility classes
  used in the template need a `scripts/build-css.sh` run to appear in the
  committed output. `asset_url` will then compute a new `?v=` hash
  automatically (`src/app/assets.rs:26-33`). No risk of stale CSS, but the
  build step must not be forgotten.