# Research Findings

## Q1: Routing, page handlers, rendering

### Findings
- Handler modules live under `src/interfaces/handlers/`. Five domains declared in `src/interfaces/handlers/mod.rs:1-5`: `dump`, `home`, `metrics`, `singlethread`, `unsplash`. Each domain dir has a `mod.rs` re-exporting its handler file (e.g. `home/mod.rs:1` → `pub mod web;`, `unsplash/mod.rs` → `json`). `src/interfaces/mod.rs:1-2` re-exports `handlers` and `routes`.
- Routes assembled in `src/interfaces/routes.rs:19` `pub fn routes() -> Router<AppState>`. Main router (`routes.rs:36-59`): `/` → `home::web::index`, `/singlethread` → `singlethread::web::index`, `/dump/{key}` GET → `dump::web::index`, plus `.merge(dump_tier)` and `.merge(unsplash_tier)`, `/health`, and `.nest_service("/static", …)`.
- `routes()` registers **no state**; `.with_state(state)` is applied later in `src/main.rs:58` after the global rate-limit layer. Each handler extracts `State<AppState>` itself (e.g. `home/web.rs:7`).
- Canonical full-HTML-page handler — `home/web.rs:12-16`: incs a page-view metric (`state.metrics.inc_page_view("home")`, `src/infra/metrics.rs:26`), fetches wallpaper context via `picture::wallpaper_context(&state).await`, renders `state.templates.get_template("home.html")?.render(context!{ wallpaper_url, photographer, photographer_url })?`, returns `Ok(Html(html))`. `singlethread/web.rs:12-17` is identical (template `singlethread.html`, metric `"singlethread"`).
- Template engine is **minijinja**, built in `src/app/templates.rs:3-21` and stored as `AppState.templates` (`src/app/state.rs:10`). Loader is `path_loader("templates")` (templates dir on disk, `templates.rs:6`).
- Error mapping: `?` on `.get_template()`/`.render()` converts `minijinja::Error → WebError::Template` via `From` (`src/app/error.rs:17-19`); returned through `WebError`'s `IntoResponse` → 500 + tracing + Sentry.
- JSON handlers (`unsplash/json.rs:7-13`, `dump/web.rs:10-15`) return `Result<Json<…>, WebError>` rather than `Html`; `dump/web.rs:18-25` is the POST variant.
- Per-domain convention: full HTML pages live in `<domain>/web.rs` and call `wallpaper_context` for the decorative fallback; JSON endpoints live in `<domain>/json.rs` (unsplash) or `web.rs` (dump) and do **not** increment page-view metrics.

## Q2: Request body extraction & input validation

### Findings
- Extractors in use: `State` (every handler), `Path` (`dump/web.rs:11,19`), `Json` (request + response), **no `Query`, no `Form`**. Grep across `src/**/*.rs` finds zero `Query`/`Form`/`RawForm`/`multipart` usages; `axum-extra` (which provides `Form`) is not a dependency (`Cargo.toml`).
- Exactly **one** body-accepting endpoint: `POST /dump/{key}` → `handlers::dump::web::create` (`src/interfaces/routes.rs:24-26`), handler signature `Path(key), State(state), Json(body): Json<serde_json::Value>` (`dump/web.rs:18-22`). Body is stored after re-serialization (`dump/web.rs:23-24`).
- Two error channels:
  - **`WebError`** (`src/app/error.rs:6-14`): application errors — NotFound→404, Database→500, Template→500, External→502, TooManyRequests→429 (`error.rs:39-65`). Handlers return `Result<_, WebError>`.
  - **Extractor rejections bypass `WebError`** — no custom `FromRequest`/`FromRequestParts` rejection exists (`grep fn from_request: no matches`). Axum defaults apply: bad `Path<String>` → 400; malformed JSON into `Json<serde_json::Value>` → default **422** `JsonRejection` (confirmed by test `dump/web.rs:post_invalid_json_rejected` asserting 400 for malformed JSON in axum 0.8).
