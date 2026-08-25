# Research Findings

Endpoint: `GET /unsplash`. Scope: handler, orchestrator, domain type, upstream
client, persistence, and tests.

## Q1: Full data flow for `/unsplash`

### Findings
- **Route wiring**: The route is registered in `src/interfaces/routes.rs:31-34`
  as part of a rate-limited "tier": `Router::new().route("/unsplash", get(handlers::unsplash::json::index))`, wrapped by `tiered_routes` with `UNSPLASH_TIER_PER_MS = 200` (5 calls/sec sustained) and `UNSPLASH_TIER_BURST = 5` (`src/app/rate_limit.rs:75-76`). The tier is merged into the main router at `routes.rs:42`.
- **Handler**: `src/interfaces/handlers/unsplash/json.rs:7-8` — `index(State(state): State<AppState>) -> Result<Json<Picture>, WebError>` calls `picture::current(&state).await?` and wraps the result in `Json<Picture>`.
- **Orchestrator**: `src/app/picture.rs:12-25` — `current()`:
  1. `latest(&state.db).await?` reads the newest cached row (`picture.rs:27-33`).
  2. If a row exists **and** `!picture.is_stale()` (`picture.rs:13`), returns it without network.
  3. Otherwise calls `fetch_random(&state.http, &state.unsplash_base_url, &state.env.unsplash_api_key)` (`picture.rs:18-23`), then `create(&state.db, &picture)` to persist, and returns the DB row.
- **AppState inputs**: `src/app/state.rs:20-28` — `AppState` carries `db: SqlitePool`, `http: reqwest::Client`, `env: Arc<Env>` (holding `unsplash_api_key`), and `unsplash_base_url: Arc<str>` (test-overridable). `picture::current` reaches infra only through the sanctioned re-export `pub use crate::infra::unsplash::fetch_random` at `src/app/picture.rs:3`.
- **Upstream client**: `src/infra/unsplash.rs:29-58` — `fetch_random` GETs `{base_url}/photos/random?query=nature` with `Authorization: Client-ID {api_key}`, deserializes the body, and builds a `Picture` with `url = urls.regular`, `photographer = user.name`, and `created_at = String::new()` (left empty; the DB fills it on insert, `infra/unsplash.rs:55-57`).
- **JSON serialization**: `Json<Picture>` from axum serializes the `Picture` struct via its `Serialize` derive (`src/domain/picture.rs:9`) and sets `content-type: application/json`. Axum's `Json` is the response body for the success arm; the error arm is handled by `WebError::into_response` (see Q4).
- **Fields carried by `Picture` and where populated**:
  - `url` — from Unsplash `urls.regular` (`infra/unsplash.rs:55`) or read from the `url` column (`app/picture.rs:29`).
  - `photographer` — from Unsplash `user.name` (`infra/unsplash.rs:56`) or the `photographer` column (`app/picture.rs:29`).
  - `created_at` — only ever set by the DB: `String::new()` on fetch (`infra/unsplash.rs:57`), then overwritten by `RETURNING created_at` in `create()` (`app/picture.rs:38-39`) or read in `latest()` (`app/picture.rs:29`).
- **Pattern observed**: app layer (`picture.rs`) is the single chokepoint between `interfaces` (handler) and `infra` (unsplash client); `db` and `http` come from shared `AppState`, and per-request no state beyond `AppState` is threaded in.

## Q2: `Picture` domain type and persistence

