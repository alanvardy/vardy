# Implementation Plan — Add SQLite (VAR-653)

## Overview

Wire SQLite into the app end-to-end as infrastructure only: a `SqlitePool` in
`AppState` built by a shared `db::init()` factory, a `Database` error variant
symmetric with `Template`, one trivial placeholder-table migration proven by a
`#[sqlx::test]` test, sqlx-cli migration steps in the Dockerfile, and local
`DATABASE_URL` contributor docs. No product schema, no new routes, no Fly
volumes.

**Pinned version**: `sqlx = "0.9.0"` — the current crates.io release and the
exact version of the `../api` SQLite-era prior art, so the design's
`#[sqlx::test]` version-drift risk does not apply.

---

## Phase 1: Pool in AppState

### Changes

#### 1. Add the sqlx dependency
**File**: `Cargo.toml`
**Action**: modify

Add to `[dependencies]` (keep alphabetical-ish grouping; `Cargo.lock` updates
on next cargo command and must be committed because CI clippy uses `--locked`):

```toml
sqlx = { version = "0.9.0", features = ["sqlite", "runtime-tokio", "chrono", "migrate"] }
```

The `migrate` feature is required by `#[sqlx::test]` (Phase 3); `chrono` is in
the design's dependency list for the first real query later.

#### 2. New DB factory module
**File**: `src/app/db.rs`
**Action**: create

Mirrors `../api` at `4fb273f^:src/infra/db.rs`, plus WAL journal mode (design:
"harmless improvement for concurrent readers"). `create_if_missing(true)` is
what makes `sqlite:data/vardy.db` boot a fresh checkout.

```rust
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use std::str::FromStr;

pub async fn init(database_url: &str) -> SqlitePool {
    let options = SqliteConnectOptions::from_str(database_url)
        .expect("invalid DATABASE_URL")
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal);

    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .expect("failed to connect to database")
}
```

(`.expect` over `.unwrap` for message clarity; both fine — pick whichever
clippy prefers. No `?` because `main` uses `Box<dyn Error>` and the factory
mirrors prior art.)

#### 3. Register the module
**File**: `src/app/mod.rs`
**Action**: modify

```rust
pub mod db;
```

#### 4. Add the `db` field
**File**: `src/app/state.rs`
**Action**: modify

```rust
#[derive(Clone)]
pub struct AppState {
    pub templates: minijinja::Environment<'static>,
    pub db: sqlx::SqlitePool,
}
```

`SqlitePool` is internally `Arc`-backed, so per-request clones of
`#[derive(Clone)]` state share one pool (prior-art pattern).

#### 5. Production construction site
**File**: `src/main.rs`
**Action**: modify

Read `DATABASE_URL` with the default from the design; keep everything else in
`main` untouched:

```rust
let database_url = std::env::var("DATABASE_URL")
    .unwrap_or_else(|_| "sqlite:data/vardy.db".to_string());
let state = app::state::AppState {
    templates: app::templates::init(),
    db: app::db::init(&database_url).await,
};
```

#### 6. Test construction site
**File**: `src/test/mod.rs`
**Action**: modify

Use an in-memory URL so parallel tests never share or create files on disk.
Note for future readers: each pool connection to `sqlite::memory:` is a
separate database; that's irrelevant now (no handler queries the DB) and
real DB-backed tests will use `#[sqlx::test]` per-test pools (Phase 3), not
`start_app()`.

```rust
pub async fn start_app() -> SocketAddr {
    let state = crate::app::state::AppState {
        templates: crate::app::templates::init(),
        db: crate::app::db::init("sqlite::memory:").await,
    };
    // ... rest unchanged
}
```

`start_app()` becomes `async`-awaiting `init` — it is already `async`, so no
signature change; only the body gains the `.await`.

#### 7. Ignore the local database directory
**File**: `.gitignore`
**Action**: modify

```
/target
/data
```

### Verification

#### Automated
- [x] `cargo nextest run` — all existing tests green with **zero test-code
      changes** (proves state plumbing compiles and both construction sites
      work)
- [x] `cargo clippy --all-targets --all-features --locked -- -D warnings` —
      clean (also confirms `Cargo.lock` is committed)
- [x] `cargo fmt --all -- --check` — clean
- [x] `./scripts/lint_string.sh "FIXME "` (and the other four invocations
      from `ci.yml`) — no occurrences

#### Manual
- [ ] `DATABASE_URL=sqlite:data/vardy.db cargo run` — app boots, prints
      "Hosting on http://localhost:3000", and `data/vardy.db` (plus
      `-wal`/`-shm` WAL sidecars) is created
- [ ] `curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/` → `200`

---

## Phase 2: Database error variant

### Changes

#### 1. `WebError::Database` variant, `From` impl, response mapping, tests
**File**: `src/app/error.rs`
**Action**: modify

