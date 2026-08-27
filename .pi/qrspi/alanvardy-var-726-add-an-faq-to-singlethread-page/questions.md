# Research Questions

## Context

Study the SingleThread page (`templates/singlethread.html`, rendered by
`src/interfaces/handlers/singlethread/web.rs`) and the shared layout it
extends. Look at how content sections are composed with Tailwind utilities
and the repo's custom utility classes, how the page is tested, and how the
page's documented behavior is kept in sync.

## Questions

1. How are the content sections on the SingleThread page composed? Trace the
   headings, prose, list, badge, card, and divider building blocks in
   `templates/singlethread.html` and the custom utility classes
   (`heading-hero`, `heading-section`, `heading-subsection`, `text-muted`,
   `badge`, `card`, `divider`) defined in the template source.

2. Is there any existing pattern in the codebase for collapsible or
   expandable content (e.g. `<details>`/`<summary>`, accordion menus,
   disclosure widgets, or client-side JS interactivity)? If not, how is
   static content currently laid out on the page?

3. How does `templates/singlethread.html` inherit from
   `templates/layout.html`, and what context variables must a template
   supply (e.g. `wallpaper_url`, `photographer`, `active_page`)? How are
   shared chrome like the nav and wallpaper credit applied across pages?

4. How are the rendered SingleThread page contents asserted in
   `src/interfaces/handlers/singlethread/web.rs`'s tests? What string and
   structure checks exist, and how does `scripts/test.sh` (format, sqlx
   offline, clippy, tests, CSS-drift check) verify the page?

5. How is the `/singlethread` route documented in `ROUTES.md`, and what is
   the documented pattern for keeping page behavior and templates in sync
   with the site's static CSS (`static/site.css`, Tailwind build)?