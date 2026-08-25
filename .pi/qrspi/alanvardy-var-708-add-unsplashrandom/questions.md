# Research Questions

## Context

The repo is an axum + sqlx (SQLite) web app. Focus on the Unsplash picture
handling: the end-to-end request flow for `GET /unsplash`, the data-access
layer over the `unsplash_pictures` table, the rate limiting applied to
unsplash routes, the integration test harness for unsplash, and any
randomness/selection utilities available in the codebase and dependency set.
All relevant source lives under `src/` and `migrations/`.

## Questions

1. Trace the full request flow for `GET /unsplash`, from route registration
   (including its rate-limit tier) through the handler to the app and data
   layers and out to the upstream Unsplash API. Identify the exact functions,
   types, and `file:line` locations involved at each step.

2. What query forms and row-mapping patterns does the data-access layer use
   against the `unsplash_pictures` table (e.g. `ORDER BY id DESC LIMIT 1`,
   `INSERT ... RETURNING`, `sqlx::query_as` / `query_scalar`, count vs
   row-selection queries)? Where are they defined and used, and how is the
   `Picture` type mapped from rows?

3. How are routes and their stricter per-IP rate-limit tiers defined, merged,
   and nested inside the global limiter in `routes.rs` and `rate_limit.rs`?
   Exactly how does the existing `/unsplash` route register its handler and
   its `UNSPLASH_TIER_*` budget?

4. What test harness and patterns exist for unsplash: how are rows seeded and
   cleared, how is the Unsplash stub server wired, and how are integration
   tests written to assert both HTTP status and response body? Give concrete
   examples with `file:line` references.

5. What randomness or random-selection primitives are already available —
   either in the Rust dependency set (e.g. a `rand` crate) or via SQLite
   (e.g. `RANDOM()` / `ORDER BY RANDOM()`) — and where, if anywhere, are they
   currently used in the codebase?