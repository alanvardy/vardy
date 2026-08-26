# Research Questions

## Context

Focus on the Rust axum web application's request-to-response plumbing. Areas
of interest: how pages are defined, routed, and rendered; how request input
is extracted and validated; how outbound third-party HTTP calls are made and
configured; how abuse is mitigated per endpoint; how the shared page layout
and static assets are wired; and how the integration test harness is
structured. Do NOT assume any of these concern a specific new feature —
the goal is to map what exists today and how it works.

## Questions

1. **Routing, page handlers, and rendering.** Trace the full path a `GET`
   handler takes from route registration to rendered HTML: how a handler
   module is defined and registered under `src/interfaces/handlers/`, how
   routes are declared in `src/interfaces/routes.rs`, and what a handler must
   do to serve a full HTML page (AppState access, template lookup, error
   mapping). What per-domain conventions and shared helpers exist?

2. **Request body extraction and input validation.** How are incoming HTTP
   request bodies currently parsed and validated? Which axum extractors
   (e.g. `Json`, `Form`, path params) are in use, how are malformed or
   invalid inputs converted into responses, and are there any existing
   `POST` or HTML-form endpoints to model against? How would a plain HTML
   form (`application/x-www-form-urlencoded`) be handled?

3. **Outbound third-party HTTP calls end-to-end.** How does the application
   talk to external services? Trace the Unsplash integration from
   environment config (API key, base URL override) through `AppState` into
   an outbound `reqwest` call, including how failures map to `WebError`,
   how timeouts are configured, and how the base URL is overridden for
   tests.

4. **Configuration and secret lifecycle.** How is a new secret/config value
   declared and consumed across the app? Walk through `AppState::env`
   (`src/app/env.rs`), `.env_template`, `.env`, fly.io secrets, and the
   `Env` struct — what conventions require a new key to be enumerated in
   each of these places, and how does a value reach a handler?

5. **Rate limiting and abuse mitigation.** How is per-client rate limiting
   structured and composed? Describe the global limiter and the nested
   per-endpoint tiers (`src/app/rate_limit.rs`), the per-IP key extractor,
   how a `429` is rendered via the error chokepoint, and how a new endpoint
   would claim a stricter dedicated budget.

6. **Shared layout, navigation, and static assets.** How is the shared page
   layout (wallpaper, photographer credit, nav) composed in
   `templates/layout.html`, what render-context contract must every page
   template satisfy, how are new pages added to the navigation, and how are
   static assets referenced and cache-busted (`.css`/`.js`/images via
   `asset_url` and content hashing)?

7. **Integration test harness and external-service stubbing.** How is the
   integration test harness structured (`src/test/mod.rs`)? How do the
   `start_app*` variants build state, how are outbound HTTP dependencies
   stubbed (e.g. the Unsplash stub), what seed helpers exist, and what are
   the conventions for happy-path and sad-path tests (`#[sqlx::test]`,
   `test_client()`)?