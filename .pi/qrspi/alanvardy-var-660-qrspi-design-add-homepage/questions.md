# Research Questions

## Context

Focus on two sibling Rust repos. The `api` reference repo at
`/Users/vardy/dev/api` serves an HTML web service that renders pages from
minijinja templates in `templates/`. The `vardy` repo (current working
directory) is minimal, with no HTTP surface today. Understand how the `api`
repo serves template-rendered HTML requests and the present state of `vardy`
so the gap between the two is clear.

Areas to examine: `api`'s server bootstrap (`src/main.rs`), template
initialization (`src/app/templates.rs`), route registration
(`src/interfaces/routes.rs`), one HTML page handler
(`src/interfaces/handlers/*/web.rs`), the HTML templates themselves
(`templates/layout.html`, `templates/*.html`), and the current layout of
`vardy` (`Cargo.toml`, `src/main.rs`, test/toolchain configuration).

## Questions

1. How does the `api` repo bootstrap its HTTP server and wire template
   rendering into request handling? Trace `src/main.rs` and
   `src/app/templates.rs`: how is the minijinja `Environment` created,
   configured (path loader, autoescape callback), and threaded into the
   application state that handlers read?

2. What is the exact handler pattern for an HTML page in `api`? Pick one web
   handler (e.g. `src/interfaces/handlers/users/web.rs`): how is its route
   registered in `src/interfaces/routes.rs`, what extractor and return types
   does it use, and how does it fetch a template and render a minijinja
   context into the response?

3. How are the `api` HTML templates composed? Look at `templates/layout.html`
   and a concrete page template (e.g. `templates/users.html`,
   `templates/error.html`): what inheritance/block constructs does minijinja
   use, what does a page override, and how does autoescape apply to the
   rendered content?

4. How are HTML web routes tested in `api`? Look at the test modules inside a
   `web.rs` handler: how is the app started, what does the test client
   request, and what response-status/HTML assertions are made?

5. What is the present state of the `vardy` repo itself? Read `Cargo.toml`,
   `Cargo.lock`, `src/main.rs`, the toolchain/nextest/CI configuration, and
   any `AGENTS.md` module-convention rules (if present) so the difference
   between a bare console program and a template-serving service is clear.