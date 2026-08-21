# Research Questions

## Context
This is a small axum web server backed by SQLite (sqlx) with minijinja
templates. Focus on: route/handler structure under `src/interfaces/`,
database setup under `src/app/`, configuration loading in `src/main.rs`,
templating in `src/app/templates.rs` and `templates/`, and test helpers in
`src/test/mod.rs`.

## Questions
1. How are HTTP routes registered and handlers organized? Trace the flow
   from route registration through an existing handler (e.g. home,
   singlethread, health) to its response and error types, including how
   handler modules are declared and wired into the app state.
2. How is the SQLite database accessed? Trace pool creation, how queries
   are written and organized, where migrations live, and whether/where
   migrations are applied at runtime versus in tests.
3. How are environment variables and secrets loaded and made available to
   the application? What existing patterns exist for reading config at
   startup and passing it to handlers?
4. What HTTP client dependencies and outbound-request patterns exist in
   the project (production vs dev dependencies), and how are JSON payloads
   serialized/deserialized elsewhere in the codebase?
5. How are HTML pages rendered? Trace template loading, layout
   inheritance, context construction, and how static assets (e.g. images)
   are served and referenced from templates.
6. What testing conventions exist for endpoints and database-backed code?
   How do integration tests start the app, make requests, and use
   migrations or in-memory databases?
