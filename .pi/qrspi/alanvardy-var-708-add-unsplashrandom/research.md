# Research Findings

## Q1: Full request flow for `GET /unsplash`

### Findings
- **Route registration + tier**: `src/interfaces/routes.rs:31-34` builds `unsplash_tier` via `rate_limit::tiered_routes(Router::new().route("/unsplash", get(handlers::unsplash::json::index)), UNSPLASH_TIER_PER_MS, UNSPLASH_TIER_BURST)`. It is merged into the main router at `routes.rs:42` (`.merge(unsplash_tier)`).
- **Tier budget constants**: `src/app/rate_limit.rs:75-76` — `UNSPLASH_TIER_PER_MS: u64 = 200` (≈5 req/s sustained) and `UNSPLASH_TIER_BURST: u32 = 5`.
- **Handler**: `src/interfaces/handlers/unsplash/json.rs:7-9` — `pub async fn index(State(state): State<AppState>) -> Result<Json<Picture>, WebError>` calls `picture::current(&state).await?`. Registered for the module at `src/interfaces/handlers/unsplash/mod.rs:1` and parent `src/interfaces/handlers/mod.rs:6`.
- **App layer**: `src/app/picture.rs:12-25` `current()` — if `latest(&state.db)` returns a row and `!picture.is_stale()`, return it (no upstream call); else `fetch_random(&state.http, &state.unsplash_base_url, &state.env.unsplash_api_key)` then `create(&state.db, &picture)`. The `fetch_random` re-export is at `picture.rs:4` (`pub use crate::infra::unsplash::fetch_random;`) — `interfaces` may only reach infra through `app`.
- **Upstream call (infra)**: `src/infra/unsplash.rs:29-61` `fetch_random` issues `client.get("{base_url}/photos/random").query([("query","nature")]).header("Authorization","Client-ID {api_key}")`. Non-2xx → `UnsplashError` (`unsplash.rs:42-45`); parse failure → `UnsplashError` (`unsplash.rs:46-49`); success maps `body.urls.regular` → `Picture.url`, `body.user.name` → `Picture.photographer`, `created_at: String::new()` at `unsplash.rs:54-61`.
- **Domain type**: `src/domain/picture.rs:9-13` `struct Picture { url, photographer, created_at }` (derives `Serialize`, `sqlx::FromRow`). `is_stale()` at `picture.rs:16-21` parses `created_at` as `%Y-%m-%d %H:%M:%S` and compares against `MAX_AGE_HOURS = 6` (`picture.rs:4`); unparseable → `true`.
- **Global limiter (outer)**: `src/main.rs:40` wraps `routes()` in `rate_limit::with_global_limit(router, rate_limit_per_ms, rate_limit_burst)`; env-derived at `env.rs:21-22`. Served with `ConnectInfo` at `main.rs:43-44`.
- **State/wiring**: `src/app/state.rs:12-18` `AppState { db: SqlitePool, env: Arc<Env>, http: reqwest::Client, unsplash_base_url: Arc<str> }`. Built in `main.rs` from `UNSPLASH_BASE_URL = "https://api.unsplash.com"` (`main.rs:10`) and `env.unsplash_api_key` (`env.rs:17,30`).
- **Error translation**: `UnsplashError → WebError::External` (`src/app/error.rs:30-33`); `WebError::External` → HTTP 502 "bad gateway" (`error.rs:50-53`). Rate-limit `GovernorError::TooManyRequests` → `WebError::TooManyRequests` → HTTP 429 with `retry-after` ("too many requests") at `error.rs:55-61`, wired via `rate_limit_error_response` (`rate_limit.rs:43-64`).
- **Migration**: `migrations/0003_unsplash_pictures.sql:1-6` creates the table.

Full chain, in order:
```
GET /unsplash
  global limiter   with_global_limit  (main.rs:40, rate_limit.rs:96-112)
  tier limiter     tiered_routes      (routes.rs:31-34 → rate_limit.rs:116-125)
  handler          json::index        (routes.rs:32 → json.rs:7)
    app layer      picture::current   (picture.rs:12)
      DAO          latest             (picture.rs:27-33)  SELECT ... LIMIT 1
      freshness    Picture::is_stale  (domain/picture.rs:16)
      infra        fetch_random       (picture.rs:3 → infra/unsplash.rs:29-35)
      DAO          create             (picture.rs:36-45)  INSERT ... RETURNING
  response         Json<Picture>
```

## Q2: Data-access layer over `unsplash_pictures`

