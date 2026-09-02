# Research Questions

## Context

Focus on the centralized error path in `src/app/error.rs` (`WebError` and
its `IntoResponse` impl), the Sentry module `src/infra/sentry.rs` and its
startup wiring (`src/main.rs`, `src/app/env.rs`), the `sentry` crate's
public API surface as pinned in this build, the test harness
(`src/test/mod.rs` and inline `#[cfg(test)]` modules), and the architectural
rules in `src/test/arkitect.rs`. Only facts about what exists and how it
behaves today.

## Questions

1. Enumerate every Sentry capture site in the crate: which `sentry::`
   APIs are invoked (`capture_error`, `capture_message`, `capture_event`,
   …), from which files and lines, and what payload type each captures
   (concrete error types vs `String`). For each site, what distinguishes it
   from the others — error type identity, message-string contents, or
   surrounding tracing context?

2. What public API does the `sentry` crate (the version pinned in this
   build's `Cargo.lock`) expose for attaching structured metadata such as
   tags or context to a captured event? How do scope-level tags
   (e.g. `Scope::with_tag(s)`) differ from event-level tags in what they
   attach to, and how do scope tags behave across events and threads? Does
   the crate offer `capture_message`, and how does it differ from
   `capture_error` in what the client records?

3. How is the Sentry client configured at startup — which `ClientOptions`
   are set in `src/infra/sentry.rs` / `src/app/env.rs` / `src/main.rs`
   (release, PII, sampling, logs), under what environment conditions is a
   client active at all, and is there any tracing/OTel bridge feeding
   Sentry, or are captures exclusively explicit `sentry::` calls?

4. What does the test harness do to keep Sentry out of play
   (`enable_sentry` flag, DSN values in `src/test/mod.rs`)? Does any test
   anywhere assert on Sentry capture behavior — e.g. is the crate's test
   transport (`sentry::test::with_captured_events` in 0.49) used, and is
   the `sentry` cargo feature that provides it enabled in `Cargo.toml` —
   or do error-path tests only assert HTTP status codes and response
   bodies?

5. What module-dependency rules do the arkitect tests in
   `src/test/arkitect.rs` enforce? Which layers may import the `sentry`
   crate directly, which may call into `vardy::infra` (where
   `src/infra/sentry.rs` lives), and what restrictions apply to
   `vardy::app` and `vardy::interfaces` in each direction?

6. How is `WebError` extended today — what `From` impls feed upstream
   errors (`UnsplashError`, `ResendError`) into it, and is the
   `IntoResponse` match the only place error-to-Sentry capture decisions
   are made? Are captures or `tracing::error!` calls ever triggered outside
   `IntoResponse` (middleware, background work, `main.rs`), and what do the
   `tracing::error!` calls beside each capture record?