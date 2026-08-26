# Research Questions

## Context

Focus on the axum web layer under `src/interfaces/handlers/`, the minijinja
templates under `templates/`, the Tailwind v4 CSS-first setup in `css/` and its
generated `static/site.css`, the static image assets in `static/`, the shared
`layout.html` chrome, and the route documentation in `ROUTES.md`.

## Questions

1. Trace the full request-to-response flow for the `/contact` route (both GET
   form and POST submit paths) and the `/singlethread` route: handler layer,
   metrics instrumentation, template rendering context, the `submitted`
   flag/thank-you branch, and how each template extends `layout.html` and the
   blocks/classes it inherits.

2. How is styling organized in this app? Describe the Tailwind v4 CSS-first
   source (`css/site.css`), the theme tokens (`@theme` colors), the
   `scripts/build-css.sh` compile step, the generated `static/site.css`, and
   what shared layout/reusable markup patterns (nav, wallpaper, page/container
   wrappers) already exist that a page template relies on.

3. What static image assets are available under `static/` for the SingleThread
   page vs the Contact page, and how are they referenced/served — what does the
   `asset_url` helper do, and how is the `/static` asset service configured
   (cache headers, versioning)?

4. What visual/composition patterns are commonly used to make a landing or feature page attractive when styled with Tailwind — hero sections, card grids, call-to-action blocks, forms, image/asymmetrical layouts, and typography treatments — and which of these do other pages in this repo (e.g. `templates/home.html`) already demonstrate?

5. What do the existing tests assert about the HTML body of `/contact` and
   `/singlethread` — the specific element selectors, class names, form field
   names, and nav-chrome strings checked in `src/interfaces/handlers/*/web.rs`
   and `src/test/mod.rs` that any markup change would need to keep passing?

6. What non-template concerns must stay in sync when page markup changes — how
   is `ROUTES.md` organized per route, and how are the page-view metric labels
   (`inc_page_view`) and any other docs/references tied to the `/contact` and
   `/singlethread` pages?