### Findings
- **Schema**: `migrations/0003_unsplash_pictures.sql:1-6` — `id INTEGER PK AUTOINCREMENT, url TEXT NOT NULL, photographer TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now'))`.
- **Row type**: `src/domain/picture.rs:9-13` `Picture` derives `sqlx::FromRow`; the three struct fields (`url`, `photographer`, `created_at`) map by column name. There is NO `id` field — `id` is never selected/bound, only used as the `ORDER BY` key in `latest()`.
- **Row-selection query**: `src/app/picture.rs:27-34` `latest(pool)` uses `sqlx::query_as::<_, Picture>("SELECT url, photographer, created_at FROM unsplash_pictures ORDER BY id DESC LIMIT 1")` + `.fetch_optional(pool)` → `Option<Picture>`.
- **Insert query**: `src/app/picture.rs:36-45` `create(pool, picture)` uses `sqlx::query_as::<_, Picture>("INSERT INTO unsplash_pictures (url, photographer) VALUES (?, ?) RETURNING url, photographer, created_at")` with two `.bind()` and `.fetch_one()`. `created_at` is not bound — defaulted by SQLite `DEFAULT (datetime('now'))` and read back via `RETURNING`.
- **Orchestrator**: `current()` at `src/app/picture.rs:12-25` combines `latest()` + `is_stale()` (serve cache) else `fetch_random` + `create()`; returns `web::error`-typed result.
- **Mapping mechanisms**: (1) `sqlx::FromRow` derive for both row-selecting/queries (`query_as`); (2) manual `Picture { .. }` construction in `infra/unsplash.rs:54-61` from parsed JSON with `created_at: String::new()` (overwritten by DB on insert).
- **Test-only queries** (all `#[cfg(test)]`, no `id`):
  - count scalar `sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM unsplash_pictures")` + `.fetch_one` at `json.rs:56-73` (and 97-104, 129-136, 178-184);
  - `DELETE FROM unsplash_pictures` in `clear_pictures` `json.rs:20-22`; seed inserts `json.rs:33-42`, `mod.rs:135-140`.

### Callers of DAO
- `src/interfaces/handlers/unsplash/json.rs:7-9` (`index` → `current`).
- `src/interfaces/handlers/home/web.rs:12` and `singlethread/web.rs:12` both call `picture::current(&state).await.ok().map(|p| p.url)` for page wallpaper (errors swallowed).

## Q3. Route rate-limit tiers + global limiter wiring

### Findings
- **Tier constants**: `src/app/rate_limit.rs:73-76` — `DUMP_TIER_PER_MS = 1_000`/`DUMP_TIER_BURST = 3`, `UNSPLASH_TIER_PER_MS = 200` (5 req/s sustained)/`UNSPLASH_TIER_BURST = 5`. Comment: "Stricter budgets for expensive endpoints. Policy lives in code, not config."
- **Tier builder**: `tiered_routes(limited, per_ms, burst)` at `rate_limit.rs:116-125` — builds `GovernorConfigBuilder` with `key_extractor(FlyClientIpKeyExtractor)`, `per_millisecond(per_ms)`, `burst_size(burst)`, `use_headers()`, then `limited.layer(GovernorLayer::new(cfg).error_handler(rate_limit_error_response))`. **Does not spawn a pruner** (only `with_global_limit` spawns `prune_loop`, `rate_limit.rs:51`).
- **Tier nesting**: `routes.rs:31-34` wraps a `Router` containing exactly one `GET /unsplash` route, then merged at `routes.rs:42`. Merge composes the tier's `GovernorLayer` inside the final router returned by `routes()`.
- **Global wrapper**: `with_global_limit(router, per, burst)` at `rate_limit.rs:96-113` is applied **after** `routes()` returns (`src/main.rs:40`; test mirror at `src/test/mod.rs:60-64`), wrapping all merged tiers in an outer per-IP `GovernorLayer` and spawning a `prune_loop` at `rate_limit.rs:109`.
- **Budgets are independent**: each tier builds its own `GovernorConfigBuilder`/`SharedRateLimiter`; comment in `routes.rs:17-30` states budgets "do not pool across tiers". `/unsplash` has its own 5-token store separate from global.
- **Per-IP keying**: `FlyClientIpKeyExtractor` (`rate_limit.rs:19-42`) prefers the `fly-client-ip` header, falls back to `ConnectInfo<SocketAddr>` peer IP; `X-Forwarded-For` deliberately ignored (comment `rate_limit.rs:30-32`).
- **429 mapping (shared)**: `rate_limit_error_response` (`rate_limit.rs:54-59`) maps `GovernorError::TooManyRequests` → `WebError::TooManyRequests` with `wait_time` → `retry-after` header + `"too many requests"` body; all else → 500.

## Q4. Test harness and patterns for unsplash

