# Research Questions

## Context

This is a small axum web server backed by SQLite (sqlx) with minijinja
templates and a Tailwind CSS v4 pipeline. The home page (`templates/home.html`)
extends `templates/layout.html` and renders a fixed, full-screen wallpaper from
the Unsplash proxy. Focus on the home-page render path, the layout template and
its shared chrome, the `Picture` type and data available to templates, the CSS
source/compilation pipeline, and the home-page HTML tests.

## Questions

1. Trace the home page render flow end to end: how `src/interfaces/handlers/home/web.rs` fetches the `Picture` via `src/app/picture.rs`, which fields it extracts into the template context, how `context!` is built in `render(state.templates.get_template("home.html"))`, and how the wallpaper value reaches the `layout.html` `.wallpaper` div. How does `minijinja` context plus layout/block inheritance work here?

2. How is the `Picture` type (in `src/domain/picture.rs`) populated, and what distinguishing data does it carry beyond the image `url`? Where do the additional string fields come from — the upstream parse in `src/infra/unsplash.rs` and/or the `unsplash_pictures` table? When can those fields be empty (e.g. legacy rows, the stub used in tests)?

3. How does `layout.html` handle template data that may or may not be present? Trace the `.wallpaper` div's `{% if wallpaper_url %}` conditional and `aria-hidden="true"`, and describe the existing minijinja idioms in the templates for conditionally rendering a value and rendering a link with `target`/`rel` attributes.

4. What is the Tailwind CSS v4 build pipeline? Trace `css/site.css` (the CSS-first source with `@layer`, `@theme`, `@source not ...` directives) through `scripts/build-css.sh` into the committed `static/site.css`, and explain how `static/site.css` is referenced/versioned from templates via `asset_url()` in `src/app/templates.rs`. How would new utility classes or hand-written base rules get into the compiled output?

5. What styling and accessibility conventions apply to the shared chrome? Summarize the `.wallpaper`, `.page`, `.container`, and global `a` (link) rules in `css/site.css` (colors, `:visited`, `:focus-visible`, the WCAG 1.4.1 non-color-differentiator comment), and how existing decorative vs interactive `role`/`aria-hidden` attributes are used in the templates.

6. How are the home page's rendered HTML and the `/unsplash` JSON response tested? Describe the assertions in `src/interfaces/handlers/home/web.rs` `mod tests` and `src/interfaces/handlers/unsplash/json.rs` `mod tests`, the shared harness in `src/test/mod.rs` (`start_app`, `start_app_with`, `start_unsplash_stub`), and how they check template tokens, asset URLs, and JSON fields.