# Task: Add a hello-world homepage

Add an HTML "hello world" homepage to the `vardy` repo. Today `vardy` is a
bare Rust console program (`src/main.rs` prints "Hello, world!" and has no
HTTP server). We want it to serve a simple HTML homepage at `/`, following
the templating approach used by the sibling `api` repo (`/Users/vardy/dev/api`):
an axum HTTP server that renders HTML from minijinja templates in a
`templates/` directory, with a shared `layout.html`.

Why: a minimal web presence for the `vardy` package, and a reference for how
templating is structured in this codebase. The design phase (this branch) will
research the `api` templating mechanics and the current `vardy` state, then
produce a build plan.