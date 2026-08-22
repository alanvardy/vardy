# Research Questions

## Context
Focus on the axum web layer under `src/interfaces/`, the minijinja templates
under `templates/`, static assets under `static/`, and route documentation in
`ROUTES.md`.

## Questions
1. Trace the full request-to-response flow for the `/singlethread` route:
   handler, metrics, template rendering context, layout inheritance, and which
   blocks/classes from `layout.html` and the shared CSS the template relies on.
2. How are static assets served, cache-busted, and referenced — what does
   `asset_url` do, how is the `/static` service configured with cache headers,
   and what is the established pattern for adding a new image asset?
3. What layout and styling patterns do other content pages in this app use for
   richer presentations (image sections, multi-column or screenshot layouts,
   cards, typography) — survey `templates/*.html` and the CSS they share?
4. How are page-handler tests written for this app (what is asserted about
   status and body content), specifically what do the existing singlethread
   tests assert that any template change would need to keep passing?
5. What must be updated when a page's markup changes — how is `ROUTES.md`
   organized, and are there other docs, metrics labels, or nav/link references
   tied to specific singlethread page content?