### Findings
- **Struct**: `src/domain/picture.rs:9-13` — `#[derive(Serialize, sqlx::FromRow)] pub struct Picture { pub url: String, pub photographer: String, pub created_at: String }`. Derives only `Serialize` and `sqlx::FromRow` — **no `Deserialize`, no `Clone`, no `Debug`**. No `#[serde(rename)]` on any field; JSON field names equal the struct field names.
- **Staleness logic**: `Picture::is_stale` (`src/domain/picture.rs:16-22`) parses `created_at` as `%Y-%m-%d %H:%M:%S` and returns `age > `MAX_AGE_HOURS`(= 6, `picture.rs:4`)`; an unparseable timestamp returns `true` (stale → force refresh).
- **Table schema**: `migrations/0003_unsplash_pictures.sql`:
  ```sql
  CREATE TABLE unsplash_pictures (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL,
    photographer TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
  );
  ```
  `created_at` is `TEXT NOT NULL DEFAULT (datetime('now'))` — the DB (SQLite `datetime('now')`, UTC string) is the only source of `created_at`; `id` is auto-increment. This is the only table for pictures; there is no user/artist column.
- **Queries in `src/app/picture.rs`** (all `sqlx::query_as::<_, Picture>`):
  - `latest()` (`picture.rs:27-33`): `SELECT url, photographer, created_at FROM unsplash_pictures ORDER BY id DESC LIMIT 1` — exact column list `{url, photographer, created_at}`, newest row by `id`.
  - `create()` (`picture.rs:36-41`): `INSERT INTO unsplash_pictures (url, photographer) VALUES (?, ?) RETURNING url, photographer, created_at` — binds `url`, `photographer`; `RETURNING` supplies the DB-generated `created_at`. Column list `{url, photographer, created_at}`.
- **DB init**: migrations applied in `src/test/mod.rs:45-46` (`sqlx::migrate!("./migrations").run(&db)`); `AppState.db` built via `crate::app::db::init` (`state.rs:24` → used in `test/mod.rs:40`). No raw `INSERT`/`UPDATE` of pictures outside `picture.rs` except test seeds (see Q5).
- **Observation for this branch**: no `artist_url`/photographer-URL field exists anywhere in the struct, table, queries, or handler today.

## Q3: Upstream Unsplash API modeling/parsing

### Findings
- **Internal structs** (`src/infra/unsplash.rs:5-19`), all `#[derive(Deserialize)]`, all **private** to the module:
  - `RandomPhotoResponse { urls: RandomPhotoUrls, user: RandomPhotoUser }` (`unsplash.rs:6-9`).
  - `RandomPhotoUrls { regular: String }` (`unsplash.rs:12-14`).
  - `RandomPhotoUser { name: String }` (`unsplash.rs:17-19`).
- **Request**: `fetch_random` (`unsplash.rs:29-58`) issues `GET {base_url}/photos/random` with query `query=nature` and header `Authorization: Client-ID {api_key}`.
- **Parsing location**: the raw response is passed to `response.json().await` (`unsplash.rs:50`), deserializing into `RandomPhotoResponse`. Field extraction: `url: body.urls.regular`, `photographer: body.user.name` (`unsplash.rs:55-56`).
- **Missing/malformed behavior**:
  - Transport errors (DNS/connect/request) → `UnsplashError("unsplash request failed: {e}")` (`unsplash.rs:39-41`).
  - Non-2xx status → `Err(UnsplashError(format!("unsplash returned status {}", response.status())))`, returned before any parsing (`unsplash.rs:42-45`).
  - Malformed or missing fields → serde deserialization failure in `response.json()` → `Err(UnsplashError("unsplash response parse failed: {e}"))` (`unsplash.rs:50-52`). Because fields are non-optional `String` with no `#[serde(default)]`, a missing `urls`/`user`/`regular`/`name` makes the whole parse fail (no partial data, no defaulting).
  - On success returns a `Picture` with `created_at` empty, to be filled by the DB (`unsplash.rs:55-57`).
- **Error type**: `pub struct UnsplashError(pub String)` (`unsplash.rs:23-24`), marked `#[derive(Debug)]` only; doc comment states it maps to `WebError::External` (HTTP 502) at the app layer (`unsplash.rs:20-22`).

## Q4: How the handler shapes the JSON response and errors

