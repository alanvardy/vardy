# Research Findings

## Q1: Route registration, handler organization, and minimal status-code-only handlers

### Route registration (this repo)
- All routes registered in a single `pub fn routes() -> Router<AppState>` — `src/interfaces/routes.rs:7-12`:
  ```rust
  pub fn routes() -> Router<AppState> {
      Router::new()
          .route("/", get(handlers::home::web::index))
          .route("/singlethread", get(handlers::singlethread::web::index))
          .nest_service("/static", ServeDir::new("static"))
  }
  ```
- Pattern: `.route(path, get(handler_fn))`, handler referenced by full path `handlers::<domain>::web::<fn>`. Static assets via `nest_service` + `ServeDir`.
- Lifecycle: called once in `src/main.rs:13-16`, then `.with_state(state)` + `.into_make_service_with_connect_info::<SocketAddr>()` before `axum::serve`. The test harness mirrors this (`src/test/mod.rs:13`).

### Handler organization (this repo)
- `src/interfaces/mod.rs:1-2`: `pub mod handlers; pub mod routes;`
- `src/interfaces/handlers/mod.rs:1-2`: one module per domain — `pub mod home; pub mod singlethread;`
- Each domain is a directory with `mod.rs` re-exporting a `web` submodule (`src/interfaces/handlers/home/mod.rs:1`, `.../singlethread/mod.rs:1`); handler fns live in `web.rs`.
- State access: axum `State` extractor over `AppState` (`src/app/state.rs:1-4`), which holds only `templates: minijinja::Environment<'static>`. Handler shape returns `Result<Html<String>, WebError>` — see `src/interfaces/handlers/singlethread/web.rs:9`.

### Minimal status-code-only handler
- **None exists in this repo** — both handlers render templates.
- Sibling project has one, registered as an inline closure directly in the router — `../api/src/interfaces/routes.rs:18`:
  ```rust
  .route("/health_check", get(|| async { StatusCode::OK }))
  ```
  (`StatusCode` from `axum::http` imported at `../api/src/interfaces/routes.rs:5`.) No dedicated function or module; no state needed.

## Q2: Integration tests and coverage measurement

### `src/test/` (this repo)
- Wired into crate at `src/main.rs:22` (`mod test;`).
- **`start_app()`** (`src/test/mod.rs:6-20`): builds real `AppState`, binds `127.0.0.1:0` (OS-assigned port), serves `routes().with_state(state)` on a spawned Tokio task, returns `SocketAddr`. No database involved.
- **`test_client()`** (`src/test/mod.rs:22-24`): returns plain `reqwest::Client::new()`.
- Sub-helper pattern in sibling repo: same names but `start_app(pool: PgPool)` takes a Postgres pool and builds a full test `Env` (`../api/src/test/mod.rs:148-196`); also provides auth/seed/assertion helpers there.

### How route tests are written
- Tests live in `#[cfg(test)] mod tests` inside `src/interfaces/routes.rs`. Existing example: `static_icon_is_served` at `src/interfaces/routes.rs:18-35` — calls `start_app().await`, `test_client().get(format!("http://{addr}/..."))`, asserts `res.status() == StatusCode::OK`.
- Canonical health-check-style test exists only in the sibling repo — `health_check_returns_200` at `../api/src/interfaces/routes.rs:192-203`:
  ```rust
  #[sqlx::test]
  async fn health_check_returns_200(pool: PgPool) {
      let addr = start_app(pool).await;
      let client = test_client();
      let response = client
          .get(format!("http://{addr}/health_check"))
          .send()
          .await
          .expect("request to health_check should succeed");
      assert_eq!(response.status(), reqwest::StatusCode::OK);
  }
  ```
- Variants observed: DB-backed tests use `#[sqlx::test]`; pure tests use plain `#[tokio::test]` (or `tower::ServiceExt::oneshot`, e.g. `metrics_router_serves_metrics_endpoint`, `../api/src/interfaces/routes.rs:241-263`). This repo's existing route test uses plain `#[tokio::test]` (no DB).
- Handler modules also carry inline `#[cfg(test)] mod tests` blocks (e.g. `src/interfaces/handlers/home/web.rs`).

