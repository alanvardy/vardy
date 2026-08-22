# Research Findings

## Q1: How does `src/main.rs` bootstrap the application end-to-end?

### Findings
- `main.rs` is 34 lines; entire bootstrap is one `async fn main()` (`src/main.rs:5-30`).
- Sequence, in order:
  1. Module declarations: `mod app; mod infra; mod interfaces;` (`src/main.rs:1-3`)
  2. Env read: `std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/vardy.db".to_string())` (`src/main.rs:7-8`) — **no dotenv/dotenvy call exists anywhere**; `dotenvy` appears only as a transitive dep of sqlx (`Cargo.lock:373`).
  3. First fallible step: `let metrics = Arc::new(infra::metrics::AppMetrics::new()?);` (`src/main.rs:9`)
  4. State construction — struct literal with three fields (`src/main.rs:10-14`; struct at `src/app/state.rs:1-6`):
     - `templates: app::templates::init()`
     - `db: app::db::init(&database_url).await` (pool built in `src/app/db.rs:7-28`: `SqliteConnectOptions`, WAL, FKs, max_connections(5))
     - `metrics`
  5. Listener bindings: main on `"0.0.0.0:3000"` (`src/main.rs:15`), metrics on `"0.0.0.0:9090"` (`src/main.rs:17`), with a startup `println!` between them (`src/main.rs:16`).
  6. `tokio::try_join!` (`src/main.rs:18-29`) concurrently serves:
     - main router: `interfaces::routes::routes().with_state(state).into_make_service_with_connect_info::<SocketAddr>()` (`src/main.rs:21-24`) — note the router is built *inside* `try_join!`, after both listeners are bound
     - metrics router: `metrics_router(metrics).into_make_service()` (`src/main.rs:25-28`)
- Facts about where early init fits in the current sequence: nothing executes before the env read at `src/main.rs:7`; everything from `src/main.rs:6-14` runs before any port binds (`src/main.rs:15,17`); first possible inbound request is at the `try_join!` (`src/main.rs:18`).
- No telemetry exists today: grep for `sentry|tracing_subscriber` in src/ returns nothing; deps are axum, minijinja, prometheus, serde, serde_json, sha2, sqlx, tokio, tower-http only (`Cargo.toml:6-19`).

## Q2: How are env vars read/validated here vs. sibling's typed `Env`?

### Findings (main repo)
- Exactly one true env read: `DATABASE_URL` via plain `std::env::var` with inline string fallback (`src/main.rs:8`); never panics, never validated.
- No dotenv loading, no other configurable settings; ports hardcoded at `src/main.rs:15,17`.
- Dockerfile sets `ENV SQLX_OFFLINE=true` (builder, `Dockerfile:15`) and `ENV DATABASE_URL=sqlite:data/vardy.db` (runtime, `Dockerfile:26`); migrations run at image build time (`Dockerfile:27-28`).
- `fly.toml` has no `[env]` section.

### Findings (sibling `/Users/vardy/dev/api`)
- Typed `Env` struct at `api/src/app/env.rs:8-28`, including `pub sentry_dsn: String` (`env.rs:19`) and `pub enable_sentry: bool` (`env.rs:20`).
- **Premise correction**: these fields are not optional — helpers panic on missing/empty:
  - `get_string_env` (`env.rs:163-167`): `env::var(key).ok().filter(!is_empty).unwrap_or_else(|| panic!("{key} must be set and non-empty"))`
  - `get_bool_env` (`env.rs:169-174`): accepts only `"true"`/`"false"`, panics otherwise.
- `Env::init()` reads `get_string_env("SENTRY_DSN")` / `get_bool_env("ENABLE_SENTRY")` (`env.rs:37-38`); no field has defaults. The only truly optional setting is `SES_AWS_ENDPOINT_URL` via `if let Ok(...)` (`env.rs:117-119`).
- Consumption gate: `let _guard = env.enable_sentry.then(|| infra::sentry::init(&env.sentry_dsn));` (`api/src/main.rs:24-26`).

## Q3: Sibling's Sentry init, panic hook, broken-pipe filter, crates

### Findings (`api/src/infra/sentry.rs`, 49 lines)
- Init: `sentry::init((sentry_dsn, ClientOptions::default().maybe_release(sentry::release_name!()).send_default_pii(true)))` (`sentry.rs:2-9`), returning `ClientInitGuard` (`sentry.rs:1,37`). No `environment`, no traces/sample rate configured.
- Panic hook chaining (`sentry.rs:20-35`):
  - Captures the crate-installed hook: `std::panic::take_hook()` (`sentry.rs:20`)
  - New hook writes panic to stderr, discarding write errors (`sentry.rs:22-23`)
  - Broken-pipe short-circuit before forwarding: `if is_broken_pipe(info) { return; }` (`sentry.rs:26-28`)
  - Original hook invoked inside `catch_unwind(AssertUnwindSafe(...))` to avoid double-panic abort (`sentry.rs:30-34`; rationale comments `sentry.rs:11-19`)
