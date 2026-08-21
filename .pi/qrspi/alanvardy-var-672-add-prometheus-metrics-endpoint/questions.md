# Research Questions

## Context
Focus on the Rust web service in `src/`, its dependency manifest, test
helpers, and deployment configuration (Dockerfile, fly.toml, `.github/`).
The service is small: an axum router in `src/interfaces/routes.rs`, handlers
under `src/interfaces/handlers/`, app state in `src/app/state.rs`, and startup
logic in `src/main.rs`.

## Questions
1. How are HTTP routes registered and how are handler modules organized under
   `src/interfaces/`? Trace the full flow from `src/main.rs` server startup to
   the router in `src/interfaces/routes.rs`, including how existing endpoints
   like `/health` are defined and what their handlers look like.
2. What middleware or tower layers (if any) are currently applied to the
   router, and where would per-request instrumentation hooks naturally live?
   Check `Cargo.toml` / `Cargo.lock` for which `tower` / `tower-http`
   features and versions are already available.
3. Is there any existing logging, tracing, error-reporting, or metrics
   instrumentation anywhere in the codebase (`src/`, `main.rs` println usage,
   `src/app/error.rs`)? Describe exactly what observability exists today.
4. How is application state (`AppState`) constructed and threaded into
   handlers, and what shared mutable state patterns (e.g., counters,
   atomics, `Arc`) exist in the codebase that handlers could read from?
5. What testing infrastructure exists for HTTP endpoints — trace
   `start_app` and `test_client` in `src/test/mod.rs` and the route tests in
   `src/interfaces/routes.rs`. What response content types and body assertions
   do current tests make?
6. How is the service deployed and run — examine the Dockerfile, fly.toml,
   `.env_template`, port binding in `main.rs`, and any GitHub workflows. Are
   health checks or scrape targets configured anywhere, and what ports are
   exposed?
