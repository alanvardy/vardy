# Design Discussion

## Current State

The deployment pipeline is unsafe by construction (all refs from `research.md`):

- **No CI gate**: `.github/workflows/ci.yml:8-14` and `.github/workflows/fly-deploy.yml:4-8`
  both trigger on push to `main` with no ordering (`fly-deploy.yml` has no `needs:` or
  `workflow_run`). A broken commit deploys while CI is still running.
- **Migrations run at image-build time**: `Dockerfile:31-34` bakes a freshly created,
  migrated *empty* SQLite file into an image layer via `RUN sqlx database create` /
  `RUN sqlx migrate run`. The production binary never migrates — `src/main.rs:23` calls only
  `app::db::init`; the only in-repo migrator is the test harness (`src/test/mod.rs:25-28`).
- **Storage is ephemeral**: `fly.toml` has no `[mounts]`; runtime writes go to
  `/app/data/vardy.db*` on the container rootfs and are wiped on every deploy because each
  deploy replaces the machine's rootfs from the new image.
- **Health check verifies nothing**: `GET /health` returns constant OK
  (`src/interfaces/routes.rs:20`); fly.toml health check (`fly.toml:22-27`) therefore passes
  any machine whose HTTP server binds, even with a dead/broken data layer.
- **Startup failures panic before listeners bind** (`src/app/db.rs:9-27`, `src/main.rs:28-30`),
  which Fly sees only as crash-looping logs — there is no healthy-instance signal.
- **`.env` leaks into the Docker build context**: `.dockerignore:1-14` excludes many things
  but not `.env`; it is copied into build stages (`Dockerfile:5,14`).

## Desired End State

A commit reaching production must have passed CI first, and the deployed app must
self-initialize its database and prove real health:

1. Deploys run only after the full CI workflow (test, todos, fmt, clippy) succeeds on `main`.
2. Migrations execute inside the app at startup via embedded migrations, so every freshly
   started container has a correct schema regardless of what was baked into the image.
3. `/health` performs a live database check so Fly's rollout gates on actual data-layer health.
4. `.env` never enters a Docker build context.

Verification: merge a deliberately failing change → no deploy triggers; start the app against
an empty `data/` dir → schema exists and `/health` returns 200; kill the pool / corrupt the DB
→ `/health` fails; build the image with a stray `.env` present → file absent from all layers.

## Patterns to Follow

- **Workflow structure & concurrency groups**: follow existing style in `ci.yml:29-32`
  (concurrency cancel-in-progress) and `fly-deploy.yml:12` (serialized deploys).
- **Sequential init in `main()`**: keep the strict ordering pattern of `src/main.rs:12-43`;
  insert migration as a step after `db::init` and before listener binds.
- **Result propagation for startup steps that can fail**: prefer `Err` + `?` exit (as metrics
  init does at `src/main.rs:20`) over panic-and-abort where practical.
- **Handler shape**: health check should remain a tiny handler returning `Result<_, WebError>`
  like other handlers, registered in `routes.rs` (`src/interfaces/routes.rs:20`), documented in
  `ROUTES.md`, tested inline (`src/interfaces/routes.rs:113-121`).
- **Test conventions**: happy/sad path tests inline in `#[cfg(test)]`, integration tests via
  `start_app()` from `src/test/mod.rs`.

Patterns NOT to follow:

- **Build-time migration** (`Dockerfile:33-34`) — remove these lines; schema lifecycle belongs
  to the running app, not the image layer.
- **Constant-value health handler** (`routes.rs:20`) — replaced by a real check.
- **Panic-on-failure DB init** (`db.rs:9-27`) — acceptable for process abort, but new code
  (migration runner, health check) must return `Result` and surface through `WebError` /
  exit codes rather than panicking mid-request.

## Design Decisions

1. **CI gating mechanism — `workflow_run` trigger**: `fly-deploy.yml` drops its
   `push` trigger and instead triggers on `workflow_run` (CI completed, conclusion =
   success, branch `main`). Keeps deploy config isolated from test config; standard GitHub
   pattern; preserves the existing serialized `deploy-group` concurrency.
2. **Migrations at app startup — `sqlx::migrate!()`**: embed `migrations/` in the binary and
   run immediately after `db::init` in `main()`. Idempotent (`_sqlx_migrations` bookkeeping),
   works on any fresh filesystem. A failing migration exits non-zero before listeners bind →
   Fly crash-loops the old release stays serving (Fly keeps old machines until replacement is
   healthy).
   Consequence: `sqlx-cli` stays in the runtime image (`Dockerfile:9,26`) even though
   startup migrations no longer need it — kept deliberately for manual use when
   remoting into the machine (e.g. ad-hoc `sqlx migrate info` / inspection).
3. **Storage stays ephemeral (no volume)**: accepted and confirmed trade-off — deploys wiping
   dumps/unsplash rows is acceptable. This ticket does not change the storage model;
   moving to a volume or managed Postgres remains future work if needs change.
4. **`/health` checks the database**: handler acquires a connection from the pool and runs
   `SELECT 1`; failure maps to `WebError::Database` → 500 through the standard error path
   (`src/app/error.rs:31-36`). Local SQLite ping is ~free; Fly's 30s interval
   (`fly.toml:22-27`) poses no load risk. Startup failure detection remains primarily
   crash-loop-based (panics before bind); `/health` adds runtime data-layer detection.
5. **Docker hardening — exclude `.env` only**: add `.env` to `.dockerignore`. Keep `.sqlx`
   in context: it is required at build time for offline query compilation
   (`ENV SQLX_OFFLINE=true`, `Dockerfile:15`).

## What We're NOT Doing

- No Fly volume, no Postgres migration, no backup/snapshot mechanism (data stays ephemeral).
- No automatic rollback on failed deploys (Fly's default old-release-stays-serving behavior is
  sufficient; manual `fly releases rollback` remains the escape hatch).
- No changes to branch protection rules (out of repo scope).
- No deepening beyond `SELECT 1` (no dependency fan-out, no readiness vs liveness split).
- No Sentry/logging/tracing changes; no `ROUTES.md` additions beyond updating the `/health`
  entry's description (behavior change, same route).
- No multi-machine scaling work (single machine assumed).

## Open Risks

- **Bad-migration loop**: a migration that fails partway leaves `_sqlx_migrations` partially
  advanced on the ephemeral fs — harmless across deploys (fs is recreated), but a migration
  that applies then breaks at runtime will crash-loop with no automated revert. Mitigated by
  CI tests, not eliminated.
- **`workflow_run` runs from default branch**: the workflow file version used by
  `workflow_run` events comes from `main`; first-time setup requires this change itself to be
  merged before gating takes effect (one final ungated deploy).
- **Fly remote-builder behavior unverified**: whether Fly holds the previous release while the
  new machine crash-loops depends on `min_machines_running = 1` + health-gated rollout
  (`fly.toml:15,17,22-27`); should be confirmed with one canary deploy.