- `is_broken_pipe` (`sentry.rs:40-49`): downcasts payload to `&str` then `String`; returns true if message contains `"Broken pipe"` or `"os error 32"` (`sentry.rs:48`).
- Call site: gated by flag at `api/src/main.rs:25-26`. Events reach Sentry **only via the panic hook** (default `sentry` features include sentry-panic) — there are zero `sentry::capture*` calls anywhere in api/src.
- Cargo: single direct dependency `sentry = "0.49.0"` with default features (`api/Cargo.toml:17`); transitive crates all resolved at 0.49.1 incl. `sentry-panic`, `sentry-log`, `sentry-tracing`, `sentry-contexts` etc. (`api/Cargo.lock:3697-3815`). No feature flags customized; no `sentry-tower`.

## Q4: `WebError` lifecycle and error reporting

### Findings (main repo, `src/app/error.rs`)
- Enum (`error.rs:11-15`): `Template(minijinja::Error)`, `Database(sqlx::Error)`, `NotFound` (the latter `#[allow(dead_code)]`, test-only per doc comment `error.rs:10`).
- From conversions: `From<minijinja::Error>` (`error.rs:17-21`), `From<sqlx::Error>` (`error.rs:23-27`); triggered via `?` in handlers (`home/web.rs:4,7`, `dump/web.rs:1,18,33`, `singlethread/web.rs:4,7`).
- IntoResponse (`error.rs:29-41`):
  - `NotFound` → `(404, "not found")` plain text (`error.rs:30`)
  - `Database(err)` → `eprintln!("database error: {err}")` then `(500, "internal server error")` (`error.rs:31-34`)
  - `Template(err)` → `eprintln!("template render error: {err}")` then `(500, "internal server error")` (`error.rs:35-38`)
- These two `eprintln!` sites (`error.rs:32,36`) are the only error logging in the repo (plus startup `println!` at `src/main.rs:16`). No tracing/log/sentry usage anywhere in src/.
- In-file tests confirm status mapping (`error.rs:46-68`).

### Findings (sibling error path)
- api captures **no handled errors into Sentry** — grep for `capture_error|capture_message|capture` in api/src finds nothing. Its richer `AppError`/`WebError` types log via `tracing::error!` inside IntoResponse paths (`api/src/app/error.rs:79,186,198,326`); those logs go to tracing-subscriber JSON output, not Sentry.
- Only panics reach Sentry, via the wrapped panic hook installed at boot (see Q3).

## Q5: Middleware/tower layers

### Findings (main repo, `src/interfaces/routes.rs`)
- Main router `routes()` (`routes.rs:11-31`): routes `/`, `/singlethread`, `/dump/{key}` GET+POST, `/health`, and `nest_service("/static", ...)` (`routes.rs:13-27`). **Zero `.layer()`/`.route_layer()` calls.** The only middleware-ish item is `SetResponseHeader::overriding(ServeDir::new("static"), CACHE_CONTROL, ...)` wrapping just the static service (`routes.rs:22-26`).
- `.with_state(state)` is applied by callers: `src/main.rs:22`, `src/test/mod.rs:20,44`.
- Metrics router (`routes.rs:34-37`): `/metrics` + `.with_state(metrics)` — the only `with_state` in routes.rs.
- No middleware defined elsewhere in src/ (grep `layer(|tower|middleware` across src/ confirms).

### Findings (sibling)
- Top-level assembly (`api/src/main.rs:83-87`): `.layer(app::log::trace_layer())` (TraceLayer for HTTP with route+method spans, `api/src/app/log.rs:62-84`), wrapped by rate limiting `with_global_limit(router, ...)` then `.with_state(state)`.
- Rate limiting via tower-governor `GovernorLayer` (`api/src/app/rate_limit.rs:56,72`), keyed on Fly client IP.
- Per-subrouter layers via `axum::middleware::from_fn_with_state` (JWT or web-password auth) throughout `api/src/interfaces/routes.rs` (lines 55-177).
- **No sentry-tower layer, Hub usage, CaptureControl, or per-request capture middleware** (grep confirms none). Sentry capture is process-level panic-hook only.

## Q6: Test state construction

