# Design — Add Sentry (VAR-675)

## Current State
- Bootstrap is a single 34-line `async fn main()`: read `DATABASE_URL` (only env
  var, silent inline fallback), build `AppState` (templates/db/metrics), bind
  `0.0.0.0:3000` + `0.0.0.0:9090`, serve both under `tokio::try_join!`
  (`src/main.rs:5-30`). Nothing runs before the env read; first request is
  possible only inside `try_join!`.
- Zero telemetry: no tracing/log facade, no sentry dep (`Cargo.toml:6-19`).
  The only error "logging" is two `eprintln!` calls in `WebError::IntoResponse`
  for `Database` and `Template` variants, each followed by a plain-text 500
  (`src/app/error.rs:29-41`).
- No middleware anywhere on the main router — zero `.layer()` calls
  (`src/interfaces/routes.rs:11-31`).
- Tests boot the real router via `start_app()` building an `AppState` literal
  with in-memory SQLite (`src/test/mod.rs:5-25`, `11-14`). `AppState` has three
  fields, no config (`src/app/state.rs:3-5`).
- Deployment: Dockerfile sets only `SQLX_OFFLINE`/`DATABASE_URL`
  (`Dockerfile:15,26`); fly.toml has no `[env]`; CI passes only
  `FLY_API_TOKEN` (`fly-deploy.yml`). `.env_template` has one line.
- Sibling `../api` already solved this exact problem and is our template:
  typed required `Env` struct with panicking parse helpers
  (`api/src/app/env.rs:8-28,163-174`), `infra/sentry.rs` init + hardened panic
  hook + broken-pipe filter (`api/src/infra/sentry.rs:1-49`), flag-gated init
  in `main()` holding the `ClientInitGuard` (`api/src/main.rs:23-26`), tests
  unaffected because init lives only in `main()` (`api/src/test/mod.rs:203-204`).

## Desired End State
- A typed `Env` struct loads configuration at startup, panicking fast on
  missing/invalid values: existing `DATABASE_URL` plus new required
  `SENTRY_DSN` (string) and `ENABLE_SENTRY` (strict bool).
- When enabled, a Sentry client initializes before any listener binds;
  panics reach Sentry via a hardened panic hook that filters client-disconnect
  broken-pipe noise; handled `WebError::Database`/`Template` errors are also
  captured to Sentry at response-render time.
- When disabled, behavior is identical to today except the DSN must still be
  present (fail-fast contract, mirroring api).
- Tests need no Sentry awareness: init stays confined to `main()`.
- `.env_template` documents the new vars with the api-style checklist.
- Verify: `./scripts/test.sh` green; manual run with real DSN shows panic +
  error events in Sentry; run without vars fails fast with clear message.

## Patterns to Follow
- **Sentry module**: copy the shape of `api/src/infra/sentry.rs:1-49` verbatim
  as `src/infra/sentry.rs` — `init(dsn) -> ClientInitGuard` with
  `maybe_release(sentry::release_name!()).send_default_pii(true)`
  (`sentry.rs:2-9`), panic-hook chaining via `take_hook` + `catch_unwind(
  AssertUnwindSafe(...))` to avoid double-panic abort (`sentry.rs:20-35`),
  broken-pipe short-circuit matching `"Broken pipe"` / `"os error 32"`
  (`sentry.rs:40-49`). Keep its rationale comments.
- **Env struct**: follow `api/src/app/env.rs` — private fields + `Env::init()`
  reading through `get_string_env` / `get_bool_env` helpers that panic with
  "`{key} must be set and non-empty`" / strict `"true"|"false"` parsing
  (`env.rs:163-174`). Place it at `src/app/env.rs`.
- **Init placement/gating**: `let _guard = env.enable_sentry.then(|| 
  infra::sentry::init(&env.sentry_dsn));` before state construction/listener
  binds, so the guard outlives the server (`api/src/main.rs:24-26`). Init must
  NOT live in `routes()` or `start_app()` — that's what keeps tests Sentry-free.
