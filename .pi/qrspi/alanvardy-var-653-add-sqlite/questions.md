# Research Questions

## Context

Focus on this repo's application core (`src/app/`, `src/interfaces/`,
`src/test/`), its build/deploy configuration (`Dockerfile`,
`.github/workflows/`, `fly.toml`, `Cargo.toml`), and the sibling repository at
`../api`, which contains historical database configuration in its git history.

## Questions

1. How is `AppState` defined, constructed, and threaded through the router and
   handlers (`src/main.rs`, `src/app/state.rs`, `src/interfaces/routes.rs`),
   and which code paths (production vs. test) construct it independently?

2. In `../api`'s git history before its Postgres switch, how was SQLite
   configured end-to-end: which crates and feature flags were used, how were
   connections created and shared across handlers, how did migrations run,
   and what query/metadata conventions existed?

3. How does the existing error handling work (`src/app/error.rs`, the
   `WebError` type and its `IntoResponse` impl), and what pattern would new
   fallible operations follow to surface errors as HTTP responses?

4. How are tests organized (`src/test/mod.rs` helpers, inline `#[cfg(test)]`
   modules, `reqwest` dev-dependency), and what constraints exist around
   parallelism, fixtures, or coverage thresholds that affect code touching
   shared resources?

5. What do the Dockerfile runtime image, `fly.toml` VM config, and CI
   workflows (coverage gates, string linting, deploy-on-merge) assume about
   the dependency set, and where do persistent files live relative to the
   deployed app?