- `application/x-www-form-urlencoded` is unsupported today: no `Form` extractor/layer, no `tower-http` form feature (`Cargo.toml` enables only `fs`, `set-header`, `trace`). A form POST to `/dump/{key}` would hit the `Json` extractor and be rejected (422).
- Popular default `DefaultBodyLimit` (2MB) applies to the dump POST; no custom `DefaultBodyLimit` configured.

## Q3: Outbound third-party HTTP (Unsplash) end-to-end

### Findings
- Only one real outbound client: `src/main.rs:24-26` builds `reqwest::Client::builder().timeout(Duration::from_secs(10)).build()`. Single total 10s timeout, no retries, no per-phase timeouts.
- Base URL is a **compile-time `const`**: `UNSPLASH_BASE_URL = "https://api.unsplash.com"` (`src/main.rs:12`), NOT env-driven. Assigned to `state.unsplash_base_url` (`main.rs:45`).
- `AppState` carries `http: reqwest::Client` and `unsplash_base_url: Arc<str>` (`src/app/state.rs:14-17`); API key in `Arc<Env>` (`state.rs:12`).
- Env key `UNSPLASH_API_KEY` parsed in `Env::init()` (`src/app/env.rs:20`), stored as `env.unsplash_api_key`.
- Orchestration in `src/app/picture.rs`: `current()` serves cached row unless stale, else refetches (`picture.rs:39-48`), `random()` refetches only when <5 cached rows (`RANDOM_CACHE_MIN_ROWS`, `picture.rs:53-58`). Single network call site `fetch_and_insert` (`picture.rs:60-66`) passes `&state.http`, `&state.unsplash_base_url`, `&state.env.unsplash_api_key` into `fetch_random`.
- The HTTP call + error mapping: `src/infra/unsplash.rs` builds `GET {base_url}/photos/random` with `?query=nature` and `Authorization: Client-ID {api_key}`. Three failure classes all collapse into `UnsplashError(String)`: transport/timeout (`send()` err), non-2xx status, JSON parse failure.
- `UnsplashError → WebError::External` via `From` (`src/app/error.rs:26-29`); `IntoResponse` renders External as **502 "bad gateway"** (`error.rs:57-60`).
- Page-render path diffExceptions: `wallpaper_context` (`picture.rs:23-27`) calls `.ok().unwrap_or_default()` returning empty strings, so home/singlethread render OK even if Unsplash fails (template guards suppress wallpaper/credit). Only the JSON endpoints surface 502.
- Test base-URL override: harness sets `unsplash_base_url` directly in a hand-built `AppState` (`src/test/mod.rs:48-55`) and `start_unsplash_stub` spawns a local axum stub returned as `UnsplashStub { base_url, call_count }` (`test/mod.rs:165-190`).

## Q4: Config & secret lifecycle

### Findings
- `Env` struct (`src/app/env.rs:4-12`): `unsplash_api_key`, `database_url`, `sentry_dsn`, `enable_sentry`, `rate_limit_per_ms`, `rate_limit_burst`. Parsed by a hand-rolled synchronous `Env::init()` (`env.rs:14-31`) — no `FromEnv`/figment/config crate, no runtime dotenv loader.
- Three parsing helpers (`env.rs:35-47`): `get_string_env` (panics "must be set and non-empty"), `get_bool_env` (only `"true"`/`"false"`), `get_parse_env::<T: FromStr>` (panics on invalid int). All **panic at boot** — fail-fast; any missing key aborts `main()`.
- Keys read: `UNSPLASH_API_KEY`, `DATABASE_URL`, `SENTRY_DSN`, `ENABLE_SENTRY`, `RATE_LIMIT_PER_MS`, `RATE_LIMIT_BURST` (`env.rs:17-23`).
- `.env_template` (repo root) lists the same six keys and a mandating comment: "New entries need to be added: In .env, In .env_template, In fly.io dashboard, In 1Password". `.env` has real local values (incl. a live Unsplash key). `.env` is gitignored; `.envrc` (`dotenv`) loads it for the shell. `scripts/test.sh` sources `.env` explicitly at boot.
- Flight fly.secrets: **no `scripts/deploy.sh`** exists; `scripts/` has only `build-css.sh`, `lint_string.sh`, `reset_db.sh`, `test.sh`. Deploy is `.github/workflows/fly-deploy.yml` running `flyctl deploy --remote-only` with `FLY_API_TOKEN`; no `fly secrets` command in repo. `fly.toml` has no secrets/env block. Fly secrets are applied out-of-band (`fly secrets set KEY=VALUE`, per `env.rs:1-2` doc).
- Value → handler: `Env::init()` (`main.rs:13`) → `AppState.env: Arc<Env>` (`main.rs:29-36`) → handlers read `state.env.<field>` (e.g. `picture.rs:63-64` → header at `infra/unsplash.rs`). Sentry DSN consumed at `main.rs:14-15`.
- Any new Env field requires updating the **literal constructions** in tests too: `src/test/mod.rs:34-41,79-86` and `src/app/picture.rs` unit tests, or the tree won't compile.

