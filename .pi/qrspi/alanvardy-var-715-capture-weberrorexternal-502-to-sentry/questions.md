# Research Questions

## Context

Focus on the app's centralized error path (`src/app/error.rs`), the external
integration error types that flow into it (`src/infra/resend.rs`,
`src/infra/unsplash.rs`), the Sentry setup (`src/infra/sentry.rs`,
`src/app/env.rs`, `src/main.rs`), and the test harness
(`src/test/mod.rs`). No code changes are being planned — only facts about
what exists and how it behaves today.

## Questions

1. How is the `WebError` enum's `IntoResponse` implemented in
   `src/app/error.rs`, arm by arm — which arms call `sentry::capture_error`,
   which only call `tracing::error!`, and which do neither? What is the exact
   payload type of each captured error?

2. Where are `WebError::External` values constructed, and what string
   messages do the `ResendError` and `UnsplashError` newtype wrappers carry
   on each failure path (transport, non-2xx, parse)? What semantic
   information is available in those strings?

3. What do the `WebError::Database` and `WebError::Template` arms capture to
   Sentry today (which types, how), and what does the `sentry` crate offer in
   this codebase's version (0.49) for capturing a `String` message vs an
   error — e.g. `capture_message` or event-level APIs?

4. How is Sentry initialized and configured at startup, and under which
   environment conditions is a client active at all — what would the `External`
   arm's capture call do when Sentry is disabled or unconfigured?

5. How do existing tests exercise the error path — what does the test harness
   do to keep Sentry out of play (`enable_sentry`, DSN values), and are there
   any tests anywhere that assert on Sentry capture behavior, or only on HTTP
   status codes?

6. What architectural constraints (e.g. the arkitect module-dependency rules
   in `src/test/arkitect.rs`) govern where Sentry may be called from, and how
   does the current `Database`/`Template` capture in the `app` layer relate to
   those rules?