- **Error capture point**: inside `WebError`'s `IntoResponse`, alongside the
  existing `eprintln!` sites (`src/app/error.rs:31-38`) — call
  `sentry::capture_error(&err)` for `Database`/`Template` before returning the
  500. Keep the response body/status mapping untouched.
- **Secrets flow**: secrets never enter Dockerfile/fly.toml/workflows — they
  come from Fly machine env managed outside the repo; document in
  `.env_template` with the checklist pattern ("In .env / In .env_template /
  In fly.io dashboard / In 1Password", `api/.env_template:1-5,13,30`).
- **Dependency**: single direct dep `sentry = "0.49"` default features
  (includes sentry-panic) (`api/Cargo.toml:17`) — no sentry-tower, no custom
  feature flags.

## Patterns NOT to Follow
- **Silent env fallback** (`src/main.rs:8` style): do not extend it to the new
  vars; the whole point is fail-fast validation.
- **Panic-only capture** (api's actual behavior): deliberately deviated from —
  see Decision 2. Don't strip the `capture_error` calls to "match api".
- **Optional-with-default config sprawl**: don't add defaults like api's
  `SES_AWS_ENDPOINT_URL` path (`env.rs:117-119`); all three vars are required.

## Design Decisions
1. **Config approach**: full typed `Env` struct mirroring api, with panicking
   parse helpers (user chose Option A). `SENTRY_DSN` and `ENABLE_SENTRY` are
   required even when Sentry is disabled — matches api exactly and gives one
   uniform fail-fast contract. Exception: `DATABASE_URL` keeps today's silent
   fallback (`sqlite:data/vardy.db`) as the struct's single defaulted field,
   preserving current local/dev/deploy behavior.
2. **Error-path capture**: YES — `capture_error` in `IntoResponse` for
   Database/Template (user chose Option B). This app has no tracing, so
   `eprintln!` is the only record of 500s today; capturing them is the actual
   monitoring value. Panics also captured via the hook (both paths active).
3. **Broken-pipe filter**: ported as-is (user confirmed). Prevents
   client-disconnect noise flooding Sentry.
4. **Metadata**: mirror api exactly — `maybe_release(release_name!())`,
   `send_default_pii(true)`, no `environment`, no traces sample rate
   (user chose Option A).
5. **Deployment/docs**: no changes to Dockerfile/fly.toml/workflows; add
   `ENABLE_SENTRY=false` + `SENTRY_DSN=XXXX` to `.env_template` with the
   four-location checklist note (user confirmed).

## What We're NOT Doing
- No tracing/log facade introduction (that's a separate feature).
- No sentry-tower layer, per-request middleware, breadcrumbs, or performance
  tracing (`traces_sample_rate` stays unset → no APM).
- No `environment` tag or Fly-specific metadata (`FLY_MACHINE_ID`, git SHA).
- No change to `WebError`'s status codes, bodies, or existing tests' assertions.
- No capture of `NotFound` (it's not an error; variant is test-only anyway,
  `error.rs:10`).
- No dotenv/dotenvy runtime loading — scripts/source handles `.env` as api does.
- No changes to metrics router or port bindings.

## Open Risks
- **`DATABASE_URL` semantics shift**: moving it into a strict `Env` invites
  accidentally dropping the current silent fallback (`src/main.rs:8`) and
  breaking local/dev runs. Mitigation: keep it as the struct's single
  defaulted field; flag this diff explicitly in review.
- **Fail-fast DSN breaks existing Fly deploys** until `SENTRY_DSN` is set in
  the dashboard — deployment will crash-loop after merge if secrets aren't
  added first. Mitigation: set Fly secrets before/at deploy; note in PR.
- **Capture volume**: repeated template/database 500s now create Sentry events
  with no rate limiting or fingerprinting; a persistent DB failure could flood
  the quota. Acceptable for this app's traffic; revisit if it bites.
- **Double-reporting edge**: a panic during handler execution AND a captured
  error could both fire for one request — rare, accepted noise.
- **sentry 0.49 API drift**: module is copied from api's pinned version;
  keep versions aligned to avoid surprise breakage.
