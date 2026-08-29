# Research Questions

## Context

Study the three minijinja page templates (`templates/home.html`,
`templates/contact.html`, `templates/singlethread.html`) and the shared
`templates/layout.html` they extend, the hand-authored Tailwind v4 source in
`css/site.css` and its compiled output `static/site.css`, and the render,
handler, and test paths that back them. Focus on how layout width is
constrained at different viewport sizes, how the wallpaper background and
its photographer credit are supplied and displayed, and what conventions
and gates govern responsive behavior and CSS changes.

## Questions

1. Trace how page width is constrained across the site: the `container`
   element in `templates/layout.html`, the custom `@layer base` rule in
   `css/site.css`, and Tailwind v4's built-in `.container` utility as
   compiled into `static/site.css`. Given the layer ordering and the
   responsive max-widths emitted by the utility, what effective widths and
   padding result at viewports below 40rem, 40–48rem, 48–64rem, and ≥64rem?

2. How is the contact page's two-column layout composed in
   `templates/contact.html` — the `flex flex-col md:flex-row` wrapper, the
   `md:flex-1` columns, and the submit button's `self-start` — and what
   width does the form column occupy at each breakpoint? How do the
   `form-input`, `form-label`, and `btn` component classes size their
   content within that column, and what layout-related assertions exist in
   the tests in `src/interfaces/handlers/contact/web.rs`?

3. Trace the wallpaper background and artist accreditation markup in
   `templates/layout.html`: the `.wallpaper` div (inline `background-image`
   style) and the `fixed bottom-3 right-3` credit bubble with its linked
   vs. plain-text photographer variants. When is each element included or
   suppressed, how do the three page handlers (`home`, `contact`,
   `singlethread`) supply `wallpaper_url` / `photographer` /
   `photographer_url` via `picture::wallpaper_context`, and how do handler
   tests (`src/interfaces/handlers/{home,contact,singlethread}/web.rs`)
   assert that markup?

4. What responsive and mobile conventions exist across the three page
   templates (mobile-first `flex flex-col md:flex-row` stacking, `order-first
   md:order-none`, `md:`-only breakpoint variants, arbitrary-width
   utilities) and in `css/site.css`? Are there any hand-written `@media`
   queries anywhere, and what decisions in prior QRSPI design docs for
   VAR-719 and VAR-726 shaped the current approach?

5. How does the CSS build pipeline work — `css/site.css` source →
   `scripts/build-css.sh` (pinned Tailwind standalone CLI) → committed
   `static/site.css`, with matching steps in CI and the Dockerfile — and
   what does the CSS-drift gate in `scripts/test.sh` enforce? What
   constraints do that gate and the handler tests' HTML-substring
   assertions place on adding new utility classes or restructuring
   template markup, and how is `static/site.css` served (caching, asset
   hashing via `asset_url`)?