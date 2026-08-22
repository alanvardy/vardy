# Research Findings

## Q1: What happens end-to-end when a commit lands on main?

### Findings
- Two workflows trigger independently on a push to `main`:
  - **CI** — `.github/workflows/ci.yml:8-14` (`push: branches: [main]`, plus `pull_request`, `workflow_dispatch`). Four parallel jobs (no `needs:` anywhere): `test` (`ci.yml:35-90`, runs coverage path `cargo llvm-cov nextest` on main pushes only, `ci.yml:59-63`), `todos` (`ci.yml:92-101`), `fmt` (`ci.yml:103-110`), `clippy` (`ci.yml:112-129`). Concurrency group cancels superseded runs (`ci.yml:29-32`).
  - **Fly Deploy** — `.github/workflows/fly-deploy.yml:4-8` (`push: branches: [main, master]`). Single job: checkout → setup-flyctl → `flyctl deploy --remote-only` with `FLY_API_TOKEN` (`fly-deploy.yml:10-18`); its own `concurrency: deploy-group` serializes deploys (`fly-deploy.yml:12`).
- **Deploy does NOT depend on CI passing.** No `needs:`, no `workflow_run` trigger in `fly-deploy.yml`; GitHub starts both workflows simultaneously on the same push event. A red CI run does not prevent or roll back the deployment.
- Not triggered by a main push: `.github/workflows/codeql/codeql.yml` (weekly cron), `.github/workflows/rust-version-bump.yml` (daily cron; opens a PR that re-triggers this pipeline when merged, `rust-version-bump.yml:41-58`), `.github/workflows/dependabot_auto_merge.yml` (PR-only, auto-merges Dependabot minor updates).
- Repo state corroborates independence: HEAD commit on this branch is titled "Broke on deploy" — a broken commit reached deploy while CI ran separately.
- Branch protection (required checks before merge) lives outside the repo and cannot be confirmed from workflow files; even required checks only gate merges, not push-triggered workflows after the merge lands.

## Q2: Docker image build — database creation and migration timing

### Findings
- Four-stage build (`Dockerfile`): `chef` (:1-2) → `planner` (:4-6) → `builder` (:8-16) → `runtime` (:19-35).
- Builder installs `sqlx-cli` (`--no-default-features --features sqlite`) at `Dockerfile:9` specifically so it can be reused in the runtime stage; `ENV SQLX_OFFLINE=true` (:15) makes compilation use the committed `.sqlx/` query cache instead of a live DB.
- Runtime stage installs `libssl3` + `ca-certificates` (:20-24), copies `sqlx` binary, `migrations/`, `templates/`, `static/`, and the release binary (:26-30).
- **Database creation and migrations run at image-build time**, not container start:
  - `ENV DATABASE_URL=sqlite:data/vardy.db` (`Dockerfile:31`)
  - `RUN mkdir -p data` (`Dockerfile:32`)
  - `RUN sqlx database create` (`Dockerfile:33`)
  - `RUN sqlx migrate run` (`Dockerfile:34`)
  - These execute while Fly builds the image remotely (`flyctl deploy --remote-only`), baking a freshly-created, migrated **empty** SQLite file (plus `_sqlx_migrations` bookkeeping) into an image layer.
- Container start launches only `ENTRYPOINT ["/usr/local/bin/vardy"]` (`Dockerfile:35`). No entrypoint script, no startup migration step.
- The binary itself never migrates: `src/main.rs:23` calls only `app::db::init`; there is no `sqlx::migrate!()` call anywhere in production code. The only migration runner is the test harness (`src/test/mod.rs:25-28`).
- Migrations present: `migrations/0001_placeholder.sql`, `0002_create_dumps.sql` (dumps table + key index), `0003_unsplash_pictures.sql`.
- `.dockerignore:1-14` excludes `.git/`, `.github/`, `/target`, `.pi/`, `scripts/`, `fly.toml`, etc. It does **not** exclude `.env` or `.sqlx` — both are copied into build stages (`Dockerfile:5,14`).

## Q3: Persistent storage configuration and cross-deploy behavior