### Coverage measurement
- `codecov.yml` (this repo): ignores `src/main.rs`; project target 70%, patch target 90% (`codecov.yml:1-8`). Sibling adds a stricter per-path patch gate for `src/interfaces/handlers/**` at 95% (`../api/codecov.yml`).
- `.config/nextest.toml:1-2` (both repos): only `[profile.ci.junit] path = "junit.xml"`.
- CI wiring `.github/workflows/ci.yml`: PRs run `cargo nextest run --profile ci` (no coverage, `ci.yml:61-63`); pushes to main run `cargo llvm-cov nextest --profile ci --all-features --lcov` and upload `lcov.info` via codecov-action (`ci.yml:66-76`) plus JUnit results upload (`ci.yml:77-87`).

## Q3: Deployment and monitoring configuration

### This repo
- `fly.toml:8-20`: app `'vardy'`, region `'ord'`, env `PORT = '8080'` (`fly.toml:12`), `[http_service]` with `internal_port = 3000` (`fly.toml:16`), `force_https = true`, auto stop/start machines, `min_machines_running = 1`. VM: 512mb (`fly.toml:25`).
- **No `[[http_service.checks]]` and no `[metrics]` section exist anywhere in `fly.toml`** (file ends at line 25).
- Noted inconsistency (as-is): `PORT='8080'` env (`fly.toml:12`) vs app binding `0.0.0.0:3000` (`src/main.rs:9`) and `internal_port = 3000` (`fly.toml:16`).
- `Dockerfile:1-14`: three-stage cargo-chef build; runtime is `debian:bookworm-slim`, copies `templates`, `static`, binary to `/usr/local/bin/vardy`, entrypoint `["/usr/local/bin/vardy"]`. **No `EXPOSE`, no `HEALTHCHECK`.**
- Deploy workflow `.github/workflows/fly-deploy.yml:3-19`: push to main/master → `flyctl deploy --remote-only`. No post-deploy verification or smoke-test steps.
- App currently exposes only `/`, `/singlethread`, `/static` (`src/interfaces/routes.rs:7-12`); grep finds no `health_check` or metrics code in this repo's src.

### Sibling repo `../api` — comparable configuration present here
- Fly HTTP health check targeting the app route — `../api/fly.toml:22-26`:
  ```toml
  [[http_service.checks]]
    grace_period = "10s"
    interval = "30s"
    method = "GET"
    timeout = "5s"
    path = "/health_check"
  ```
- Prometheus metrics config — `../api/fly.toml:27-29`: `[metrics] port = 9090, path = "/metrics"`.
- App-side `/health_check` route (`../api/src/interfaces/routes.rs:18`) plus a separate `metrics_router(metrics) -> Router` served on a second listener bound to `0.0.0.0:9090` via `tokio::try_join!` (`../api/src/main.rs:48-60`; router defined `routes.rs:27-32`; handler `handlers/metrics.rs:9-14`).
- `../api/Dockerfile` likewise has no `EXPOSE`/`HEALTHCHECK` — its monitoring comes entirely from fly.toml checks + app code.
- Its deploy workflow additionally passes `--image-label ${{ github.sha }}` (`../api/fly-deploy.yml:18`).

## Cross-Cutting Observations
- Both projects share the same architecture: `interfaces/routes.rs` builds a `Router<AppState>`, handlers under `interfaces/handlers/<domain>/`, state via axum `State` extractor; `start_app` + `test_client` drive real-HTTP integration tests against an ephemeral port.
- The sibling `../api` is the in-tree precedent for every element of a health endpoint: inline closure handler (`routes.rs:18`), integration test asserting 200 (`routes.rs:192-203`), and Fly platform check hitting that path (`../api/fly.toml:22-26`).
- Coverage gates run only on pushes to main, not PRs (`ci.yml:61-76` in both repos), so new tests affect codecov numbers after merge.
- This repo's `AppState` carries only templates — a status-only handler would not need any state.

## Open Areas
- No existing named (non-closure) status-code-only handler anywhere in either codebase — the sole precedent is the inline closure in `../api`.
- Whether Fly's health check should target port 3000 vs the stale `PORT=8080` env (`fly.toml:12`) is unresolved by config alone; the app demonstrably binds 3000 (`src/main.rs:9`).
- No Docker-level HEALTHCHECK convention exists in either repo to follow.
