# Structure Outline — Add SQLite (VAR-653)

## Approach

Infrastructure-only vertical wiring of SQLite into the existing app, following
the `../api` SQLite-era pattern: a `SqlitePool` in `AppState` via a shared
`db::init()` factory, a `Database` error variant mirroring `Template`, one
trivial migration proven by `#[sqlx::test]`, and sqlx-cli migrations in the
Dockerfile. No product schema, no new routes, no Fly volumes.

---

## Phase 1: Pool in AppState

App boots with a `SqlitePool` in state, created from `DATABASE_URL` (default
`sqlite:data/vardy.db`), shared by production and test construction sites.
Existing behavior unchanged — this proves state plumbing end-to-end.

**Files**: `Cargo.toml`, `Cargo.lock`, `src/app/db.rs` (new), `src/app/state.rs`,
`src/app/mod.rs`, `src/main.rs`, `src/test/mod.rs`, `.gitignore`

**Key changes**:
- `sqlx = { version = "?", features = ["sqlite", "runtime-tokio", "chrono", "migrate"] }` — new dependency (pin current sqlx; design flags 0.9-era prior art)
- `pub async fn init(database_url: &str) -> SqlitePool` — new; `SqliteConnectOptions::from_str` + `create_if_missing(true)` + `foreign_keys(true)` (+ WAL), `SqlitePoolOptions`
- `AppState { templates, db: SqlitePool }` — modified; both literals become `db: app::db::init(...).await`
- `.gitignore` gains `data/`

**Verify**: `cargo nextest run` (existing tests green); `clippy -D warnings` and
`fmt --check` clean. Manually: `DATABASE_URL=sqlite:data/vardy.db cargo run`,
confirm `data/vardy.db` is created and `/` still renders 200.

---

## Phase 2: Database error variant

`WebError` can represent DB failures, so any future handler can `?` on sqlx
calls. Symmetric with the existing `Template` variant: stderr log, opaque 500.

**Files**: `src/app/error.rs`

**Key changes**:
- `enum WebError { Template(minijinja::Error), Database(sqlx::Error), NotFound }` — modified (drop the `NotFound` `#[allow(dead_code)]` if coverage allows keeping it)
- `impl From<sqlx::Error> for WebError` — new, for `?` ergonomics
- `IntoResponse`: `Database(err)` → `eprintln!` + 500 `"internal server error"` — modified match arm

**Verify**: `cargo nextest run` — new unit tests in `error.rs` pin `Database` →
500 and the `From` conversion (mirrors existing `Template` tests at
`src/app/error.rs:33-48`); Codecov patch coverage (90%) stays satisfied.

---

## Phase 3: First migration, proven by test

One trivial placeholder table (`migrations/0001_...sql`), with an integration
test that runs the app against a `#[sqlx::test]`-injected pool and asserts the
table exists. This is the end-to-end proof: migration file → pool → query.

**Files**: `migrations/0001_placeholder.sql` (new), `src/test/mod.rs` or a new
inline `#[cfg(test)]` test module

**Key changes**:
- `CREATE TABLE IF NOT EXISTS placeholder (id INTEGER PRIMARY KEY AUTOINCREMENT)` — new migration (exact schema TBD in plan)
- `#[sqlx::test] async fn test_migrations_applied(pool: SqlitePool)` — new test; queries `sqlite_master` for the table
- Possibly a `start_app_with_db(pool)` variant of `start_app()` if the test needs the full HTTP stack — decide in plan; pool-only test is acceptable since no handler touches the DB yet

**Verify**: `cargo nextest run` — migration test passes alongside all others
(fully parallel, per-test temp DBs). Manually: boot from Phase 1 and check the
placeholder table exists in `data/vardy.db` (`sqlite3 data/vardy.db .tables`).

---

## Phase 4: Deployment + contributor docs

Migrations run in the Docker build/runtime stages via sqlx-cli, and local
`DATABASE_URL` setup is documented. Deploys stay ephemeral by design.

**Files**: `Dockerfile`, `README.md` (or `.env_template`), possibly `docs` note
on `SQLX_OFFLINE=true`

**Key changes**:
- Dockerfile build & runtime stages: `cargo install sqlx-cli --no-default-features --features sqlite` + `sqlx migrate run` — modified, mirroring `../api` pre-`4fb273f`
- Copy `migrations/` into the runtime image alongside `templates/`
- Docs: `DATABASE_URL=sqlite:data/vardy.db` setup, `SQLX_OFFLINE` note

**Verify**: `docker build -t vardy .` succeeds; optionally `docker run -p 3000:3000 vardy`
and curl `/` for 200 (migrations run at build; app must still boot). CI gates
(`ci.yml`) green on the PR.

---

## Testing Checkpoints

After each phase, this should be true — use to resume after a context reset:

1. **Pool in state**: `cargo nextest run` green with zero test changes; app
   boots and creates `data/vardy.db`; `/` serves 200; clippy/fmt clean.
2. **Error variant**: new `error.rs` unit tests pass pinning `Database` → 500;
   no coverage regression (patch ≥ 90%).
3. **Migration**: a `#[sqlx::test]` test asserts the placeholder table exists;
   all tests still fully parallel; `sqlite3 data/vardy.db .tables` shows it.
4. **Deploy**: `docker build` succeeds with sqlx-cli migration stages; CI
   workflow (tests, coverage, fmt, clippy, string lint) fully green.

Note: Phases 1–3 are pure local/Rust work; only Phase 4 touches deployment.
If Phase 3's `#[sqlx::test]` behaves differently on the pinned sqlx version
than the 0.9-era prior art (open risk in design), resolve it there before
Phase 4.