### Findings
- `fly.toml` has **no `[mounts]` section and no volume** (entire file `fly.toml:1-36` reviewed). Storage is the VM's ephemeral root filesystem only.
- `DATABASE_URL` chain: read via `Env::init()` which panics if unset/empty (`src/app/env.rs:19,29-35`); passed to `db::init` at `src/main.rs:23`; image bakes default `DATABASE_URL=sqlite:data/vardy.db` (`Dockerfile:31`); `.env_template:6` documents the same path; production values are managed as Fly secrets per `.env_template:1-4` comment.
- `src/app/db.rs:7-15`: SQLite options `create_if_missing(true)`, `foreign_keys(true)`, journal mode WAL → runtime writes produce `data/vardy.db`, `-wal`, `-shm` under `/app/data/`. Pool `max_connections(5)` (`db.rs:26-30`).
- All user-persisted data lives in SQLite (no other filesystem writes found): dumps inserts at `src/interfaces/handlers/dump/web.rs:36`, reads at :21; unsplash pictures at `src/app/picture.rs:24-28`, latest fetch at :14-16.
- `.gitignore:3` ignores `/data` entirely.
- **Cross-deploy behavior:** since `/app/data/vardy.db*` lives in the container filesystem and each deploy recreates the machine's rootfs from the new image (which contains a pristine empty migrated DB baked at `Dockerfile:32-34`), all dumps and unsplash rows written at runtime are lost on every deploy. `auto_stop_machines = 'stop'` / `min_machines_running = 1` (`fly.toml:15,17`) preserve data across idle stop/start of the same machine, but not across deploy replacement.
- No backup/snapshot mechanism exists anywhere in the repo.

## Q4: Application startup order and failure surfacing

### Findings
- `main()` (`src/main.rs:12-43`) initializes strictly sequentially:
  1. Logging — `app::log::init()` (`src/main.rs:14`; JSON tracing subscriber on stderr, `src/app/log.rs:47-58`). Cannot fail.
  2. Env vars — `Env::init()` (`src/main.rs:16`) reads `UNSPLASH_API_KEY`, `DATABASE_URL`, `SENTRY_DSN`, `ENABLE_SENTRY`; missing/empty vars or invalid bools **panic** (`src/app/env.rs:31-41`).
  3. Sentry (conditional) — `env.enable_sentry.then(|| infra::sentry::init(...))` (`src/main.rs:17-19`; init + panic hook at `src/infra/sentry.rs:3-38`).
  4. Metrics — `AppMetrics::new()?` (`src/main.rs:20`; registers `page_views_total` counter, `src/infra/metrics.rs`). Only step returning `Err` instead of panicking.
  5. AppState — templates then db pool (`src/main.rs:21-27`): `templates::init()` sets up minijinja with a **lazy** path loader (`src/app/templates.rs:5`); `db::init().await` opens the SQLite pool (`src/main.rs:23`).
  6. Listeners — port 3000 (`src/main.rs:28`), port 9090 (`src/main.rs:30`), errors propagated via `?`.
  7. Serve — both servers concurrently under `tokio::try_join!` (`src/main.rs:32-42`); router built inside the join with `trace_layer()` applied to the main router only (`src/main.rs:34-37`).
- Failure surfacing:
  | Failure | Mechanism |
  |---|---|
  | Missing env var / bad `ENABLE_SENTRY` | panic (`src/app/env.rs:33,40`) |
  | Bad DATABASE_URL / unwritable dir / connect failure | panic via `.expect()` (`src/app/db.rs:9,20,27`) |
  | Prometheus registry error | `Err` → exit code 1 (`src/main.rs:20`) |
  | Port 3000/9090 already bound | `Err` via `?` (`src/main.rs:28,30`) |
  | Missing `static/` dir | panic lazily on first `asset_url` call (`src/app/assets.rs:20,27,41`; OnceLock cache at :8) |
  | Missing `templates/` entries | surfaces lazily per-request as `WebError::Template` → 500 (`src/app/error.rs:37-42`), not at boot |
- Because `db::init` panics rather than returning `Result`, a DB failure aborts the process during AppState construction — before either listener binds, so Fly's health check never sees a healthy instance.

## Q5: Health check / metrics endpoints vs fly.toml

### Findings
- `GET /health` — registered as an inline closure returning constant `StatusCode::OK` (`src/interfaces/routes.rs:20`). Verifies **nothing** — no DB ping, no dependency check. Proves only that the axum server accepts HTTP on port 3000. Documented at `ROUTES.md:37-40`. Test at `src/interfaces/routes.rs:113-121`.
- `GET /metrics` — separate router on dedicated port 9090 (`src/interfaces/routes.rs:32-37`, handler `src/interfaces/handlers/metrics/web.rs:7-13`). Serves Prometheus text format from `AppMetrics` (`src/infra/metrics.rs:26-33`), whose only metric is a `page_views_total{page}` counter incremented solely by the home handler (`src/interfaces/handlers/home/web.rs:8`). Test at `routes.rs:61-88`.
- fly.toml alignment:
  - `[http_service] internal_port = 3000` (`fly.toml:14`) matches main bind (`src/main.rs:28`).
  - Health check: GET `/health`, `grace_period = "10s"`, `interval = "30s"`, `timeout = "5s"` (`fly.toml:22-27`). Since `/health` is unconditional OK, Fly considers the machine healthy whenever the HTTP server responds — including if the database has no tables or the pool is dead.
  - `[metrics] port = 9090, path = "/metrics"` (`fly.toml:35-38`) matches the metrics listener (`src/main.rs:30`).
