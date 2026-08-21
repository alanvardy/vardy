# Design — Add a Health Endpoint (VAR-665)

## Current State

The app (`vardy`) is a small Axum service exposing only `/`, `/singlethread`,
and `/static` (`src/interfaces/routes.rs:7-12`). All handlers render
minijinja templates via `AppState` (`src/app/state.rs:1-4`), which holds only
a `templates: minijinja::Environment<'static>`.

There is **no health endpoint and no platform-level monitoring** today:

- No `[[http_service.checks]]` or `[metrics]` section exists anywhere in
  `fly.toml` (file ends at line 25).
- `Dockerfile:1-14` has no `EXPOSE` / `HEALTHCHECK`.
- The deploy workflow `.github/workflows/fly-deploy.yml:3-19` runs
  `flyctl deploy --remote-only` with no post-deploy smoke test.
- Grep finds no `health_check` or metrics code anywhere in this repo's src.

The sibling project `../api` is the in-tree reference for every element of
this work:

- Inline closure handler registered directly in the router:
  `.route("/health_check", get(|| async { StatusCode::OK }))`
  (`../api/src/interfaces/routes.rs:18`, `StatusCode` imported from
  `axum::http` at line 5).
- Integration test asserting HTTP 200 through `start_app` + `test_client`
  (`../api/src/interfaces/routes.rs:192-203`).
- Fly HTTP health check hitting that path every 30s
  (`../api/fly.toml:22-26`).

Noted pre-existing inconsistency: `fly.toml:12` sets env `PORT = '8080'` while
the app binds `0.0.0.0:3000` (`src/main.rs:9`) and Fly routes to
`internal_port = 3000` (`fly.toml:16`). The env var is dead/stale config.

## Desired End State

1. `GET /health` returns HTTP 200 with an empty body, no state required.
2. A `#[tokio::test]` integration test asserts the 200 response over real
   HTTP using the existing `start_app()`/`test_client()` helpers.
3. `fly.toml` contains a `[[http_service.checks]]` block targeting
   `path = "/health"` so Fly actively monitors uptime (grace 10s, interval
   30s, timeout 5s), mirroring `../api/fly.toml:22-26`.
4. The stale `PORT = '8080'` env line is removed from `fly.toml` so the file
   no longer contradicts the actual bind port.

Verification: `cargo nextest run` passes including the new test;
`cargo clippy` clean; on deploy, Fly's check shows healthy for the new route.

## Patterns to Follow

- **Route registration**: add one `.route(...)` line to the existing builder
  chain in `pub fn routes() -> Router<AppState>`
  (`src/interfaces/routes.rs:7-12`). Import `axum::http::StatusCode` alongside
  the existing `axum::routing::get` imports (see import style at
  `../api/src/interfaces/routes.rs:5`).
- **Inline closure handler** for status-only routes — the reference pattern
  from `../api/src/interfaces/routes.rs:18`. Deliberately chosen over this
  repo's `handlers/<domain>/web.rs` module convention because a 3-line
  stateless closure does not justify a module; matches the stated reference
  implementation.
- **Integration tests live in `#[cfg(test)] mod tests` inside
  `src/interfaces/routes.rs`**, driven by `start_app()` (`src/test/mod.rs:6-20`)
  + `test_client()` (`src/test/mod.rs:22-24`) against an OS-assigned port —
  exactly the shape of `static_icon_is_served` (`src/interfaces/routes.rs:18-35`).
  Use plain `#[tokio::test]`, not `#[sqlx::test]`: this repo has no database.
- **Fly health check block**: copy the structure of
  `../api/fly.toml:22-26` (`grace_period = "10s"`, `interval = "30s"`,
  `method = "GET"`, `timeout = "5s"`), changing `path` to `/health`.

**Patterns NOT to follow:**

- Do **not** replicate `../api`'s separate Prometheus `metrics_router` /
  second listener on port 9090 (`../api/main.rs:48-60`,
  `../api/routes.rs:27-32`, `../api/handlers/metrics.rs:9-14`) — out of scope.
- Do **not** add Docker `HEALTHCHECK`/`EXPOSE` — neither repo has this
  convention; Fly checks are the established monitoring mechanism here.
- Do **not** create a DB-backed `#[sqlx::test]` like the sibling's health
  test (`../api/src/interfaces/routes.rs:192-203`) — wrong for a DB-less app.

## Design Decisions

1. **Handler style — inline closure** in `routes.rs` (Option A). Simplest
   diff; identical to the reference implementation; no module, no state, no
   error type needed. If health later needs dependency checks (DB, upstream),
   promote it then to `handlers/health/web.rs`.
2. **Path — `/health`** (user choice, Option B). Shorter and conventional;
   intentionally diverges from the sibling's `/health_check`. The Fly check
   must target `/health`, not the sibling's path.
3. **Add Fly `[[http_service.checks]]` targeting `/health`** — without it the
   endpoint exists but nothing monitors it; there is no post-deploy smoke
   test in `fly-deploy.yml:3-19` to compensate.
4. **Remove the stale `PORT = '8080'` env** (`fly.toml:12`) while editing the
   file — it contradicts `main.rs:9` (bind 3000) and `internal_port = 3000`
   (`fly.toml:16`); leaving known-wrong config adjacent to our change invites
   future confusion. Low-risk: the app never reads it (`src/main.rs:9` hardcodes
   the address).
5. **Test — plain `#[tokio::test]`** reusing `start_app`/`test_client`
   (confirmed). Asserts `StatusCode::OK` on `GET /health`; mirrors
   `static_icon_is_served` structure.
6. **No coverage-config changes.** Coverage gates run only on pushes to main
   (`ci.yml:66-76`); the new handler line is covered by the integration test,
   satisfying patch targets without touching `codecov.yml`.

## What We're NOT Doing

- No Prometheus metrics endpoint, second listener, or `[metrics]` fly section.
- No dependency-aware health payload (JSON body, DB/cache checks) — status
  code only, matching the reference.
- No Dockerfile changes (`HEALTHCHECK`, `EXPOSE`).
- No deploy-workflow smoke-test step in `fly-deploy.yml`.
- No codecov/nextest configuration changes.
- No renaming of the sibling's `/health_check` or cross-repo refactors.

## Open Risks

- **First-deploy check window**: adding `[[http_service.checks]]` means Fly
  starts probing `/health` immediately after deploy; since the route ships in
  the same release, this should be safe, but if deploy ordering ever splits
  config from code, the check would fail during the gap. Mitigated by the
  10s grace period.
- **Coverage gates apply post-merge**: the closure body is exercised by the
  integration test, but codecov's 90% patch target (`codecov.yml:1-8`) could
  still flag surrounding lines; expected noise only, no action planned.
- **Port assumption**: design assumes Fly's `internal_port` (3000) remains
  authoritative. If anyone ever wires `PORT` back up, the removed env line
  would need restoring deliberately.
