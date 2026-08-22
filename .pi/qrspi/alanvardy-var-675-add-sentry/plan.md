# Implementation Plan — Add Sentry (VAR-675)

## Overview

Add fail-fast typed configuration (`Env`), a Sentry client with a hardened panic
hook (broken-pipe filtered), and capture of handled `Database`/`Template` 500s —
ported from the sibling `api` project. Init lives only in `main()`, so tests stay
Sentry-free.

**Deploy prerequisite (from design risks):** `SENTRY_DSN` and `ENABLE_SENTRY`
must be set in the Fly dashboard **before** the post-merge deploy, or the machine
will crash-loop on the fail-fast `Env`.

---

## Phase 1: Typed `Env` with fail-fast boot contract

### Changes

#### 1. New `Env` module
**File**: `src/app/env.rs`
**Action**: create

Port `api/src/app/env.rs` shape, reduced to our three vars. `Env::init()` is
**sync** (no AWS loaders here, unlike api). `database_url` is the single
defaulted field, preserving today's silent fallback (`src/main.rs:8`).

```rust
//! Stores all the environment variables and verifies that they are available at startup
//! Set them for production with `fly secrets set KEY=VALUE`
//! Set them locally in `.env`

pub struct Env {
    pub database_url: String,
    pub sentry_dsn: String,
    pub enable_sentry: bool,
}

impl Env {
    pub fn init() -> Env {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:data/vardy.db".to_string());
        let sentry_dsn = get_string_env("SENTRY_DSN");
        let enable_sentry = get_bool_env("ENABLE_SENTRY");

        Env {
            database_url,
            sentry_dsn,
            enable_sentry,
        }
    }
}

fn get_string_env(key: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|var| !var.is_empty())
        .unwrap_or_else(|| panic!("{key} must be set and non-empty"))
}

fn get_bool_env(key: &str) -> bool {
    match get_string_env(key).as_str() {
        "true" => true,
        "false" => false,
        other => panic!("{key} must be 'true' or 'false', got '{other}'"),
    }
}
```

