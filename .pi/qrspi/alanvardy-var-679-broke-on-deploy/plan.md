# Implementation Plan

## Overview
Deploys become safe: the app migrates its own database at startup via embedded migrations (no schema baked into image layers), `/health` proves real data-layer liveness through a pooled `SELECT 1` surfaced via `WebError`, and the Fly deploy workflow fires only on successful CI completion on `main`. `.env` is excluded from Docker build contexts. Storage stays ephemeral by design.

---

## Phase 1: Self-migrating app

### Changes

#### 1. Add `db::migrate` function
**File**: `src/app/db.rs`
**Action**: modify — add below `init()`:

```rust
/// Apply embedded migrations. Idempotent: `_sqlx_migrations` bookkeeping
/// makes repeat calls no-ops, so every boot converges to the current schema.
pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
```

No new imports needed (`SqlitePool` already imported). The macro embeds `migrations/` into the binary at compile time, so the running container no longer depends on image-baked schema state.

#### 2. Inline tests for `db::migrate`
**File**: `src/app/db.rs`
**Action**: modify — extend the existing `#[cfg(test)] mod tests`:

```rust
#[tokio::test]
async fn migrate_creates_schema_on_empty_database() {
    let dir = std::env::temp_dir().join(format!("vardy-migrate-test-{}", std::process::id()));
    let url = format!("sqlite:{}/db.sqlite", dir.display());
    let pool = init(&url).await;
    migrate(&pool).await.expect("migration should succeed");

    // Happy path: a table from each meaningful migration exists.
    for table in ["placeholder", "dumps", "unsplash_pictures"] {
        let count: i64 =
            sqlx::query_scalar(&format!("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='{table}'"))
                .fetch_one(&pool)
                .await
                .expect("query");
        assert_eq!(count, 1, "table {table} should exist");
    }
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

#[tokio::test]
async fn migrate_is_idempotent() {
    let dir = std::env::temp_dir().join(format!("vardy-migrate-idem-test-{}", std::process::id()));
    let url = format!("sqlite:{}/db.sqlite", dir.display());
    let pool = init(&url).await;
    migrate(&pool).await.expect("first migrate");
    migrate(&pool).await.expect("second migrate is a no-op");
    std::fs::remove_dir_all(&dir).expect("cleanup");
}
```

Note: the existing `migrations_applied` test (`#[sqlx::test]`) stays as-is — it covers `#[sqlx::test]`'s own auto-migration behavior.

#### 3. Call migrate in `main()`
**File**: `src/main.rs`
**Action**: modify — bind the pool to a local so `migrate` can borrow it before the pool moves into `AppState`. Failure propagates via `?` (exit code 1, pre-bind, matching the metrics-init pattern):

```rust
let metrics = Arc::new(infra::metrics::AppMetrics::new()?);
let db = app::db::init(&env.database_url).await;
app::db::migrate(&db).await?;
info!("Database migrated");
let state = app::state::AppState {
    templates: app::templates::init(),
    db,
    metrics: metrics.clone(),
    env: Arc::new(env),
    unsplash_base_url: UNSPLASH_BASE_URL.into(),
};
```

(Only the pool construction lines change; everything else in `AppState` stays.) A failed migration exits non-zero before either listener binds, so Fly crash-loops while the old release keeps serving.

#### 4. Remove build-time migration from Dockerfile
**File**: `Dockerfile`
**Action**: modify — delete these three lines from the `runtime` stage (keep `ENV DATABASE_URL=sqlite:data/vardy.db`; the app reads it at startup):

```dockerfile
RUN mkdir -p data          # DELETE
RUN sqlx database create   # DELETE
RUN sqlx migrate run       # DELETE
```

