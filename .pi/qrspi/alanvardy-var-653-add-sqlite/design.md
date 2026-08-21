# Design — Add SQLite (VAR-653)

## Current State

The repo has **no persistence layer at all**:

- `AppState` holds a single field `templates: minijinja::Environment<'static>`,
  `#[derive(Clone)]`, constructed via struct literal at both call sites
  (`src/app/state.rs:1-4`, `src/main.rs:6-8`, `src/test/mod.rs:5-18`). Both
  sites share the `app::templates::init()` factory (`src/app/templates.rs:1-11`).
- Router is generic over state: `pub fn routes() -> Router<AppState>`
  (`src/interfaces/routes.rs:6`); one route `/` → `handlers::home::web::index`
  which extracts `State(state)` and returns `Result<Html<String>, WebError>`
  (`src/interfaces/handlers/home/web.rs:7-14`).
- Error handling is minimal: two-variant `WebError { Template, NotFound }`
  with one `From<minijinja::Error>` impl and an `IntoResponse` mapping 404/500
  with no detail leakage (`src/app/error.rs:6-31`).
- No sqlx/rusqlite dependency exists in `Cargo.toml` or `Cargo.lock`.
- Tests: shared helpers `start_app()` (random port, spawned `axum::serve`) and
  `test_client()` in `src/test/mod.rs`; only dev-dependency is `reqwest`
  (`Cargo.toml:9`). Fully parallel; CI gates are clippy `-D warnings`, fmt,
  string lint (FIXME/`dbg!`), Codecov (70% project / 90% patch)
  (`.github/workflows/ci.yml:63-91`, `codecov.yml`).
- Deployment: multi-stage Dockerfile, runtime `debian:bookworm-slim`, copies
  only `/app/templates` and the binary (`Dockerfile:16-20`); fly.toml has **no
  `[mounts]`** — container filesystem is ephemeral.
- Prior art exists in `../api` git history (pre-commit `4fb273f`): sqlx 0.9
  with `["sqlite", "runtime-tokio", "chrono"]`, `SqliteConnectOptions` +
  `SqlitePoolOptions` in `src/infra/db.rs`, pool as `pub db: SqlitePool` in
  clone-state, `migrations/` SQL applied by sqlx-cli in Dockerfile stages,
  `#[sqlx::test]` per-test databases, compile-time-checked query macros,
  `.sqlx/` offline metadata for CI.

## Desired End State

SQLite persistence plumbing wired end-to-end, proven by a trivial table —
no product schema yet:

1. `Cargo.toml` gains `sqlx` with features `["sqlite", "runtime-tokio",
   "chrono", "migrate"]`.
2. `AppState` has a second field `db: SqlitePool` (Arc-backed, cheap clones),
   added at both construction sites via a shared factory.
3. A `migrations/` directory with one trivial first migration creating a
   placeholder table; migrations run via sqlx-cli in the Dockerfile build and
   runtime stages, matching the `../api` SQLite-era pattern.
4. `WebError` gains a `Database(sqlx::Error)` variant with `From` impl,
   mapped to 500 with stderr logging (mirrors the `Template` variant).
5. Integration tests use `#[sqlx::test]` (per-test temp DB, auto-migrated);
   `.sqlx/` offline metadata committed so CI compiles without a live DB.
6. Local development works with `DATABASE_URL=sqlite:data/vardy.db`;
   deploys remain ephemeral until storage is decided later.

Verification: `cargo nextest run` passes locally and in CI; the app boots
locally, creates the DB file if missing, applies migrations, and serves `/`;
clippy/fmt/string-lint gates stay green; coverage targets hold.

## Patterns to Follow

- **Shared state factory**: both production and test construct AppState via a
  common init function — extend this pattern (add `db` alongside
  `templates::init()`), mirroring how `main.rs:7` and `src/test/mod.rs:7` share
  `app::templates::init()`.
- **Pool sharing via Clone state**: store `SqlitePool` directly in
  `#[derive(Clone)]` AppState exactly as `../api` did pre-`4fb273f` — it is
  Arc-backed so per-request clones share one pool.
- **Error variant symmetry**: copy the existing `Template` variant's shape for
  the new `Database` variant — `From` impl for `?` ergonomics
  (`src/app/error.rs:14-18`) + stderr log + opaque 500 body
  (`src/app/error.rs:20-31`) + unit tests pinning status codes
  (`src/app/error.rs:33-48`).
