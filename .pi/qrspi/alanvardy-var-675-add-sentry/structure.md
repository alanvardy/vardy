# Structure Outline — Add Sentry (VAR-675)

## Approach
Port the sibling `api` project's proven pattern: a typed fail-fast `Env` struct, a
`src/infra/sentry.rs` module (init + hardened panic hook + broken-pipe filter), and
flag-gated init in `main()` holding the `ClientInitGuard` — plus a deliberate
extension api doesn't have: `capture_error` for handled `Database`/`Template` 500s.
Tests stay Sentry-free because init lives only in `main()`.

---

## Phase 1: Typed `Env` with fail-fast boot contract

Replaces the silent `DATABASE_URL` read in `main()` with a typed struct. Adds
required `SENTRY_DSN` and `ENABLE_SENTRY`; `DATABASE_URL` keeps today's
`sqlite:data/vardy.db` silent fallback as the struct's single defaulted field.
End-to-end: config → startup → observable boot behavior.

**Files**: `src/app/env.rs` (new), `src/main.rs`, `.env_template`
**Key changes**:
- `Env::init() -> Env` — panics via helpers on missing/invalid values
- `Env { database_url: String, sentry_dsn: String, enable_sentry: bool }`
- `get_string_env(key: &str) -> String` / `get_bool_env(key: &str) -> bool` — private helpers, panic with `"{key} must be set and non-empty"` / strict `"true"|"false"`
- `main()` becomes `let env = app::env::Env::init();` then uses `env.database_url`
- `.env_template` gains `ENABLE_SENTRY=false` + `SENTRY_DSN=XXXX` with the four-location checklist note

**Verify**: `./scripts/test.sh` passes (tests don't touch `Env`); manual: `DATABASE_URL=... SENTRY_DSN=x ENABLE_SENTRY=false cargo run` boots; omitting `SENTRY_DSN` panics with a clear message before any port binds.

---

## Phase 2: Sentry client init + panic hook, flag-gated in `main()`

Adds the `sentry` dep and the ported `infra::sentry` module; initializes the client
before any listener binds when `ENABLE_SENTRY=true`. Panics now reach Sentry;
broken-pipe noise is filtered. End-to-end: dep → module → main() gate → real
Sentry events.

**Files**: `Cargo.toml`, `src/infra/sentry.rs` (new) + `src/infra/mod.rs`, `src/main.rs`
**Key changes**:
- `sentry = "0.49"` default features (includes sentry-panic) — match api's pinned version
- `infra::sentry::init(dsn: &str) -> ClientInitGuard` — `maybe_release(release_name!())`, `send_default_pii(true)`
- `fn is_broken_pipe(payload: &(dyn Any + Send)) -> bool` — matches `"Broken pipe"` / `"os error 32"`
- Panic hook chaining via `take_hook()` + `catch_unwind(AssertUnwindSafe(...))`
- In `main()`: `let _guard = env.enable_sentry.then(|| infra::sentry::init(&env.sentry_dsn));` before state construction — guard outlives the server

**Verify**: `./scripts/test.sh` passes (tests remain Sentry-free — init confined to `main()`); manual: run with a real DSN and `ENABLE_SENTRY=true`, trigger a panic (e.g. temporarily panic in a handler), confirm the event appears in the Sentry dashboard; run with `ENABLE_SENTRY=false` and confirm no client initializes.

---

## Phase 3: Capture handled `Database`/`Template` errors to Sentry

Extends `WebError::IntoResponse` so the two existing `eprintln!` 500-paths also
call `sentry::capture_error` before returning the response. Status codes, bodies,
and existing test assertions untouched. End-to-end: handler `?` → `From` conversion
→ capture at response-render time → Sentry event.

**Files**: `src/app/error.rs`
**Key changes**:
- In `IntoResponse for WebError`: for `Database(err)` and `Template(err)`, add `sentry::capture_error(&err);` alongside the existing `eprintln!` — no signature changes, no response changes

**Verify**: `./scripts/test.sh` passes (existing error tests assert unchanged status + body); manual: with Sentry enabled, hit an endpoint that produces a 500 (e.g. break a template or force a DB failure), confirm the captured error with its message appears in Sentry.

---

## Testing Checkpoints
- **After Phase 1**: `./scripts/test.sh` green; app boots with all three vars set; panics fast with a clear message when `SENTRY_DSN` or `ENABLE_SENTRY` is missing/invalid; `DATABASE_URL` still falls back silently.
- **After Phase 2**: tests still green and Sentry-free; manual run with real DSN + flag on delivers panic events to Sentry; flag off = today's behavior.
- **After Phase 3**: error tests assert identical 500 responses; manual 500 with Sentry on produces a captured error event.
- **Deploy note (from design risks)**: set `SENTRY_DSN`/`ENABLE_SENTRY` in the Fly dashboard **before** the post-merge deploy, or the machine will crash-loop on the fail-fast `Env`.

## Slicing Note
These slices are vertical in the infra sense: each crosses config → runtime →
observable behavior and is independently verifiable. There is no UI/DB surface in
this feature, so no slice touches handlers or stores — Phase 3's "endpoint" layer
is the existing `IntoResponse` path.