## Q5: Rate limiting & abuse mitigation

### Findings
- Stack: `tower_governor 0.8` + `governor 0.10` (`Cargo.toml`).
- Module `src/app/rate_limit.rs`.
- Per-IP key: `FlyClientIpKeyExtractor` (`rate_limit.rs:14-31`) returns `IpAddr`. Primary key is the `fly-client-ip` header (set by Fly proxy, non-spoofable); fallback `req.extensions().get::<ConnectInfo<SocketAddr>>().map(|ci| ci.0.ip())`. `X-Forwarded-For` deliberately ignored (documented `rate_limit.rs:5-11`). Extraction failure → `GovernorError::UnableToExtractKey`.
- `ConnectInfo<SocketAddr>` is wired only on the app port: `into_make_service_with_connect_info::<SocketAddr>()` in `main.rs:65-67` and `test/mod.rs:57`. The metrics port uses `into_make_service()` (no connect-info) and is not rate limited.
- 429 chokepoint: `rate_limit_error_response(err)` (`rate_limit.rs:46-79`) maps `GovernorError::TooManyRequests { wait_time, headers }` → `WebError::TooManyRequests { retry_after_secs: wait_time }.into_response()` then merges governor headers. `WebError::TooManyRequests` body/header set in `src/app/error.rs:62-65` → `429` + `retry-after` + body `"too many requests"`.
- Global limiter: `with_global_limit(router, per_ms, burst)` (`rate_limit.rs:118-125`) — `per_millisecond`, `burst_size`, `use_headers()`, keyed per-IP; spawns a `prune_loop` (`rate_limit.rs:101-110`) that ticks every 60s calling `limiter.retain_recent()`. Wraps the entire router.
- Per-endpoint tiers: `tiered_routes(limited, per_ms, burst)` (`rate_limit.rs:130-141`) — a **separate** `GovernorLayer` with its own `SharedRateLimiter` applied to a sub-router, nested under the global. Budgets **do not pool** across tiers (comment `routes.rs:21-22`).
- Tier budget constants: `DUMP_TIER_PER_MS=1_000, DUMP_TIER_BURST=3`; `UNSPLASH_TIER_PER_MS=200, UNSPLASH_TIER_BURST=5` (`rate_limit.rs:84-87`). "Policy lives in code, not config."
- Router composition (`src/interfaces/routes.rs:20-49`): `dump_tier` (POST `/dump/{key}`) and `unsplash_tier` (`/unsplash`, `/unsplash/random`) are built then `.merge()`d into the base router. Note `/dump/{key}` GET is registered on the base router (global budget only) while `/dump/{key}` POST is in the tier — same path, different budgets. Global layer applied in `main.rs:44`.
- Global budget from env: `RATE_LIMIT_PER_MS`/`RATE_LIMIT_BURST` (`main.rs:22-24`, `env.rs:23-24`).
- Test lowering: `start_app_with_rate_limits(base_url, per_ms, burst)` (`src/test/mod.rs:25-31`) builds via the same `serve_app`. Tests proving a tier trips while global stays open: `unsplash/json.rs:227-270`, `dump/web.rs:34-70`.
- Note: `tiered_routes` does **not** spawn its own pruner; only `with_global_limit` does (`rate_limit.rs:101-110`).