Add inline `#[cfg(test)] mod tests` at the bottom (project convention: happy +
sad path tests inline; mirrors api's `env.rs` test block). Use a
`static ENV_MUTEX: Mutex<()>` to serialize env-var tests under nextest, and
`unsafe { std::env::set_var/remove_var }` (edition 2024 requires unsafe for
these):

- `get_env_returns_value_when_set_and_non_empty` — set key, assert value returned
- `get_env_panics_when_var_is_empty` — `#[should_panic(expected = "must be set and non-empty")]`
- `get_env_panics_when_var_is_missing` — same expectation
- `get_bool_var_true` / `get_bool_var_false` — assert parse result
- `get_bool_var_panics_on_invalid` — `#[should_panic(expected = "must be 'true' or 'false'")]` with value `"yes"`

Use a scratch key like `TEST_GET_ENV_KEY` / `TEST_GET_BOOL_KEY` — never the real
var names (tests must not depend on or mutate host env for `SENTRY_DSN`).

#### 2. Register module
**File**: `src/app/mod.rs`
**Action**: modify — add `pub mod env;` to the module list (alphabetical: after
`pub mod db;`, before `pub mod error;`).

#### 3. Use `Env` in bootstrap
**File**: `src/main.rs`
**Action**: modify — replace the inline `DATABASE_URL` read with `Env::init()`.

```rust
// before (delete):
let database_url =
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/vardy.db".to_string());

// after:
let env = app::env::Env::init();
```

Then change `app::db::init(&database_url)` → `app::db::init(&env.database_url)`.
Everything else in `main()` (metrics, state, listeners, `try_join!`) is
untouched. `Env::init()` must be the **first** statement in `main()` so the
fail-fast panic happens before any port binds.

#### 4. Document new vars
**File**: `.env_template`
**Action**: modify — add the checklist header (api-style, `.env_template:1-5`)
and the two new entries:

```
# New entries need to be added:
# - In .env
# - In .env_template
# - In fly.io dashboard
# - In 1Password

DATABASE_URL=sqlite:data/vardy.db
ENABLE_SENTRY=false
SENTRY_DSN=XXXX
```

### Verification

#### Automated
- [x] `./scripts/test.sh` passes (format, sqlx prepare, check, clippy, tests)
- [x] New env unit tests pass: `cargo nextest run -E 'test(get_env) or test(get_bool)'` (or `cargo test env::`)

#### Manual
- [ ] `set -x SENTRY_DSN test; set -x ENABLE_SENTRY false; cargo run` boots and hosts on port 3000 (fish: use `env SENTRY_DSN=test ENABLE_SENTRY=false cargo run` if preferred)
- [ ] `env -u SENTRY_DSN cargo run` panics with `SENTRY_DSN must be set and non-empty` **before** the "Hosting on http://localhost:3000" line (no port bound)
- [ ] `env SENTRY_DSN=x ENABLE_SENTRY=maybe cargo run` panics with `ENABLE_SENTRY must be 'true' or 'false'`
- ~~[ ] With `DATABASE_URL` unset, app still boots against `sqlite:data/vardy.db` (fallback preserved)~~ — **dropped (deviation, user-approved)**: main gained an Unsplash feature whose own `Env` made `DATABASE_URL` required (fail-fast). Rebase merged both; `DATABASE_URL` now panics if unset, matching Option A.

---

## Phase 2: Sentry client init + panic hook, flag-gated in `main()`

### Changes

#### 1. Dependency
**File**: `Cargo.toml`
**Action**: modify — add to `[dependencies]` (alphabetical, between `serde_json`/`sha2` and `sqlx`):

```toml
sentry = "0.49"
```

Default features only — includes `sentry-panic`. No `sentry-tower`, no custom
features. Match api's pinned major (`api/Cargo.toml:17`).

#### 2. New sentry module
**File**: `src/infra/sentry.rs`
**Action**: create — copy `api/src/infra/sentry.rs` **verbatim** (49 lines,
including all rationale comments). Key content:

```rust
pub fn init(sentry_dsn: &str) -> sentry::ClientInitGuard {
    let guard = sentry::init((
        sentry_dsn,
        sentry::ClientOptions::default()
            .maybe_release(sentry::release_name!())
            // Capture user IPs and potentially sensitive headers when using HTTP server integrations
            // see https://docs.sentry.io/platforms/rust/data-management/data-collected for more info
            .send_default_pii(true),
    ));

    let sentry_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), "{info}");

        // Don't forward broken-pipe panics to Sentry — they're noise.
        if is_broken_pipe(info) {
            return;
        }

        // If sentry's hook panics while writing to stderr we don't want to
        // double-panic and abort.
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            sentry_hook(info);
        }));
    }));

    guard
}

/// Returns `true` if the panic was caused by a broken pipe on stderr/stdout.
fn is_broken_pipe(info: &std::panic::PanicHookInfo<'_>) -> bool {
    let msg = info
        .payload()
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()));

    msg.is_some_and(|m| m.contains("Broken pipe") || m.contains("os error 32"))
}
```

#### 3. Register module
**File**: `src/infra/mod.rs`
**Action**: modify — add `pub mod sentry;` (alphabetical: after `pub mod metrics;`).

#### 4. Flag-gated init in `main()`
**File**: `src/main.rs`
**Action**: modify — insert after `let env = app::env::Env::init();` and
**before** state construction / listener binds:

```rust
let _guard = env
    .enable_sentry
    .then(|| infra::sentry::init(&env.sentry_dsn));
```

The binding lives in `main()`'s scope so the `ClientInitGuard` outlives the
server. Do **not** init in `routes()` or `src/test/mod.rs` — that confinement is
what keeps tests Sentry-free. No changes to `start_app()` / test helpers.

### Verification

#### Automated
- [x] `./scripts/test.sh` passes
- [x] `cargo tree -i sentry` resolves to 0.49.x with `sentry-panic` present

#### Manual
- [x] With real DSN in `.env`, `ENABLE_SENTRY=true`: run app, temporarily add `panic!("sentry test")` in a handler, curl that route, confirm the panic event appears in the Sentry dashboard (with release name set); revert the panic — **confirmed by user**
- [ ] With `ENABLE_SENTRY=false`: same panic does **not** appear in Sentry (no client initialized)
- [ ] Broken-pipe filter: kill the process's stderr consumer (e.g. pipe to a closed `head`) and trigger a broken-pipe panic — no Sentry event, process exits normally

---

## Phase 3: Capture handled `Database`/`Template` errors to Sentry

### Changes

#### 1. Capture in `IntoResponse`
**File**: `src/app/error.rs`
**Action**: modify — add `sentry::capture_error(&err);` alongside the two
existing `eprintln!` arms. No signature, status, or body changes.

```rust
WebError::Database(err) => {
    eprintln!("database error: {err}");
    sentry::capture_error(&err);
    (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
}
WebError::Template(err) => {
    eprintln!("template render error: {err}");
    sentry::capture_error(&err);
    (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
}
```

Note: `sentry::capture_error` is a safe no-op when no client is initialized, so
calling it unconditionally is correct — tests (Sentry off) are unaffected.
`NotFound` is deliberately not captured (not an error; test-only variant).

No new tests: the existing four tests in `error.rs` already assert status +
conversion behavior and remain the verification (project rule: tests validate
status **and** body — the existing tests assert status; the 500 arms return the
constant `"internal server error"` body which is unchanged and covered by
integration tests).

### Verification

#### Automated
- [ ] `./scripts/test.sh` passes — existing `error.rs` tests (`template_error_is_500`, `database_error_is_500`, etc.) assert unchanged status mapping

#### Manual
- [ ] With Sentry enabled, force a 500 (e.g. temporarily rename a template `init()` loads or break a query), hit the endpoint, confirm the captured `minijinja::Error`/`sqlx::Error` event appears in Sentry; response body is still plain-text `internal server error` with status 500; revert the breakage

---

## Testing Checkpoints (end-to-end)

- [ ] After Phase 1: `./scripts/test.sh` green; boots with all three vars; panics fast on missing/invalid `SENTRY_DSN`/`ENABLE_SENTRY`; `DATABASE_URL` still falls back silently
- [ ] After Phase 2: tests green and Sentry-free; real DSN + flag on delivers panic events; flag off = today's behavior
- [ ] After Phase 3: error tests assert identical 500 responses; manual 500 with Sentry on produces a captured error event
- [ ] Deploy: `SENTRY_DSN`/`ENABLE_SENTRY` set in Fly dashboard (and 1Password) before post-merge deploy

## Notes / Deviations from structure outline

- **`src/app/mod.rs` registration** (Phase 1.2) and **`src/infra/mod.rs` registration** (Phase 2.3) are implied by the new files but listed explicitly here.
- **Inline unit tests for `get_string_env`/`get_bool_env`** (Phase 1.1) are not named in structure.md but are mandated by project convention ("happy and sad path tests inline") and mirror the api template's test block.
- No schema migrations, no route changes, no `ROUTES.md` updates — this feature touches none of those surfaces.
