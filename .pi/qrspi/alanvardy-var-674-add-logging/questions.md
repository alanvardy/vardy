# Research Questions

## Context
Focus on application startup, the axum router composition and middleware
chain, error handling and its output paths, and the test harness. The
sibling project at `../api` contains an established pattern for the same
concerns and is worth examining for comparison.

## Questions
1. How is the application initialized in `src/main.rs` — what runs before
   the servers start, in what order are the two axum services (ports 3000
   and 9090) built and served, and where is the only existing stdout output
   produced?
2. What middleware, layers, or tower-http services are currently applied to
   the main router in `src/interfaces/routes.rs` (including the static-file
   `nest_service`), and how are the main router and metrics router composed
   differently?
3. How does `WebError`'s `IntoResponse` impl in `src/app/error.rs` surface
   errors today — what is written to stderr, what detail is available at
   that point (error variant, source error, request info), and what do its
   unit tests assert?
4. What logging or tracing crates and patterns does the sibling project
   `../api` use — how does `../api/src/app/log.rs` initialize its
   subscriber, configure filtering and output format, and wire request
   tracing into its router?
5. How does the test harness in `src/test/mod.rs` boot the app
   (`start_app`, `start_app_with_metrics`) — does it call `main`, spawn its
   own `axum::serve`, and would anything initialized globally at startup
   (e.g. a process-wide subscriber) run once or multiple times under
   `cargo nextest`?
6. How is the app deployed and run in production (`Dockerfile`,
   `fly.toml`) — what does stdout/stderr capture look like, and are there
   any environment variables or configuration conventions already in use
   (e.g. `.env`, `DATABASE_URL`)?
