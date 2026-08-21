# Research Questions

## Context
This is an Axum-based Rust web application. Routing lives in
`src/interfaces/routes.rs` with handlers under `src/interfaces/handlers/`,
integration-test helpers in `src/test/`, and deployment configuration in
`fly.toml` and `Dockerfile`. A sibling project at `../api` is a related
Axum service with a similar structure.

## Questions
1. How are HTTP routes registered in `src/interfaces/routes.rs`, how are
   handlers organized under `src/interfaces/handlers/` (module layout,
   `web`/`api` submodules, state access), and what does a minimal handler
   that returns only a status code look like in this codebase or in `../api`?
2. How do the integration tests work — what do `start_app` and `test_client`
   in `src/test/` provide, how are route tests written (see the existing
   tests in `src/interfaces/routes.rs` and in `../api/src/interfaces/routes.rs`
   such as `health_check_returns_200`), and how is test coverage measured
   (codecov.yml, nextest config)?
3. How is the application deployed and monitored — do `fly.toml` and
   `Dockerfile` define any health checks, exposed ports, or probes, and does
   `../api` configure anything similar (e.g. a metrics router or Fly health
   check settings) that this service lacks?
