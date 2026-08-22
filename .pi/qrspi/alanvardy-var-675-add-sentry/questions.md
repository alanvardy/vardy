# Research Questions

## Context
Focus on application startup in `src/main.rs`, configuration/environment
handling, the `WebError` type and its conversions in `src/app/error.rs`, the
router construction in `src/interfaces/routes.rs`, shared test setup in
`src/test/mod.rs`, deployment config (`Dockerfile`, `fly.toml`), and
dependencies declared in `Cargo.toml`. The sibling project at `/Users/vardy/dev/api`
contains an existing implementation of similar infrastructure worth comparing against.

## Questions
1. How does `src/main.rs` bootstrap the application end-to-end (env loading,
   state construction, listener binding, serving), and where in that sequence
   would an early-initialization component naturally fit?
2. How are environment variables currently read and validated in this app
   (`DATABASE_URL` handling, defaults, Dockerfile env vars), and how does the
   sibling `/Users/vardy/dev/api` project structure its typed `Env` struct for
   optional string/boolean settings?
3. How does the sibling project's `src/infra/sentry.rs` initialize the Sentry
   client, customize panic hooks, and filter broken-pipe panics — and which
   sentry crates/features does its `Cargo.toml` declare?
4. How does `WebError` in `src/app/error.rs` get created, converted from
   template/database errors, rendered into responses, and reported today
   (`eprintln!`) — and does the sibling project capture errors into Sentry
   anywhere in its error path?
5. What middleware/tower layers are applied to the routers in
   `src/interfaces/routes.rs`, and what layers or request-capture mechanisms
   does the sibling project attach to its axum router?
6. How does `src/test/mod.rs` construct `AppState` and boot the test app, and
   how does the sibling project's equivalent handle fields like
   `sentry_dsn`/`enable_sentry` in its test state so tests run with Sentry off?
7. What deployment configuration exists (`Dockerfile`, `fly.toml`), which
   environment variables and secrets does Fly.io expect, and how does the
   sibling project document or pass its Sentry-related secrets?
