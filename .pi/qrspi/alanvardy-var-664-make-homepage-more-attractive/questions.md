# Research Questions

## Context
Focus on the Rust/Axum web app in this repository (templates, handlers, routes,
static assets, tests) and the reference Elixir/Phoenix site at
`/Users/vardy/dev/alan_vardy` (homepage template, styling, assets, content).

## Questions
1. How does the minijinja templating layer work end-to-end in this app — how the
   environment is initialized, how layout inheritance and blocks are used, how
   context data is passed from handlers to templates, and what autoescaping rules
   apply?
2. What does the reference site's homepage actually display, section by section
   (bio text, photos, icon links, post previews), and where does each piece of
   that content live — hard-coded in templates, config files, or external files?
3. How is page styling currently structured in this app (the inline CSS in
   `layout.html`, CSS variables, theme colors, fonts, container/card/nav classes),
   and how does the reference site organize its styles (CSS framework, asset
   pipeline, custom classes) by comparison?
4. How are static assets served and organized — the `/static` route setup, which
   assets exist, how templates reference them, and how images/icons are stored and
   referenced in the reference site's `priv/static` tree?
5. How do the existing HTTP tests assert rendered page content, what exact strings
   do the home-page tests depend on, and what patterns would new or updated page
   assertions follow?
6. How are routes and handlers wired for adding a new page (router composition,
   handler module structure, `AppState`, error handling), and does the reference
   site have additional pages (about/contact/blog) whose structure could inform a
   multi-page layout?
