# Design — VAR-688: Per-client-IP rate limiting

Decisions mirror the proven implementation in the sibling `../api` project,
adapted to this repo's conventions (WebError chokepoint, two-server split).

## Current State

- No rate limiting anywhere in `src/`; only layers are `TraceLayer`
  (`src/main.rs:42`) and static-file cache headers (`src/interfaces/routes.rs:30-37`).
- Two-server architecture (`src/main.rs:37-49`): all user traffic on :3000 via
  `routes().with_state(state).layer(trace_layer())
  .into_make_service_with_connect_info::<SocketAddr>()` (:40-43); `/metrics`
  isolated on :9090 with its own router/state and no layers (:45-47).
- `ConnectInfo<SocketAddr>` extension is inserted in production but consumed by
  nothing; no proxy-header handling exists.
- Errors flow exclusively through `WebError::into_response` — plain-text bodies,
  `(StatusCode, &'static str)` style (`src/app/error.rs:35-53`). Sentry capture
  lives inside that impl; client-caused errors (`External`) are NOT sentried.
- Config loading is fail-fast panic, zero defaults (`src/app/env.rs:29-42`).
- Test harness boots real TCP servers with plain `into_make_service()`
  (`src/test/mod.rs:45,79`); `Env` is a hard-coded struct literal (:20-26); the
  only config seam is `start_app_with(unsplash_base_url)` (:19). All test
  traffic comes from 127.0.0.1, so per-IP differentiation requires header-based
  identity.
- Fly probes `/health` every 30s (`fly.toml:20-25`); machine stop/start depends
  on it passing (`fly.toml:15-17`).

## Desired End State

Every request to :3000 passes through a global per-IP GCRA limiter;
`POST /dump/{key}` and `/unsplash` additionally pass a stricter tier.
Over-limit requests get **429** with plain-text body `"too many requests"` and
standard rate-limit headers. `/metrics` (:9090) stays unlimited. `/health`
shares the global budget. Limits come from required env vars. Verified by:

- unit tests for the key extractor (header preference, XFF ignored, fallback);
- integration tests asserting 429 status AND body under a tight-limit config;
- existing suites still green with limits effectively disabled in the harness;
- `ROUTES.md` documenting 429 behavior per endpoint.

## Patterns to Follow

Follow `../api` (it solved this exact problem) adapted to this repo:

- **Key extractor**: clone of `FlyClientIpKeyExtractor`
  (`../api/src/app/rate_limit.rs:17-39`) — read `fly-client-ip` first (cannot
  be spoofed through Fly's edge), fall back to `ConnectInfo<SocketAddr>` peer
  address for local dev/tests, deliberately ignore `X-Forwarded-For` (Fly
  appends to it → spoofable).
- **Global limit wiring**: apply in `main.rs` after tracing, before serving —
  cf. `../api/src/main.rs:85`
  (`rate_limit::with_global_limit(router, env.rate_limit_per_ms, env.rate_limit_burst)`).
- **Stricter tiers via nested-router helper**: build route group, wrap with its
  own limiter inside routes construction — cf. `auth_routes()` in
  `../api/src/interfaces/routes.rs:32-43`.
- **Env convention**: required vars parsed in `Env::init()` with the existing
  `must be set and non-empty` panic — identical pattern already shared between
  repos (`../api/src/app/env.rs:31-32`, `src/app/env.rs:29-34`).
- **Store pruning**: background task calling `retain_recent()` every 60s so the
  in-memory key store doesn't grow unboundedly — cf.
  `../api/src/app/jobs/rate_limit_prune.rs`. This repo has no jobs module; put
  the spawn inside the rate-limit module rather than inventing a jobs framework.
- **Tests disable limiting**: harness sets `per_ms: 1, burst: 1_000_000` so
  existing integration tests never trip it — cf.
  `../api/src/test/mod.rs:208-209`.

Patterns NOT to follow:

- Do **not** let tower-governor emit its default JSON-ish 429 body unchecked:
  our rule is all errors surface through the `WebError` chokepoint
  (`src/app/error.rs`) and tests assert body content. `../api` skipped this;
  we don't.
- Do **not** hand-roll a token bucket (rejected: dependency-free but we'd own
  concurrency/eviction correctness for no benefit over governor's GCRA).
- Do **not** trust `X-Forwarded-For`.

## Design Decisions

1. **Library**: `tower_governor = "0.8"` (+ `governor = "0.10"`), features
   `["axum"]` — battle-tested GCRA, standard headers via `.use_headers()`,
   compatible with our axum 0.8/tower 0.5/http 1.x stack.
2. **Client identity**: `Fly-Client-IP` header → `ConnectInfo` fallback;
   XFF deliberately ignored.
3. **Coverage**: single global limiter on the whole :3000 router (`/health`
   included — it fits easily in a sane global budget at 2 req/min probe rate);
   stricter hard-coded tiers for `POST /dump/{key}` and `/unsplash`;
   `/metrics` untouched on :9090.
4. **Config**: required `RATE_LIMIT_PER_MS` and `RATE_LIMIT_BURST` env vars,
   fail-fast panic on missing/invalid; tier limits for dump/unsplash hard-coded
   in code (like api's auth tier) — they're policy, tuned with the code.
5. **429 shape**: extend `WebError` with `TooManyRequests { retry_after_secs }`
   → 429, body `"too many requests"`, `Retry-After` header, no Sentry capture
   (client fault — consistent with `External` skipping Sentry, `error.rs:49-52`).
   Wire governor's error responder to produce exactly this response so
   middleware-sourced 429s and any future handler-level 429s share one format.
6. **Test strategy**: unit tests for extractor (happy/sad: header present,
   XFF-only rejected, fallback works); a dedicated tight-limit app builder in
   the test harness for integration 429 tests (assert status + body +
   `Retry-After`); default harness keeps limits effectively disabled.

## What We're NOT Doing

- No persistent/distributed rate-limit store (in-memory per-machine is fine at
  this scale; multi-instance machines get independent budgets — accepted).
- No authenticated-user-based keying (all endpoints unauthenticated today).
- No HTTP-level request metrics beyond what exists; no new Prometheus counters
  for 429s unless trivially cheap to add later.
- No changes to `/metrics` port/router or the two-server split.
- No exemption machinery for `/health`.
- No dotenv loader, no config defaults, no runtime limit changes.
- Not touching `External`-variant Sentry behavior.

## Open Risks

- tower-governor's error-response customization API differs across versions;
  if 0.8's responder hook can't reproduce our exact body/header shape, fall
  back to mapping `GovernorError` rejections in a thin wrapper layer around the
  limited routers (still funneled into one `WebError`-shaped builder).
- All production traffic arrives via Fly proxy: if `Fly-Client-IP` is ever
  absent (config change), everything falls back to the proxy IP = one shared
  bucket. Mitigation: the global budget must be sized so a single-bucket worst
  case doesn't lock out legitimate traffic entirely.
- Integration 429 tests need a second app-builder entry point in
  `src/test/mod.rs`; keep it minimal (one extra function, reuse `start_app_with`
  internals) to avoid a config-builder sprawl.
- New env vars must be added to `.envrc`, fly secrets, and the harness `Env`
  literal in the same PR or deploys/tests break at startup.
