# Structure Outline

## Approach
Make deploys safe in three moves: teach the app to migrate its own database at startup (so images carry no schema state), make `/health` prove data-layer liveness (so Fly gates rollouts on reality), then chain the deploy workflow behind CI success via `workflow_run`. Storage stays ephemeral per design decision #3.

---

## Phase 1: Self-migrating app

The binary embeds `migrations/` via `sqlx::migrate!()` and applies them at startup, right after pool creation. The Dockerfile stops baking a migrated empty DB into image layers — schema lifecycle moves entirely into the running app.

**Files**: `src/main.rs`, `src/app/db.rs`, `Dockerfile`
**Key changes**:
- `db::migrate(pool: &SqlitePool) -> Result<(), MigrateError>` — new; wraps `sqlx::migrate!().run(pool)`
- `main()`: insert `db::migrate(&pool).await?` between `db::init` and listener binds — failure exits non-zero pre-bind (Fly keeps old release)
- `Dockerfile:32-34`: delete `RUN mkdir -p data` / `RUN sqlx database create` / `RUN sqlx migrate run`

**Verify**: `./scripts/test.sh` passes; new inline test runs `db::migrate` against an empty temp-dir SQLite and asserts `dumps` table exists (happy); sad path asserts `Err` on a corrupt migration source isn't needed — instead assert second call is idempotent (no-op). Manual: `rm -rf data && cargo run` boots cleanly and `/health` responds.

---

## Phase 2: Real `/health` check

`GET /health` stops being a constant OK: it acquires a pooled connection and runs `SELECT 1`. Failure flows through `WebError::Database` → logged + Sentry-captured 500 via the standard error chokepoint. Fly's rollout now fails when the data layer is dead.

**Files**: `src/interfaces/routes.rs`, `ROUTES.md`
**Key changes**:
- `async fn health(State(state): State<AppState>) -> Result<StatusCode, WebError>` — replaces inline constant closure at `routes.rs:20`; registered same route
- Existing test (`routes.rs:113-121`) updated; new sad test closes the pool first (`state.db.close().await`) and asserts 500 **and** error-shaped body

**Verify**: `./scripts/test.sh` passes; manual: stop Postgres-equivalent (delete `data/vardy.db` mid-run or kill WAL perms) → `/health` returns 500, restore → 200.

---

## Phase 3: CI-gated deploy + context hardening

Deploy no longer races CI: `fly-deploy.yml` triggers on successful completion of the `CI` workflow on `main`, keeping its serialized `deploy-group` concurrency. `.env` is excluded from all Docker build contexts.

**Files**: `.github/workflows/fly-deploy.yml`, `.dockerignore`
**Key changes**:
```yaml
# fly-deploy.yml — replaces push trigger
on:
  workflow_run:
    workflows: [CI]
    types: [completed]
    branches: [main]
jobs:
  deploy:
    if: github.event.workflow_run.conclusion == 'success'
```
- `.dockerignore`: append `.env` (keep `.sqlx` — needed for offline compile)

**Verify**: `gh workflow list` + push branch → no deploy run on PR; manual post-merge: deliberately red commit on `main` produces no deploy run; green merge deploys once. Confirm `.env` absent: `tar --exclude-from=.dockerignore -cf - . | tar -tf - | grep -c '^\.env$'` → 0, and one canary deploy confirms Fly holds the old release during rollout.

---

## Testing Checkpoints

| After | Should be true |
|---|---|
| Phase 1 | Fresh empty `data/` dir + app start = working schema, zero image-build migration steps |
| Phase 2 | `/health` 200 with live DB, 500 (via `WebError`) without; both covered by tests |
| Phase 3 | Red commit on `main` never deploys; green merge deploys exactly once; `.env` never enters build context |

**Not vertically sliceable (noted)**: Phases 1–2 are fully testable locally; Phase 3's gating semantics can only be proven by merging to `main` (GitHub uses the workflow file from the default branch for `workflow_run`). Expect one final ungated deploy when this PR itself merges — accepted per design's open risks.
