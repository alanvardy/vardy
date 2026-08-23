# Structure Outline

## Approach

Port the proven `../api` GCRA rate-limit setup: a `tower-governor` global
limiter keyed by `Fly-Client-IP` (with `ConnectInfo` fallback) applied to the
whole :3000 router, stricter hard-coded tiers for `POST /dump/{key}` and
`/unsplash`, limits from required env vars, and all 429s funneled through one
`WebError`-shaped response. No database involvement anywhere — "vertical"
here spans **config → middleware → HTTP behavior → tests → docs**, which each
phase crosses end-to-end.

## Phase 1: Global per-IP limiter, wired and invisible to existing tests

Delivers end-to-end: every :3000 request now passes through a working GCRA
limiter, configured by two new required env vars, with the key extractor
unit-tested — while the test harness keeps limits effectively disabled so all
existing suites stay green.

**Files**: `Cargo.toml`, `src/app/rate_limit.rs` (new), `src/app/env.rs`,
`src/app/mod.rs`, `src/main.rs`, `src/test/mod.rs`

**Key changes**:
- `FlyClientIpKeyExtractor` — impl of governor's `KeyExtractor`: reads
  `fly-client-ip` header, falls back to `ConnectInfo<SocketAddr>`, ignores XFF;
  with unit tests (header present / XFF-only rejected / fallback works)
- `Env { rate_limit_per_ms: u64, rate_limit_burst: u32 }` — new required
  fields parsed in `init()` via a new `get_parse_env::<T>(key)` panic-on-
  missing/invalid helper
- `with_global_limit(router: Router<AppState>, per_ms: u64, burst: u32) -> Router` — wraps router with `.use_headers()` governor layer
- Harness: switch both servers to `into_make_service_with_connect_info::<SocketAddr>()`; `Env` literal sets `per_ms: 1, burst: 1_000_000`
- `main.rs`: call `with_global_limit(...)` after tracing layer

**Verify**: `./scripts/test.sh` passes (all existing suites green); manual —
run server with `RATE_LIMIT_PER_MS`/`RATE_LIMIT_BURST` in `.envrc`, hammer an
endpoint with curl, observe eventual 429s and standard rate-limit headers.

---

## Phase 2: 429 through the WebError chokepoint + integration proof

Delivers end-to-end: over-limit responses get our exact error shape — plain
text `"too many requests"`, `Retry-After` header, no Sentry capture — proven
by real-TCP integration tests using a new tight-limit harness entry point.

**Files**: `src/app/error.rs`, `src/app/rate_limit.rs`, `src/test/mod.rs`,
`src/interfaces/routes.rs` (tests)

**Key changes**:
- `WebError::TooManyRequests { retry_after_secs: u64 }` — new variant;
  `IntoResponse` arm returns `(429, "too many requests")` + `Retry-After`, no
  sentry capture (mirrors `External`)
- Custom governor error responder in `rate_limit.rs` producing exactly that
  response (fallback per design risk: thin rejection-mapping wrapper layer)
- `start_app_with_rate_limits(per_ms: u64, burst: u32) -> (SocketAddr, SqlitePool)` — minimal second harness builder reusing `start_app_with` internals
- Integration tests: assert status == 429 **and** body == `"too many requests"`
  **and** `Retry-After` present (happy + sad: under-limit request unaffected)

**Verify**: `./scripts/test.sh` passes including new 429 tests; manual — curl
past the limit, confirm exact body text and header.

---

## Phase 3: Stricter tiers for dump + unsplash

Delivers end-to-end: `POST /dump/{key}` and `/unsplash` sit behind their own
hard-coded, tighter budgets inside the global one — cheap endpoints keep the
global allowance while expensive ones throttle first.

**Files**: `src/interfaces/routes.rs`, `src/app/rate_limit.rs`,
`src/interfaces/handlers/dump/` (tests), `src/interfaces/handlers/unsplash/`
(tests), `ROUTES.md`

**Key changes**:
- `tiered_routes(limited: Router<AppState>, per_ms: u64, burst: u32) -> Router<AppState>` — nested-router helper wrapping a route group with its own governor layer (cf. api's `auth_routes()`)
- Hard-coded tier constants in `rate_limit.rs` (e.g. `DUMP_TIER`, `UNSPLASH_TIER`) — policy in code, tuned like api's auth tier
- Route registration: dump POST + unsplash moved through the tiered group; GET `/dump/{key}`, pages, `/health` unchanged
- Integration tests: trip only the tier limit while global budget stays open; also confirm GET dump shares only the global budget
- `ROUTES.md`: 429 behavior + headers documented per affected endpoint block

**Verify**: `./scripts/test.sh` passes; manual — exceed the unsplash tier and
confirm 429 while `/` still serves normally.

---

## Phase 4: Store hygiene + deploy readiness

Delivers end-to-end: the in-memory key store can't grow unboundedly, and every
place that must know the new env vars does — deploys and CI won't break at
startup.

**Files**: `src/app/rate_limit.rs`, `src/main.rs`, `.envrc`, `fly.toml`
(secrets noted in commit message if managed via fly CLI)

**Key changes**:
- `spawn_pruner(store: Arc<Mutex<...>>)` — background task calling
  `retain_recent()` every 60s, spawned once inside `with_global_limit`'s setup
  (no jobs framework invented)
- Wire pruner spawn in `main.rs`; ensure shutdown isn't blocked by the task
- `.envrc.example` / local `.envrc`: document `RATE_LIMIT_PER_MS`,
  `RATE_LIMIT_BURST`; note fly secret commands in PR description

**Verify**: `./scripts/test.sh` passes; manual — boot server >60s, watch logs
for prune cadence, memory stable under load; confirm fresh checkout fails fast
with a clear panic when either env var is missing.

## Testing Checkpoints

- **After Phase 1**: extractor unit tests pass; all pre-existing suites green
  with limits disabled (`per_ms: 1, burst: 1M`); manual curl can produce a 429.
- **After Phase 2**: integration tests prove 429 status + exact body +
  `Retry-After` over real TCP; `WebError` chokepoint owns the format.
- **After Phase 3**: tier limits trip independently of the global budget;
  `ROUTES.md` documents 429 behavior.
- **After Phase 4**: pruner runs on cadence; missing-env startup fails fast
  with clear messages; full gate green — safe to open PR.