### Findings (main repo)
- `AppState` fields: `templates: minijinja::Environment<'static>`, `db: SqlitePool`, `metrics: Arc<AppMetrics>` (`src/app/state.rs:3-5`). No sentry-related fields exist.
- `start_app()` (`src/test/mod.rs:5-25`): `db::init("sqlite::memory:")` (`test/mod.rs:6`) → run migrations (`test/mod.rs:7-10`) → AppState literal (`test/mod.rs:11-14`) → bind `127.0.0.1:0` (`test/mod.rs:15-17`) → `routes().with_state(state)` (`test/mod.rs:20`) → spawn `axum::serve`, return addr (`test/mod.rs:21-24`).
- Variant `start_app_with_metrics()` also serves `metrics_router` on a second random port (`src/test/mod.rs:30-56`); `test_client()` is `reqwest::Client::new()` (`src/test/mod.rs:59-61`).

### Findings (sibling)
- api tests build a full `Env` literal: `sentry_dsn: "test-dsn".into()` (const at `api/src/test/mod.rs:29`) and `enable_sentry: false` (`api/src/test/mod.rs:203-204`).
- Sentry stays off because init lives solely in `main()` behind the flag (`api/src/main.rs:23-26`); the `app()` builder performs no sentry init, so no client is ever created in tests regardless of dsn value.

## Q7: Deployment configuration & secrets

### Findings (main repo)
- `Dockerfile` (30 lines): cargo-chef builder (`Dockerfile:1`), installs sqlite sqlx-cli (`Dockerfile:9`), `cargo build --release --bin vardy` (`Dockerfile:16`); runtime `debian:bookworm-slim` (`Dockerfile:19`). ENV: `SQLX_OFFLINE=true` (`:15`), `DATABASE_URL=sqlite:data/vardy.db` (`:26`); `sqlx database create` + `sqlx migrate run` executed at image build (`:27-28`); `ENTRYPOINT ["/usr/local/bin/vardy"]` (`:30`). No ARG, no EXPOSE, no Sentry vars.
- `fly.toml`: `app = 'vardy'`, region `ord` (`fly.toml:6-7`); empty `[build]` (`:9-10`); **no `[env]` section**; `[http_service]` internal_port 3000, health check GET `/health` every 30s (`:12-25`); `[[vm]]` 512mb/1cpu (`:27-29`); `[metrics]` port 9090 path `/metrics` (`:31-33`).
- CI/CD: `fly-deploy.yml` runs `flyctl deploy --remote-only` with `FLY_API_TOKEN` secret (`fly-deploy.yml:1-19`) — the only referenced secret.
- `.env_template` contains one line: `DATABASE_URL=sqlite:data/vardy.db`. Grep for SENTRY/DSN in deployment config, README, workflows: **zero matches** (matches exist only under `.pi/qrspi/` docs).

### Findings (sibling)
- api passes Sentry secrets **neither via Dockerfile nor fly.toml** (its fly.toml `[env]` holds only `PORT = '3000'`). Runtime values come from Fly machine environment managed outside the repo.
- Documented checklist in `.env_template:1-5`: "New entries need to be added: In .env / In .env_template / In fly.io dashboard / In 1Password"; entries at `.env_template:13` (`ENABLE_SENTRY=false`) and `:30` (`SENTRY_DSN=XXXX`).
- Same checklist repeated in `CONTRIBUTING.md:9`. Local real DSN present in gitignored `.env:17`.

## Cross-Cutting Observations
- The sibling api project is a direct template for the infrastructure in question: typed required `Env` struct with panicking parse helpers, `infra::sentry.rs` module, flag-gated init in `main()` holding the returned `ClientInitGuard`, and tests that simply set `enable_sentry: false` while init stays confined to `main()`.
- In api, Sentry receives events **exclusively via the default panic integration's hook** (wrapped to filter broken pipes); handled errors are logged through tracing only and never captured explicitly. Any error-path capture would have no precedent in api either.
- Neither project loads `.env` programmatically despite docs referencing it; api scripts source `.env` manually (`api/scripts/make-jwt.sh:32`, `seed-admin.sh:15`).
- Main repo currently has zero middleware layers, zero telemetry deps, and exactly one config var (`DATABASE_URL` with silent fallback rather than fail-fast validation).
- Deployment secret flow in both repos: secrets live in Fly machine env/dashboard (plus 1Password for api), never committed in Dockerfile/fly.toml/workflows.

## Open Areas
- Whether release/environment metadata should come from Fly (e.g., `FLY_MACHINE_ID`, git SHA) — neither repo sets `ClientOptions.environment`, and api relies only on `sentry::release_name!()`.
- Main repo has no tracing/log facade at all; how (or whether) non-panic errors would surface beyond `eprintln!` is unaddressed in both codebases.
- Metrics port 9090 is exposed via `[metrics]` in fly.toml but no public services config for it was found beyond that block.
