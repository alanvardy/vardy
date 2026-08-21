# Structure Outline

## Approach
Redesign the `/` homepage as template-only work modeled on the reference site:
copy real assets into `static/`, extract the inline CSS to `static/site.css`,
rewrite `templates/home.html` with greeting/bio/link-list/portrait, and update
the substring HTTP tests. No handler, route, state, or error changes — handlers
keep rendering empty contexts.

Note on slicing: this task has no database/service layers, so "vertical" means
each slice spans **assets → templates/CSS → tests** and is verifiable end-to-end
via `cargo test` + browser. Each phase leaves the app green and visually sane.

---

## Phase 1: Extract CSS to `static/site.css`

Move the inline `<style>` block out of `layout.html` into an external
stylesheet, linked from `<head>`. Zero intended visual change; establishes the
stylesheet that later phases extend.

**Files**: `templates/layout.html`, `static/site.css` (new), `src/interfaces/routes.rs`
**Key changes**:
- `static/site.css` — new file containing the existing rules (`:root`
  variables `--bg/--surface/--text/--muted/--accent`, `body`, `.container`,
  `.card`, `nav` rules)
- `layout.html`: `<style>…</style>` replaced by `<link rel="stylesheet" href="/static/site.css">`
- `routes.rs::static_icon_is_served` — add sibling test
  `static_stylesheet_is_served`: GET `/static/site.css` → 200 + content-type contains `text/css`

**Verify**: `cargo test` passes (all existing assertions unchanged and green);
manual: `cargo run`, load `/` and `/singlethread` — appearance identical to
before; hard-refresh to bypass CSS caching.

---

## Phase 2: Copy assets from reference repo

Copy the needed images from `/Users/vardy/dev/alan_vardy/priv/static/images/`
into `static/`. Pure asset drop; each becomes independently servable.

**Files**: `static/wave.svg`, `static/quill.png`, `static/github.svg`,
`static/linkedin.svg`, `static/alanvardy.jpg` (all new); optionally a
routes-level test asserting one representative asset serves 200 with an image
content-type (extend pattern from Phase 1).

**Verify**: `curl -sI localhost:3000/static/<name>` returns 200 for each;
check SVGs aren't white-on-white against the dark theme (note which need
recolored chips or sizing — feeds Phase 4).

---

## Phase 3: Rewrite homepage content

Replace placeholder content in `home.html` with the full new page: wave icon +
"Hi!" heading, two bio paragraphs, "You are invited to" list (blog →
alanvardy.com, GitHub, LinkedIn), and portrait photo. Update home tests to
assert the new content.

**Files**: `templates/home.html`, `src/interfaces/handlers/home/web.rs`
**Key changes**:
- `home.html`: replaces `.card` placeholder with sections —
  greeting (`<img src="/static/wave.svg">` + "Hi!"), two `<p>` bio blocks,
  invite list (`<a href="https://www.alanvardy.com">`, `https://github.com/alanvardy`,
  `https://www.linkedin.com/in/alanvardy/` each with icon img), portrait
  (`<img src="/static/alanvardy.jpg">`)
- Heading block: `{% block heading %}` now wraps greeting markup instead of "Welcome to vardy"
- `web.rs::index_serves_ok_html` — rewritten assertions: `<title>Home</title>`,
  a bio fragment, the three hrefs, `/static/alanvardy.jpg`, nav anchors kept

**Verify**: `cargo test` passes with new assertions; manual: browser shows all
sections with correct links/images (unstyled layout acceptable at this point).

---

## Phase 4: Style the homepage

Extend `site.css` with homepage-specific rules: flex row for text + portrait
columns, styled invite link list (border-left accent treatment like the
reference), image sizing, and one responsive media query stacking columns on
narrow screens.

**Files**: `static/site.css`, possibly `templates/home.html` (class names only)
**Key changes**:
- `.home`, `.home-columns` (flex), `.invite-list`, `.portrait` classes — new
- `@media` query for stacking below ~48rem — new
- No logic changes; tests unaffected (class names may be added to assertions
  if desired)

**Verify**: `cargo test` passes; manual at desktop + narrow widths: columns
side-by-side then stacked, icons legible on dark background, portrait rounded
and proportioned. Get Alan's sign-off on bio wording.

---

## Testing Checkpoints
- After Phase 1: `cargo test` green with **unchanged** home assertions; new
  `/static/site.css` test passes; pages look identical to before.
- After Phase 2: all five assets serve 200; SVG visibility on dark theme assessed.
- After Phase 3: home tests assert new content (hrefs, images, bio fragment);
  page fully readable though plainly laid out.
- After Phase 4: everything green + responsive layout verified manually; PR ready
  for review including bio wording sign-off.
- If resuming mid-way: any phase boundary above is a valid stopping point with
  a green build.
