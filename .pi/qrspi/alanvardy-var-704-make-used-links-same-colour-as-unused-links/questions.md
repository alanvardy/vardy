# Research Questions

## Context

The site is a small Rust (axum) web app using Tailwind CSS v4. Anchor element styling
is split between a Tailwind source stylesheet compiled into a committed static CSS
file, the utility classes used in the HTML templates, and browser-default link
behaviour. The styling pipeline and how CSS changes reach browsers matter as much as
the individual `<a>` rules themselves.

## Questions

1. **Anchor/colour rules in the Tailwind source:** In `css/site.css` (the Tailwind v4
   source, base/theme layers), what styling is applied to anchor (`a`) elements
   globally, and what browser-default `:link` / `:visited` colour and underline
   behaviour do anchors rely on when no rule targets them?

2. **Link styling across templates:** For every anchor in `layout.html`,
   `home.html`, and `singlethread.html`, what colour and text-decoration do they get
   from utility classes (e.g. `no-underline`, `hover:text-accent`, `text-accent`,
   `text-muted`, nav links), and which anchors have no explicit colour at all and
   therefore fall back to browser default visited styling?

3. **Compiled CSS pipeline:** How does the committed `static/site.css` get generated
   from `css/site.css` via `scripts/build-css.sh` with the standalone Tailwind CLI,
   how is the `@layer base` authored, and how are custom element rules (including
   pseudo-class selectors such as `:visited`) preserved through minification?

4. **Static asset cache-busting:** How does the server-side `asset_url()` in
   `src/app/assets.rs` compute the versioned `?v=` hash for CSS assets, and how does a
   change to `static/site.css` get picked up by browsers (hashes computed at startup,
   referenced from templates)?