# Design Discussion — VAR-664: Make homepage more attractive

## Current State

The Rust/Axum app serves a placeholder homepage:

- `templates/home.html:1-8` extends `templates/layout.html` and renders one
  `.card` containing the sentence "This is the vardy homepage, rendered with
  minijinja." No variables are used; the handler passes an empty context
  (`src/interfaces/handlers/home/web.rs:6-12`).
- `templates/layout.html` provides nav (Home / SingleThread), a `<h1>` heading
  block, and all CSS inline in one `<style>` block (`layout.html:7-51`) using
  dark-theme custom properties (`--bg/--surface/--text/--muted/--accent`,
  `layout.html:8-14`). `--muted` is defined but unused. No media queries,
  no external assets, no favicon.
- Static assets live in repo-root `static/` served by
  `ServeDir::new("static")` at `/static` (`src/interfaces/routes.rs:10`);
  exactly one file exists (`singlethread-icon.png`).
- HTTP tests assert exact substrings of rendered HTML
  (`home/web.rs:18-41`): title `"Home"`, heading `"Welcome to vardy"`, body
  sentence, and both nav anchors. They will break under any redesign.
- Reference site (`/Users/vardy/dev/alan_vardy`) homepage shows: wave icon +
  "Hi!" greeting (`index.html.heex:5-8`), two bio paragraphs (`:11-16`),
  three icon links (blog/GitHub/LinkedIn, `:18-41`), portrait photo
  `alanvardy.jpg` (`:44-46`), and a Latest Post section fed by Postex from
  markdown files (`page_controller.ex:9-13`, `blog.ex:1-4`).

## Desired End State

The `/` homepage is visually attractive and informative about Alan Vardy,
modeled on the reference site but scoped to this repo's zero-build stack:

1. Greeting ("Hi!" + wave), two bio paragraphs adapted from the reference
   (`index.html.heex:11-16`).
2. "You are invited to" link list with icons: blog (external to
   alanvardy.com), GitHub (`github.com/alanvardy`), LinkedIn.
3. Portrait photo displayed beside/below the text.
4. Styling from an external stylesheet `static/site.css` linked by
   `layout.html`; keeps the existing dark palette as the starting point.
5. All content hard-coded in templates; handler still renders empty context.
6. Updated tests pass: `cargo test` green, assertions rewritten for new
   content; existing SingleThread and static-route tests untouched.

Verify: `cargo test`, plus manual `cargo run` + browser check of `/`.

## Patterns to Follow

- **One page = route + handler dir + template extending `layout.html`**
  (`routes.rs:6-11`, `handlers/home/mod.rs:1`, `home/web.rs:6-12`). This task
  changes only the home page's template content, not its wiring.
- **Hard-coded static content in the child template** — matches both this
  repo (`home.html:5`) and the reference (`index.html.heex:8-41`). Do not
  introduce render-context data for static pages.
- **Tests inside `web.rs` under `#[cfg(test)]`, substring `contains` on
  rendered HTML** via `start_app()` + reqwest (`test/mod.rs:5-22`,
  `home/web.rs:18-41`).
- **Repo-relative asset directories** (`static/` here vs reference
  `priv/static`) referenced as absolute URLs (`/static/...`,
  `singlethread.html:5`).
- **Patterns NOT to follow**: Tailwind + esbuild build pipeline from the
  reference (`mix.exs:62,69`) — adds a build step to a zero-build repo;
  Postex/markdown blog integration (`blog.ex:1-4`) — out of scope; runtime
  dynamic content in handlers — new unneeded pattern.

## Design Decisions

1. **Content source: hard-coded in `templates/home.html`.** Matches the
   reference site's approach and the repo's existing empty-context pattern;
   simplest change surface (template-only).
2. **Latest Post section: skipped.** The repo has no blog, markdown store,
   or HTTP client; fetching at runtime would add dependencies and failure
   modes for little value. The blog link list item covers discoverability of
   alanvardy.com content.
3. **Assets: copy real images from the reference repo** into `static/`
   (portrait `alanvardy.jpg`, plus icons: wave, blog/quill or equivalent,
   github.svg, linkedin.svg). Closest visual match to the reference;
   templates reference them as `/static/<name>`.
4. **Styling: extract CSS to `static/site.css`**, linked from
   `layout.html:5`-ish head section. Cleaner than a growing inline block and
   cacheable; keep the existing dark CSS-variable palette
   (`layout.html:8-14`) as the base, extended with homepage-specific rules
   (flex layout for text+photo columns, styled link list, responsive
   stacking via one media query). This intentionally departs from the
   inline-CSS pattern — `layout.html`'s style block moves out rather than
   grows.
5. **Heading: replace "Welcome to vardy" `<h1>`** with a greeting treatment
   matching the reference ("Hi!" with wave icon); page `<title>` stays
   "Home".
6. **Tests: rewrite home assertions to match new content**, same
   `contains`-substring pattern (`home/web.rs:18-41`): assert title, a bio
   fragment, key hrefs (`https://github.com/alanvardy`,
   `https://www.linkedin.com/in/alanvardy/`, blog URL), and image srcs
   (`/static/...`). Keep nav-anchor assertions since `layout.html` nav
   markup is unchanged. Add a routes-level test asserting
   `/static/site.css` returns 200 with `text/css`, mirroring
   `static_icon_is_served` (`routes.rs:17-32`).
7. **Nav stays Home/SingleThread** — external links belong on the homepage
   body, not the app chrome (matches reference where navbar is minimal and
   homepage carries invitations).

## What We're NOT Doing

- No blog, post previews, RSS, or markdown integration (reference's Postex
  path).
- No new pages (about/contact) — single-page scope per VAR-664.
- No light/dark theme toggle, no JS, no animations framework.
- No Tailwind, npm, esbuild, or any asset build step.
- No favicon work (noted as a gap in research; separate concern).
- No changes to SingleThread page, error handling (`error.rs`), state
  (`state.rs`), or `main.rs`.
- No minijinja filters/globals/context data — templates remain fully static.

## Open Risks

- **CSS cache-invalidation**: once CSS is external, browsers cache
  `/static/site.css`; edits may not appear without a hard refresh. Mitigate
  now with a short note; consider a version query param later if it bites.
- **Image licensing/sizing**: copied reference assets were sized for
  Tailwind classes; may need explicit width/height attributes in
  `site.css`/markup to look right on the dark theme (e.g., white-background
  SVGs like GitHub/LinkedIn logos may need recoloring or rounded chips).
- **Test brittleness remains**: substring assertions stay sensitive to
  whitespace/attribute rewrites; accepted per decision 6 since the pattern
  is repo-standard.
- **Bio wording**: adapting the Elixir/Rust phrasing from the reference
  (`index.html.heex:14-16`) to this site's voice needs Alan's final sign-off
  during review of the implementation PR.