### Findings
- **Success response**: `Json<Picture>` (`src/interfaces/handlers/unsplash/json.rs:7`) implements axum's `IntoResponse`; it serializes `Picture` (all three fields) and emits `content-type: application/json`. The handler emits `Ok(Json(picture::current(&state).await?))`.
- **Error path**: `?` converts any error into `WebError` via `From`. Relevant conversions:
  - `From<UnsplashError> for WebError` → `WebError::External(err.0)` (`src/app/error.rs:30-33`).
  - `From<sqlx::Error> for WebError` → `WebError::Database` (`error.rs:26-28`).
  - `picture.rs` `current()` returns `Result<Picture, WebError>`, so `?` on `latest(...)`'s `sqlx::Result` is auto-converted (Database), `?` on `fetch_random`'s `UnsplashError` uses the `UnsplashError → External` mapping (`error.rs:30-33`), and `create`'s sqlx error → Database.
- **`IntoResponse` mapping** (`webError` enum at `error.rs:10-17`, impl at `error.rs:37-61`):
  - `External(_)` → HTTP 502 `BAD_GATEWAY`, body `"bad gateway"`, logs at ERROR, **no Sentry capture** (`error.rs:50-53`). This is the arm upstream fetch/parse failures land on.
  - `Database` / `Template` → HTTP 500, body `"internal server error"`, logs + `sentry::capture_error` (`error.rs:41-48`).
  - `NotFound` → 404 `"not found"` (`error.rs:39`).
  - `TooManyRequests { retry_after_secs }` → 429 with `retry-after` header and `"too many requests"` body (`error.rs:55-60`).
- **Handler never returns bare status tuples**; all error responses flow through `WebError::into_response` (matches repo convention in AGENTS.md).

## Q5: Test conventions for `/unsplash`

### Findings
- **Harness surface** (`src/test/mod.rs`):
  - `start_app()` (`test/mod.rs:13-15`) → app pointed at real `https://api.unsplash.com` but a seeded cache row means no network. Returns bound `SocketAddr`.
  - `start_app_with(unsplash_base_url)` (`test/mod.rs:19-22`) → returns `(SocketAddr, SqlitePool)` so tests can seed/clear rows; app's `unsplash_base_url` is overridden to the stub.
  - `serve_app()` (`test/mod.rs:32-76`) builds `Env` with `unsplash_api_key: "test-key"`, in-memory SQLite (`sqlite::memory:`), applies `sqlx::migrate!("./migrations")`, calls `seed_wallpaper`, constructs `AppState`, wraps router in `with_global_limit`, spawns server on 127.0.0.1:0.
  - `start_app_with_rate_limits(url, per_ms, burst)` (`test/mod.rs:25-28`) for 429 concurrency tests.
  - `test_client()` (`test/mod.rs:129-131`) → `reqwest::Client::new()`.
  - `seed_wallpaper(&db)` (`test/mod.rs:135-143`) → inserts a fresh `unsplash_pictures` row (`url = 'https://example.com/wallpaper.jpg'`, `photographer = 'Wallpaper Photographer'`) so page handlers never hit the network. This is the shared auto-seed; `/unsplash` inline tests therefore **clear the table first**.