## Q6: Shared layout, navigation, static assets

### Findings
- Base template `templates/layout.html`. A comment at top (`layout.html:1-4`) documents the render-context contract: every extending page must supply `wallpaper_url`, `photographer`, `photographer_url` (missing → wallpaper hidden, credit suppressed).
- `wallpaper` div (`layout.html:10-11`) emits `background-image: url('{{ wallpaper_url }}')` only `{% if wallpaper_url %}`. `photographer` credit (`layout.html:13-20`) renders a linked name when `photographer_url` populated, else plain text — Unsplash attribution.
- Nav (`layout.html:21-24`) is a **hardcoded** list: `<a href="/">Home</a>` and `<a href="/singlethread">SingleThread</a>`. Adding a page to nav = hand-edit here (no data-driven loop).
- Content slots: `<title>{% block title %}Home{% endblock %}</title>`, `<h1>{% block heading %}{% endblock %}</h1>`, `{% block content %}{% endblock %}` (`layout.html:8,26-33`).
- Stylesheet: `<link rel="stylesheet" href="{{ asset_url('site.css') }}">` (`layout.html:9`).
- No context struct — each page passes `context!{}` inline (minijinja macro). Both page handlers pass exactly the three contract fields (home/web.rs:13-17, singlethread/web.rs:13-17). `wallpaper_context` fills them from DB or empty defaults (`picture.rs:23-27`).
- Assets are cache-busted via `asset_url` — a template function (`templates.rs:12-17`) delegating to `src/app/assets.rs`: `ASSET_HASHES: OnceLock<HashMap>` lazily computes a 12-hex sha256 prefix per file under `static/` (`hash_all`, assets.rs:13-31) on first use during `templates::init()`. `asset_url(file)` → `/static/<file>?v=<hash>` (`assets.rs:45-50`), panicking on unknown files. Templates reference images via `{{ asset_url('...') }}` (home.html: wave.svg, github.svg, linkedin.svg, alanvardy.jpg; singlethread.html: shot-*.jpg, watch-*.png).
- Static serving with immutable caching: `routes.rs:36-43` nests `ServeDir::new("static")` behind `SetResponseHeader` forcing `Cache-Control: public, max-age=31536000, immutable`. The `?v=` content hash is what busts the immutable cache on change. Tests assert this (`routes.rs:120-133`).
- `static/` contents: `alanvardy.jpg`, `github.svg`, `linkedin.svg`, `singlethread-icon.png`, singlethread-shot-*.jpg, singlethread-watch-*.png, `site.css`, `wave.svg`.

## Q7: Integration test harness & external-service stubbing

### Findings
- Harness is `src/test/mod.rs`. **No separate `tests/` dir**; HTTP tests are inline `#[cfg(test)] mod tests` in handler/routing files, reusing `crate::test`.
- Startup variants:
  - `start_app()` (`mod.rs:13`) → default `https://api.unsplash.com`, returns `SocketAddr`.
  - `start_app_with(base_url)` (`mod.rs:19`) → returns `(SocketAddr, SqlitePool)` (the widely used variant).
  - `start_app_with_rate_limits(base_url, per_ms, burst)` (`mod.rs:25`) for 429 tests.
  - `start_app_with_metrics()` (`mod.rs:78`) spawns app + separate metrics router, returns both addrs.
  - All bind random port `127.0.0.1:0`, serve via `axum::serve` in a spawned task with `into_make_service_with_connect_info::<SocketAddr>` (`mod.rs:57-60`).