Keep the `#[allow(dead_code)]`: after this change `NotFound` is still only
constructed from tests (the `Database` variant is constructed via the `From`
impl — trait impls are exempt from the dead-code lint, but `NotFound` is not),
and dropping the allow would fail CI clippy `-D warnings`.

```rust
#[allow(dead_code)]
pub enum WebError {
    Template(minijinja::Error),
    Database(sqlx::Error),
    NotFound,
}

impl From<sqlx::Error> for WebError {
    fn from(err: sqlx::Error) -> Self {
        WebError::Database(err)
    }
}
```

Extend `IntoResponse` (stderr log + opaque 500, mirroring the `Template` arm;
no error detail leaks to the client):

```rust
WebError::Database(err) => {
    eprintln!("database error: {err}");
    (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
}
```

Add unit tests to the existing `#[cfg(test)] mod tests` (mirroring
`template_error_is_500`); `sqlx::Error::RowNotFound` is a unit variant, so no
live DB is needed:

```rust
#[test]
fn database_error_is_500() {
    let res = WebError::from(sqlx::Error::RowNotFound).into_response();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn sqlx_error_converts_via_from() {
    let err: WebError = sqlx::Error::RowNotFound.into();
    assert!(matches!(err, WebError::Database(_)));
}
```

### Adaptation (Phase 1)

`create_if_missing(true)` creates the database **file** but not its parent
**directory**, so `sqlite:data/vardy.db` panicked on a fresh checkout
("unable to open database file"). `db::init` now creates the parent directory
(from `SqliteConnectOptions::get_filename()`) before connecting, skipping
`:memory:`. Covered by a new `db::tests::init_creates_database_file_and_parent_directory`
test. The `db` field also carries `#[allow(dead_code)]` (same precedent as
`WebError::NotFound`) until the first handler query reads it.

### Verification

#### Automated
- [ ] `cargo nextest run` — the two new `error.rs` unit tests pass alongside
      all others (keeps Codecov patch coverage ≥ 90%; the 500-mapping branch
      is exercised, the `eprintln!` line is covered by running the test)
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings` —
      clean (confirms keeping `#[allow(dead_code)]` was correct; remove it
      only if clippy proves the lint no longer fires)
- [ ] `cargo fmt --all -- --check` — clean

#### Manual
- [ ] None beyond the automated gates (no behavior change on the request path
      yet; `/` still serves 200 per Phase 1 manual check)

---

## Phase 3: First migration, proven by test

**Decision (per structure.md's open point)**: pool-only test — no
`start_app_with_db` variant. No handler touches the DB, so a full HTTP-stack
test would prove nothing extra; `#[sqlx::test]` already proves migration file
→ injected pool → query end-to-end.

### Changes

#### 1. The migration
**File**: `migrations/0001_placeholder.sql`
**Action**: create

```sql
CREATE TABLE IF NOT EXISTS placeholder (
    id INTEGER PRIMARY KEY AUTOINCREMENT
);
```

Filename follows sqlx's `<version>_<description>.sql` convention. `IF NOT
EXISTS` is belt-and-braces; sqlx tracks applied migrations in `_sqlx_migrations`.

#### 2. Migration test
**File**: `src/app/db.rs`
**Action**: modify (append inline test module)

`#[sqlx::test]` (sqlx 0.9) creates a per-test temp SQLite database, injects
the pool as the first argument, and auto-applies `migrations/` from the crate
root — fully parallel-safe, no changes to `.config/nextest.toml`. The
attribute requires the `migrate` feature (added in Phase 1).

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    #[sqlx::test]
    async fn migrations_applied(pool: SqlitePool) {
        let row = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'placeholder'",
        )
        .fetch_one(&pool)
        .await
        .expect("placeholder table should exist after migrations");
        assert_eq!(row.get::<String, _>("name"), "placeholder");
    }
}
```

If `fetch_one` errors because the table is missing, the `.expect` message
makes the failure self-explanatory.

### Verification

#### Automated
- [ ] `cargo nextest run` — `app::db::tests::migrations_applied` passes and
      the suite remains fully parallel (no serial gating introduced)
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings` —
      clean
- [ ] `cargo fmt --all -- --check` — clean

#### Manual
- [ ] The app itself never runs migrations at runtime (sqlx-cli does, in
      Docker / locally), so verify the file directly: with sqlx-cli installed
      (`cargo install sqlx-cli --no-default-features --features sqlite`), run
      `DATABASE_URL=sqlite:data/vardy.db sqlx migrate run`, then
      `sqlite3 data/vardy.db .tables` lists `placeholder` and
      `_sqlx_migrations`

---

## Phase 4: Deployment + contributor docs

### Changes

#### 1. Dockerfile: sqlx-cli + migrations in build and runtime stages
**File**: `Dockerfile`
**Action**: modify