- `ROUTES.md` does not document `GET /metrics` or its separate port.
- Dockerfile has no `EXPOSE` directive; port exposure relies on fly.toml.

## Q6: Error handling, logging, monitoring — how a failed release reveals itself

### Findings
- **Logging** (`src/app/log.rs`): JSON tracing-subscriber on stderr (`log.rs:47-58`), filter from `RUST_LOG` falling back to `"info,tower_http=info"` (`log.rs:48-49`); custom `StderrWriter` swallows BrokenPipe errors so piped stderr (Fly log capture) can't panic the writer (`log.rs:13-42`). `trace_layer()` traces HTTP requests with `ServerErrorsAsFailures` classifier and route-pattern spans (`log.rs:62-81`), applied to the main router at `src/main.rs:36`.
- **Sentry** (`src/infra/sentry.rs`): `sentry = "0.49"` (`Cargo.toml:17`); init tags events with `release_name!()` (`sentry.rs:5`) so failures attribute to a release; custom panic hook skips broken-pipe panics and wraps the original hook in `catch_unwind` (`sentry.rs:20-35,41-49`). Gated by `ENABLE_SENTRY`/`SENTRY_DSN` (`src/main.rs:17-19`, `src/app/env.rs:17-18`).
- **WebError** (`src/app/error.rs`): variants `Template`, `Database`, `NotFound`, `External` (:8-14). IntoResponse mapping (:28-47): NotFound → 404 unlogged (:29-30); Database → `tracing::error!` + `sentry::capture_error` + 500 (:31-36); Template → error! + capture + 500 (:37-42); External → error! + 502, **not** sent to Sentry (:43-46). All handlers return `Result<_, WebError>` and propagate with `?`.
- Capture points to Sentry: only Database/Template 500s (`error.rs:35,40`) and non-broken-pipe panics (`sentry.rs:21-35`). No tracing→Sentry bridge exists.
- **Startup failures** surface only as panic messages / JSON error lines on stderr → Fly logs: env panics (`env.rs:33,40`), db panics (`db.rs:9,20,27`), bind errors (`main.rs:28,30`), plus startup info lines "Hosting on http://localhost:3000" (`main.rs:29`) and "Metrics listening on http://localhost:9090" (`main.rs:31`).
- **Monitoring**: Fly health checks hit `/health` (`fly.toml:22-27`); Fly scrapes `/metrics` on 9090 (`fly.toml:35-38`).

## Cross-Cutting Observations
- **The deploy pipeline has no CI gate**: CI and Fly Deploy trigger concurrently on the same push to main with no ordering mechanism (`ci.yml:8-14` vs `fly-deploy.yml:4-8`).
- **Schema lifecycle lives entirely in the Docker build**, not the app: migrations run once per image build against a fresh empty DB (`Dockerfile:33-34`); the binary assumes tables exist (`src/test/mod.rs:25-28` is the only in-repo migrator). Any schema change reaches production only when a new image is built.
- **Data is ephemeral by construction**: no Fly volume + DB baked into image layers means runtime writes vanish on every deploy (Q3), while `/health` (`routes.rs:20`) reports OK regardless, so Fly will not detect data-layer breakage.
- Ports 3000 (app) and 9090 (metrics) are bound separately in code and mirrored exactly in `fly.toml` (`main.rs:28,30` vs `fly.toml:14,36`).
- Startup uses panics for config/db failures and `Result` propagation only for metrics init and listener binds — all visible only in stderr/Fly logs (or Sentry, if enabled, for panics).

## Open Areas
- Branch protection rules on the GitHub repo (required checks before merge) cannot be confirmed from in-repo files; they would gate PR merges but not post-merge deploys either way.
- Whether `ENABLE_SENTRY=true` and a valid `SENTRY_DSN` are actually set as Fly secrets in production is not observable from the repo (managed per `.env_template:1-4` in the Fly dashboard / 1Password).
- Fly remote-builder build logs (where `sqlx migrate run` failures at `Dockerfile:34` would appear) are outside the repository; whether past deploys failed at build time vs runtime cannot be determined from code alone.