- `serve_app` state (`mod.rs:33-75`): hardcoded `Env` (`database_url: "sqlite::memory:"`, `unsplash_api_key: "test-key"`, `enable_sentry:false`, passed per_ms/burst) at `mod.rs:34-41`; DB `db::init` + `sqlx::migrate!("./migrations")` (`mod.rs:42-45`) then `seed_wallpaper`; `AppState` hand-built (`mod.rs:48-55`) with `http: reqwest::Client::new()` and `unsplash_base_url` override, then wrapped with global rate limit.
- Outbound stubbing: **custom axum stub, not wiremock/mockito**. `start_unsplash_stub(status)` (`mod.rs:165-`) returns `UnsplashStub { base_url, call_count: Arc<AtomicUsize> }` (`struct` at `mod.rs:157-163`), serving `GET /photos/random` with canned success JSON or the given status, counting calls. Tests route traffic via `start_app_with(&stub.base_url)`. Malformed-payload stubs built inline as one-off axum routers (e.g. `unsplash/json.rs:150-167` returns JSON missing `user.links`).
- Seed helpers: `seed_wallpaper(db)` (`mod.rs:135-144`) inserts a fresh `unsplash_pictures` row so page tests never hit network (called by harness at `mod.rs:46`); `seed_wallpaper_no_url(db)` (`mod.rs:147-154`) inserts name/no photographer_url. Other tests seed inline via `sqlx::query("INSERT ...")` against the returned pool.
- Runner conventions:
  - HTTP integration tests = `#[tokio::test]` + `start_app*` + `test_client()` (`reqwest::Client::new()`, `mod.rs:129`). `#[sqlx::test]` NOT used in the HTTP harness.
  - Data-layer unit tests = `#[sqlx::test]` taking an injected `pool: SqlitePool` (sqlx auto-applies `./migrations`): `picture.rs:100,111,134,140,160`.
  - Pure unit tests = `#[test]`/`#[tokio::test]` in `env.rs` (mutex-serialized env tests), `rate_limit.rs`, `error.rs`, `assets.rs`, `templates.rs`.
  - `src/test/arkitect.rs` is a separate `rust_arkitect` architecture-layering test, not part of the HTTP harness.
- Happy-path examples: `home/web.rs:28-67` (200 + HTML assertions); `unsplash/json.rs:59-102` (cache hit, no upstream); `dump/web.rs:122-145` (POST/GET round-trip).
- Sad-path examples: `unsplash/json.rs:136-143` (upstream 500 → 502 "bad gateway"); `home/web.rs:108-124` (upstream fails but page still renders, decorative fallback); `road.rs:145-153` (`health_returns_500_when_database_is_dead`, closes pool via `pool.close().await`); `routes.rs:120-140`, `dump/web.rs:34-70` (429 + retry-after).

## Cross-Cutting Observations
- Layering enforcement: `app`/`infra`/`interface`/`domain` separation is checked by `src/test/arkitect.rs` (rust_arkitect). `infra` is reachable from `interfaces` only via sanctioned re-exports in `src/app/state.rs` (`pub use crate::infra::unsplash::fetch_random;`, `pub use crate::infra::metrics::AppMetrics;`) — see arkitect deps allow-list (`arkitect.rs:11-30`).
- Single error chokepoint: `WebError` (`src/app/error.rs`) is used by both handlers (via `?` + `From`) and middleware (rate-limit `rate_limit_error_response` builds `WebError::TooManyRequests`). Client-fault variants (`External`, `TooManyRequests`) are logged but **not** Sentry-captured.
- Decorative-fallback convention: `wallpaper_context` swallows all Unsplash failures to empty strings (page.rs:23-27) so HTML pages never 502; only JSON endpoints surface `WebError::External`.
- Metrics convention: only full-HTML page handlers call `inc_page_view`; JSON/dump handlers don't.
- Immutable static caching + content-hash query-string is applied consistently via `asset_url` (server-side template fn) + `ServeCache` with `Cache-Control: public,max-age=31536000,immutable`.

## Open Areas
- Exact axum version-specific rejection codes for extractor failures were stated as 400 (Path) and 422 (Json) based on axum 0.8 defaults; the repo defines no custom rejection handling, and only the dump malformed-JSON case is covered by a test (`dump/web.rs:143`) — the Path/Form rejection paths are untested.
- Whether Fly secrets are set via dashboard vs `fly secrets set` is not verifiable from the repo; `fly.toml` and `.github/workflows/flydeploy` contain no secret wiring.
- The `tiered_routes` per-tier stores are not pruned (no `prune_loop`), but this has no observable test coverage one way or the other.