Keep everything else: `sqlx-cli` stays in the runtime image (deliberate, per design decision #2 — manual inspection via `sqlx migrate info`), and `COPY --from=builder /app/migrations ./migrations` stays so remote inspection sees the same SQL. Parent-dir creation for `data/vardy.db` is already handled by `db::init`'s `create_dir_all`.

### Verification

#### Automated
- [x] `./scripts/test.sh` passes (fmt, sqlx prepare refresh, type-check, clippy, tests, TODO grep)
- [x] `cargo test --lib db::tests` green — includes new happy-path and idempotency tests

#### Manual
- [ ] `rm -rf data && cargo run` boots cleanly with zero prior schema; log line "Database migrated" appears; `GET http://localhost:3000/health` returns 200
- [ ] Second consecutive `cargo run` also boots (idempotent migration against an already-migrated DB)

---

## Phase 2: Real `/health` check

### Changes

#### 1. Replace constant closure with DB-pinging handler
**File**: `src/interfaces/routes.rs`
**Action**: modify — three small edits:

(a) Extend imports:

```rust
use axum::{
    Router,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    routing::get,
};

use crate::app::error::WebError;
```

(b) Add a private handler above `routes()`:

```rust
/// Prove data-layer liveness: acquire from the pool and run a trivial query.
/// Any failure flows through `WebError::Database` → logged + Sentry-captured 500.
async fn health(State(state): State<AppState>) -> Result<StatusCode, WebError> {
    sqlx::query("SELECT 1").execute(&state.db).await?;
    Ok(StatusCode::OK)
}
```

(c) Swap the route registration:

```rust
.route("/health", get(|| async { StatusCode::OK }))  // DELETE this line
.route("/health", get(health))                        // REPLACE with
```

#### 2. Update tests
**File**: `src/interfaces/routes.rs`
**Action**: modify the `health_returns_200` test (happy) and add a sad test. Both assert status **and** body (project convention):

```rust
#[tokio::test]
async fn health_returns_200() {
    let (addr, _pool) = crate::test::start_app_with("https://api.unsplash.com").await;
    let client = test_client();
    let res = client
        .get(format!("http://{addr}/health"))
        .send()
        .await
        .expect("request to /health should succeed");
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), ""); // bare StatusCode body
}

#[tokio::test]
async fn health_returns_500_when_database_is_dead() {
    let (addr, pool) = crate::test::start_app_with("https://api.unsplash.com").await;
    pool.close().await; // kill the data layer behind the running server
    let client = test_client();
    let res = client
        .get(format!("http://{addr}/health"))
        .send()
        .await
        .expect("request to /health should complete");
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(res.text().await.unwrap(), "internal server error");
}
```

The existing test currently uses `start_app()`; switching to `start_app_with(...)` gives the test ownership of the pool so it can be closed for the sad case. `WebError::Database`'s `IntoResponse` arm already produces exactly `"internal server error"` with logging + Sentry capture (`src/app/error.rs:31-36`) — no error-code changes needed.

#### 3. Document behavior change
**File**: `ROUTES.md`
**Action**: modify the `### GET /health` section body:

```markdown
### GET /health

Health check. Runs `SELECT 1` against the database pool; returns `200` when
the data layer responds, `500` (via the standard error path) otherwise.

---
```

Route path, method, and port are unchanged — description-only edit, within the self-contained `###`…`---` block.

### Verification

#### Automated
- [x] `./scripts/test.sh` passes
- [x] `cargo test --lib interfaces::routes::tests` green — both health tests (200 happy, 500-with-body sad)

#### Manual
- [ ] With app running: `curl -i localhost:3000/health` → `200`
- [ ] Break the data layer mid-run (e.g. `chmod 000 data/vardy.db-wal`, or point `DATABASE_URL` at an unwritable path in a second boot) → `curl -i localhost:3000/health` → `500` with body `internal server error`; restore perms → `200` again

---

## Phase 3: CI-gated deploy + context hardening

### Changes

#### 1. Trigger deploy off CI success via `workflow_run`
**File**: `.github/workflows/fly-deploy.yml`
**Action**: modify — replace the `on:` block and add an `if:` to the job. Concurrency group and steps unchanged:

```yaml
# See https://fly.io/docs/app-guides/continuous-deployment-with-github-actions/

name: Fly Deploy
on:
  workflow_run:
    workflows: [CI]
    types: [completed]
    branches: [main]
jobs:
  deploy:
    name: Deploy app
    runs-on: ubuntu-latest
    if: github.event.workflow_run.conclusion == 'success'
    concurrency: deploy-group    # optional: ensure only one action runs at a time
    steps:
      - uses: actions/checkout@v4
      - uses: superfly/flyctl-actions/setup-flyctl@master
      - run: flyctl deploy --remote-only
        env:
          FLY_API_TOKEN: ${{ secrets.FLY_API_TOKEN }}
```

Notes:
- The `push: branches: [main, master]` trigger is removed entirely — deploys fire only when the CI workflow completes successfully on `main`. Red commits never deploy; green merges deploy exactly once.
- `master` branch support drops implicitly (CI itself only runs on `main`); acceptable — repo's default branch is `main`.
- GitHub evaluates `workflow_run` using the workflow file **from the default branch**, so gating activates only once this change merges (one final ungated deploy — accepted open risk).

#### 2. Exclude `.env` from build contexts
**File**: `.dockerignore`
**Action**: modify — append under the "Local development and tooling" group:

```
.env
```

Do **not** touch `.sqlx` — it is required in the context for offline query compilation (`ENV SQLX_OFFLINE=true`, `Dockerfile:15`).

### Verification

#### Automated
- [x] `gh workflow list` shows both workflows enabled
- [x] Pushing this feature branch creates **no** Fly Deploy run (deploy no longer has any push trigger); `gh run list --workflow=fly-deploy.yml` confirms
- [x] YAML sanity: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/fly-deploy.yml'))"` exits 0 (validated via `ruby -ryaml` — python3 has no pyyaml module installed)
- [x] Build-context check with a stray `.env` present (from repo root):
      `touch .env; tar --exclude-from=.dockerignore -cf - . | tar -tf - | grep -c '^\.env$'` → `0`;
      then `rm .env`

#### Manual
- [ ] After merge: land a deliberately red commit on `main` (e.g. failing test) → CI fails → **no deploy run appears**; `fly status` release unchanged
- [ ] Land a green commit on `main` → CI succeeds → exactly one Fly Deploy run
- [ ] One canary deploy: confirm Fly holds the previous release serving traffic while the rollout proceeds (`min_machines_running = 1` + `/health`-gated rollout per `fly.toml`)

---

## Testing Checkpoints (from structure.md)

| After | Should be true |
|---|---|
| Phase 1 | Fresh empty `data/` dir + app start = working schema, zero image-build migration steps |
| Phase 2 | `/health` 200 with live DB, 500 (via `WebError`) without; both covered by tests |
| Phase 3 | Red commit on `main` never deploys; green merge deploys exactly once; `.env` never enters build context |

**Known sequencing caveat**: Phase 3's gating semantics can only be fully proven by merging to `main` (GitHub reads `workflow_run` triggers from the default branch). Expect one final ungated deploy when this PR itself merges — accepted per design's open risks.