### Findings
- **Harness location**: `src/test/mod.rs` (shared integration harness; declares `mod arkitect;` line 1). Inline `#[cfg(test)] mod tests` across source files including `src/interfaces/handlers/unsplash/json.rs:11`.
- **App boot**: `start_app()` (`mod.rs:13-16`) binds random port; `start_app_with(unsplash_base_url)` (`mod.rs:19-23`) returns `(SocketAddr, SqlitePool)`; `start_app_with_rate_limits(per, burst)` (`mod.rs:25-31`) for tight-limit tests; `serve_app` (`mod.rs:35-64`) builds `sqlite::memory:` DB, runs `sqlx::migrate!`, seeds wallpaper, constructs `AppState`, spawns axum on `127.0.0.1:0`, applies `with_global_limit`. `start_app_with_metrics()` at `mod.rs:78-90`. `test_client()` = shared `reqwest::Client` (`mod.rs:129`).
- **Seed/clear**: `seed_wallpaper(db)` (`mod.rs:135-140`) `INSERT INTO unsplash_pictures (url, photographer) VALUES ('https://example.com/wallpaper.jpg',...); run by `serve_app` (`mod.rs:47`). Unsplash tests then call `clear_pictures(&db)` (`json.rs:20-22`, `DELETE FROM unsplash_pictures`) before seeding custom row (`json.rs:33-42`).
- **Stub server**: `start_unsplash_stub(status)` (`mod.rs:153-183`) — axum `Router` with `GET /photos/random`, canned JSON `{"urls":{"regular":"https://images.example.com/photo.jpg"},"user":{"name":"Stub Photographer"}}` on success (non-success returns 500 `INTERNAL_SERVER_ERROR` verbatim). Calls tracked via `Arc<AtomicUsize>`. Tests pass `&stub.base_url` into `start_app_with` so the app's `unsplash_base_url` points at the stub.
- **Assertion pattern** — status + body: e.g. `json.rs:66-69` `assert_eq!(res.status(), 200)` then `res.text()` + `body.contains(...)`; exact body for failures `json.rs:154-155` `assert_eq!(body, "bad gateway")` (status 502). Also checks `content-type` `json.rs:45`.
- **Row assertions** via `query_scalar::<_, i64>("SELECT COUNT(*)...")` (`json.rs:73`, `json.rs:104`, etc.) and stub `call_count` (`json.rs:78`, `json.rs:110` for `==0`/`==1`).
- **Rate-limit test**: `unsplash_tier_trips_while_global_budget_stays_open` (`json.rs:191-230`) uses `start_app_with_rate_limits(&stub, 1, 1_000_000)` (global effectively open), fires 20 concurrent GETs, asserts ≥1 `200`, ≥5 `429` with `retry-after` + `"too many requests"`, and `stub.call_count < 20`.

## Q5. Randomness / random-selection primitives available

### Findings
- **Rust dependency set (`Cargo.toml`)**: NO `rand`/`getrandom`/`fastrand`/`uuid` etc. as direct deps (`Cargo.toml:11-18` includes `sqlx`, `tower_governor`, `governor`). `rand` appears only as transitive (e.g. `rand` in Cargo.lock:2329/2339, `rand_chacha:2350`, `rand_core:2360`, `rand_pcg:2375`, `getrandom:1050/1063/1077`, `fastrand:976`) — not available to `src/` code without adding it.
- **SQLite**: `RANDOM()` / `ORDER BY RANDOM()` is **not used anywhere** in `src/` or `migrations/`; the only `ORDER BY` is the deterministic `ORDER BY id DESC LIMIT 1` (`src/app/picture.rs:29`).
- **Only active randomness**: the external Unsplash `GET /photos/random` API call (`src/infra/unsplash.rs:35-37`), plus OS-assigned ephemeral test ports (`TcpListener::bind("127.0.0.1:0")` at `src/test/mod.rs:56,103,174`). No local RNG is invoked in app code.
- **Note**: `governor` (a direct dependency, used for rate limiting) internally uses `rand`, but that is not exposed to application code.

## Cross-Cutting Observations
- **Layering rule**: `interfaces` may only reach the Unsplash fetch through the `app` layer re-export (`picture.rs:3-4`): infra implements `fetch_random`, app re-exports it, handlers call `picture::current`.
- **Error chokepoint**: every handler-produced error flows through `WebError::into_response` (`src/app/error.rs:25-66`); both upstream failures (`External`→502) and rate-limit trips (`TooManyRequests`→429) route through it. No bare status-code tuples.
- **Two-layer rate limiting**: a single global per-IP limiter wraps the whole router, and per-tier (unsplash/dump) limiters nest inside it; each owns an independent budget.
- **Fusion pattern used consistently**: transactions + `current()` preferring cache; wallpaper flows (`home`, `singlethread`) use `picture::current(...).ok()` and degrade gracefully.

## Open Areas
- The exact live behavior of `governor`'s `use_headers()`/`X-Forwarded-For` interaction under Fly is not verified at the code level (only documented in comments).
- Whether `created_at` format string `%Y-%m-%d %H:%M:%S` exactly matches SQLite `datetime('now')` output (it appears to, but is not asserted outside tests).