- **Per-test DB isolation**: `#[sqlx::test]` with injected pool, as in
  `../api`'s SQLite era (e.g., `4fb273f^:src/app/sessions.rs:124`) — keeps the
  repo's fully-parallel test setup intact.
- **Offline query metadata**: commit `.sqlx/*.json` via `cargo sqlx prepare`,
  as `../api` did (`cc30e4b`), so compile-time-checked macros work in CI
  without a live database.
- **Connection options**: reuse the `../api` `init()` recipe —
  `SqliteConnectOptions::from_str(url)` with `create_if_missing(true)` and
  `foreign_keys(true)`. Consider also enabling WAL journal mode (prior art
  never set it; harmless improvement for concurrent readers).

### Patterns NOT to follow

- **In-code DDL / `CREATE TABLE IF NOT EXISTS` helpers** (`../api` `44fa2df`,
  dropped in `9f789b8`) — rejected in favor of migration files.
- **Baking a runtime-written DB into the container image** — fly.toml has no
  mounts; any deployed data would be lost. Explicitly deferred, not solved.
- **Hardcoded ports/env drift** — note `fly.toml [env] PORT = '8080'` is dead
  config vs `0.0.0.0:3000` in `src/main.rs:9`; don't add a second source of
  truth for the DB path beyond `DATABASE_URL`.

## Design Decisions

1. **Scope: infrastructure only** — one trivial placeholder table proves the
   plumbing (pool, migrations, errors, tests). Real entities come in follow-up
   tickets; nothing here should anticipate a specific schema.
2. **Deployment persistence deferred** — SQLite is local-dev-first
   (`DATABASE_URL=sqlite:data/vardy.db`, gitignored `data/`). Fly volumes are
   out of scope until a real write-path feature needs durability. Deploys keep
   working because nothing on the request path requires the DB yet.
3. **Migrations via sqlx-cli in Dockerfile stages** — matches `../api`'s
   SQLite era exactly: install
   `sqlx-cli --no-default-features --features sqlite` and run
   `sqlx migrate run` in build/runtime stages. Chosen over embedded
   `sqlx::migrate!()` for consistency with prior art, accepting the larger
   runtime stage.
4. **Error handling: dedicated `Database(sqlx::Error)` variant** — symmetric
   with `Template`, type-preserving, mapped to 500 with stderr logging and no
   client-facing detail. A generic `Internal(anyhow::Error)` was rejected as
   premature.
5. **Tests: `#[sqlx::test]` + committed `.sqlx/` metadata** — parallel-safe
   isolation per test; CI compiles macros offline. `sqlx` offline mode
   (`SQLX_OFFLINE=true`) documented for contributors.

## What We're NOT Doing

- No product schema (no users/sessions/etc.) — placeholder table only.
- No Fly volume, `[mounts]`, or any deploy persistence work.
- No new routes/handlers beyond what's needed to prove wiring (if anything:
  keep `/` untouched; prove via tests, not endpoints).
- No query-macro usage in production code yet (no queries exist); macros and
  `.sqlx/` metadata become relevant with the first real query.
- No Postgres compatibility shims, no connection-string parsing beyond
  `SqliteConnectOptions::from_str`.
- No changes to CI workflow files unless the coverage/lint gates demand it.
- No fixing the dead `PORT` env config or hardcoded bind address.

## Open Risks

- **Codecov patch target (90%)**: new error-handling code paths need unit
  tests from day one (the `NotFound` `#[allow(dead_code)]` history in
  `src/app/error.rs:6-8` shows this gate bites).
- **sqlx-cli in runtime image** adds build time and size to
  `debian:bookworm-slim`; acceptable now, revisit if deploy latency suffers.
- **`#[sqlx::test]` behavior differences** between sqlx 0.9-era (`../api`) and
  current sqlx versions — verify the attribute's env/migration expectations on
  whichever version we pin; adjust docs accordingly.
- **Windows/local-contributor friction**: `create_if_missing` plus relative
  `data/` path depends on working directory; document `DATABASE_URL` setup.
- **tokio lacks the `fs` feature** (`Cargo.toml:7`); not needed by sqlx itself,
  but any future manual file handling around the DB must add it explicitly.