- **Upstream stub**: `start_unsplash_stub(status)` (`test/mod.rs:153-186`) spins a local axum router serving `GET /photos/random` that returns `call_count`-incremented canned JSON `{"urls":{"regular":"https://images.example.com/photo.jpg"},"user":{"name":"Stub Photographer"}}` on success status, or the given non-success status verbatim (to simulate 500s). Exposes `base_url` and `call_count: Arc<AtomicUsize>`.
- **Clearing**: inline tests call `clear_pictures(&db)` (defined `src/interfaces/handlers/unsplash/json.rs:20-25`) which runs `DELETE FROM unsplash_pictures`.
- **Assertion style** (all in `json.rs` `#[cfg(test)] mod tests`): assert both HTTP status **and** body content:
  - `unsplash_pictures_table_exists` (`json.rs:29-37`) — `#[sqlx::test]` checks table presence via `sqlite_master`.
  - `insert_picture_returns_row_with_created_at` (`json.rs:39-54`) — `#[sqlx::test]` exercises `picture::create` → non-empty `created_at`, and `picture::latest` equality.
  - `unsplash_serves_seeded_row` (`json.rs:56-74`) — seeds a row, asserts 200 + `content-type` contains `application/json` + body contains url & photographer.
  - `no_row_triggers_fetch_and_insert` (`json.rs:77-98`) — empty table, stub OK, asserts 200, body contains stub URL/photographer, row count `== 1`, `stub.call_count == 1`.
  - `fresh_row_does_not_call_upstream` (`json.rs:100-126`) — fresh seeded row, asserts 200 + body + `call_count == 0` + `count == 1`.
  - `stale_row_triggers_refetch` (`json.rs:128-159`) — seeds row with `created_at = datetime('now','-7 hours')` (> 6h), asserts 200 + stub body + `call_count == 1` + `count == 2`.
  - `upstream_failure_is_502` (`json.rs:161-173`) — stub 500, asserts 502 + exact body `"bad gateway"`.
  - `second_request_within_window_is_cached` (`json.rs:175-200`) — two GETs, same body, `call_count == 1`, `count == 1`.
  - `unsplash_tier_trips_while_global_budget_stays_open` (`json.rs:202-243`) — `start_app_with_rate_limits`, 20 concurrent GETs, expects mix of 200 and 429 (with `retry-after` + body `"too many requests"`), and `stub.call_count < 20` (tier throttles before upstream).
- **Injection pattern** (from `test/mod.rs:165-166`): `#[sqlx::test]` uses a per-test in-memory DB with migrations auto-applied; integration `#[tokio::test]` use `start_app_with` and raw `sqlx::query` to seed/clear.

## Cross-Cutting Observations
- **Layering rule enforced by comments**: `interfaces` reaches infra types only through `app` — `fetch_random` is re-exported at `app/picture.rs:3`; `AppState` re-exports `AppMetrics` at `state.rs:8`.
- **`created_at` is single-sourced by the DB**; upstream/domain set it to `String::new()` and persistence `RETURNING` overrides it. Any new field following this pattern would be DB-populated via `RETURNING` and must be added to both `SELECT` and `INSERT ... RETURNING` column lists.
- **Column lists appear in three places and must stay in sync**: `latest()` SELECT (`picture.rs:29`), `create()` INSERT+RETURNING (`picture.rs:38-39`), and the table DDL (`migrations/0003...`). Adding a column means touching all three plus the `Picture` struct.
- **All picture authors** write through `picture::create`; the only hand-written `INSERT into unsplash_pictures` statements are in test seeding (`test/mod.rs:137`, `json.rs` tests) — tests bypass `create()` deliberately to control `created_at`.
- **Rate limiting**: `/unsplash` sits behind a per-IP tier (burst 5, 200/ms refill) nested under a global limiter; 429 responses carry `retry-after`. Stub `call_count` lets tests distinguish tier-throttle (no upstream call) from upstream errors.
- **Errors**: every handler error routes through `WebError::into_response`; the Unsplash-specific arm is `External` (502, no Sentry), distinguishing it from db/template 500s.

## Open Areas
- No test exercises a malformed *parse* failure (non-JSON / missing fields) from the stub — `start_unsplash_stub` only returns well-formed JSON; the parse-failure branch (`unsplash.rs:50-52`) has no direct integration test, only the non-2xx path (`upstream_failure_is_502`).
- `RandomPhotoResponse` is defined to consume exactly two sub-objects and three leaf strings; whether the upstream `/photos/random` payload contains `user.links.html` or similar (relevant to an "artist url" feature) is not represented or modeled anywhere in the current code.
- The unused-import warning for `use serde::Serialize` and the `#[allow(dead_code)]` on `WebError::NotFound` indicate coverage-hardening conventions but no behavior gap.