Mirrors `../api` pre-`4fb273f` exactly (sqlx-cli installed in builder, copied
to runtime, migrations applied against a baked DB at image-build time).
`SQLX_OFFLINE=true` is set before `cargo build` — no query macros exist yet,
so it is inert today but makes the build correct the moment the first
`query!` lands (and `.sqlx/` metadata is committed).

```dockerfile
FROM chef AS builder
RUN cargo install sqlx-cli --no-default-features --features sqlite
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin vardy

# We do not need the Rust toolchain to run the binary!
FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
COPY --from=builder /app/migrations ./migrations
COPY --from=builder /app/templates ./templates
COPY --from=builder /app/target/release/vardy /usr/local/bin
ENV DATABASE_URL=sqlite:data/vardy.db
RUN mkdir -p data
RUN sqlx database create
RUN sqlx migrate run
ENTRYPOINT ["/usr/local/bin/vardy"]
```

Notes:
- `sqlx database create` is required because `sqlx migrate run` won't create
  the SQLite file itself (the app's `create_if_missing` doesn't apply to the
  CLI).
- The baked `data/vardy.db` is throwaway (ephemeral deploys, no mounts) — it
  just guarantees a migrated schema exists at boot, matching prior art.
- `migrations/` is copied into the runtime image so a future entrypoint could
  re-run `sqlx migrate run` at container start without a rebuild.
- `.dockerignore` only excludes `fly.toml` and `.git/`, so `migrations/`
  reaches the builder untouched.

#### 2. Contributor env template
**File**: `.env_template`
**Action**: create

```
DATABASE_URL=sqlite:data/vardy.db
```

#### 3. Contributor docs
**File**: `README.md`
**Action**: create

Short development section (repo has no README today):

```markdown
# vardy

## Development

cp .env_template .env   # or export the vars manually

- `DATABASE_URL` defaults to `sqlite:data/vardy.db` (the `data/` directory is
  created on first boot and gitignored).
- Run migrations locally: `sqlx migrate run` (requires
  `cargo install sqlx-cli --no-default-features --features sqlite`).
- Tests: `cargo nextest run`. Tests use `#[sqlx::test]`, which provisions a
  temporary per-test database and applies `migrations/` automatically.
- Compile-time-checked query macros (`query!` etc.) need either a reachable
  `DATABASE_URL` or committed offline metadata: set `SQLX_OFFLINE=true` and
  refresh metadata with `cargo sqlx prepare` after schema changes.
```

No CI workflow changes: `#[sqlx::test]` needs no live DB service, and there
are no query macros yet, so `SQLX_OFFLINE` is unnecessary in CI today.

### Verification

#### Automated
- [ ] `docker build -t vardy .` — succeeds (sqlx-cli install + `sqlx migrate
      run` in both stages complete; this is the slow first build, later runs
      hit the cargo-chef cache layer)
- [ ] `docker run --rm -p 3000:3000 vardy` in background, then
      `curl -s -o /dev/null -w "%{http_code}" http://localhost:3000/` → `200`
- [ ] Full CI gate set locally:
      `cargo nextest run --profile ci`, `cargo fmt --all -- --check`,
      `cargo clippy --all-targets --all-features --locked -- -D warnings`,
      `./scripts/lint_string.sh "FIXME "` / `"FIXME:"` / `"fixme "` /
      `"fixme:"` / `"dbg!"` — all green (CI `ci.yml` should match on the PR)

#### Manual
- [ ] `docker run --rm -it vardy ls data` shows the migrated DB file baked at
      build time (confirms migrations ran in the runtime stage)
- [ ] `.env_template` and README instructions are copy-pasteable on a fresh
      clone: `cp .env_template .env`, `cargo run`, `/` serves 200

---

## Testing Checkpoints (resume points)

1. **After Phase 1**: `cargo nextest run` green, zero test changes; app boots
   and creates `data/vardy.db`; `/` → 200; clippy/fmt clean.
2. **After Phase 2**: two new `error.rs` tests pass; coverage patch target
   still ≥ 90% (checked on merge to main via Codecov).
3. **After Phase 3**: `app::db::tests::migrations_applied` passes; suite
   fully parallel; `sqlite3 data/vardy.db .tables` shows `placeholder` after
   a local `sqlx migrate run`.
4. **After Phase 4**: `docker build` succeeds; all CI gates green on the PR.

## Risk Notes

- **sqlx version**: pinned 0.9.0 = prior art version, so `#[sqlx::test]`
  semantics (auto-migration of crate-root `migrations/`, pool injection) are
  known-good. If the attribute fails unexpectedly, fall back to a manual
  `SqlitePoolOptions` + `sqlx::migrate!("./migrations").run(&pool)` test in
  `src/app/db.rs` — same assertion, no attribute.
- **sqlx-cli install time** in the Docker builder is significant (~minutes)
  on cache misses; accepted per design decision 3. If the runtime image size
  from copying the `sqlx` binary matters later, migrations can move to
  container start or `sqlx::migrate!()` in a follow-up.
