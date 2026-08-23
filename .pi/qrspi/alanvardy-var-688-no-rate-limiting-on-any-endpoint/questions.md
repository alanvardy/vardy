# Research Questions

## Context
Focus on the HTTP request-handling pipeline of this Rust axum application:
router construction and middleware layers, shared application state and
environment configuration, the error-response convention, the integration-test
harness, and deployment configuration (Docker/Fly.io).

## Questions
1. Trace the full request pipeline from `src/main.rs` through
   `src/interfaces/routes.rs`: where are layers attached, in what order do they
   run relative to routing, what state types do the routers use, and how does
   `into_make_service_with_connect_info::<SocketAddr>()` make peer address
   available to inner services?
2. What tower/axum middleware crates exist in `Cargo.toml` today, which
   features of `tower-http` and `tower` are enabled, how is `tower` used in
   tests (`oneshot`), and what axum/tower version compatibility constraints
   apply to adding a new middleware layer?
3. How does `src/app/env.rs` load and validate configuration: which env vars
   exist, how are optional values defaulted, what happens on missing/invalid
   input, and how are its unit tests structured (e.g. mutex serialization)?
4. How does the error convention in `src/app/error.rs` work: what variants and
   status codes does `WebError` map to via `IntoResponse`, what response bodies
   does it produce, and can a middleware (which sits outside handlers) return
   responses through it or reuse its body format?
5. How do the integration tests boot the app (`src/test/mod.rs`
   `start_app()` / `test_client()`): do requests come over a real TCP socket or
   `tower::ServiceExt::oneshot`, what would that mean for per-IP keying based
   on `ConnectInfo`, and is there any existing mechanism for per-test
   configuration overrides?
6. Which endpoints serve infrastructure vs. user traffic — how is `/health`
   implemented and consumed by Fly.io probes in `fly.toml`, and is `/metrics`
   served from the same router or a separate one with separate state?
7. What patterns do existing middleware-adjacent components follow for
   observability: what does `trace_layer()` in `src/app/log.rs` record per
   request, what metrics does `src/infra/metrics.rs` expose, and how are rate
   limiters conventionally observed (headers like `Retry-After`,
   `X-RateLimit-*`) in this stack's dependencies?
