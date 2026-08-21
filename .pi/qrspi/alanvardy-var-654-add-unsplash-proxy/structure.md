# Structure Outline

## Approach
Build the Unsplash proxy lazily: a JSON endpoint backed by an SQLite cache
table that is refreshed on request when older than 6 hours, with the API key
loaded panic-fast at startup via a new `Env` module (pattern from `../api`).
Sliced vertically so each phase ends with a working, testable `/unsplash`
endpoint.

---

## Phase 1: Config, dependencies, and a stub JSON endpoint

Delivers `GET /unsplash` returning real JSON (placeholder values), with
`UNSPLASH_API_KEY` loaded fail-fast at startup and threaded through
`AppState`. Establishes the first JSON response and the secrets precedent.

**Files**: `Cargo.toml`, `.env_template`, `src/app/env.rs` (new),
`src/app/state.rs`, `src/app/mod.rs`, `src/main.rs`,
`src/interfaces/routes.rs`, `src/interfaces/handlers/mod.rs`,
`src/interfaces/handlers/unsplash/{mod.rs,web.rs}` (new)

**Key changes**:
- `reqwest = { version = "0.13", features = ["json"] }`, `serde`,
  `serde_json` promoted to prod deps
- `struct Env { unsplash_api_key: String }` with `Env::init() -> Env`
  (panics on missing var); called first thing in `main`
- `AppState { templates, db, unsplash_api_key: Arc<str>, unsplash_base_url: Arc<str> }`
  — removes `#[allow(dead_code)]` on `db`; base URL defaults to
  `https://api.unsplash.com` but is overridable for tests (mock strategy)
- `pub async fn index(State(state): State<AppState>) -> Result<Json<Value>, WebError>`
  — returns stub `{ "url": ..., "photographer": ..., "created_at": ... }`

**Verify**: `cargo test` — new route test asserts status 200,
`content-type: application/json`, body contains all three keys; existing
tests still pass. Manual: `cargo run` with key unset panics with clear
message; with key set, `curl localhost:3000/unsplash` returns JSON.

---

## Phase 2: SQLite cache table and read-through endpoint

Delivers persistence: migration creates `unsplash_pictures`; the endpoint
now serves the newest stored row (or 404-equivalent empty state when none).
No upstream call yet — seeded rows are served directly.

**Files**: `migrations/0002_unsplash_pictures.sql` (new, via
`sqlx migrate add`), `src/interfaces/handlers/unsplash/web.rs`

**Key changes**:
```sql
CREATE TABLE unsplash_pictures (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  url TEXT NOT NULL,
  photographer TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```
- `async fn latest_picture(pool: &SqlitePool) -> Result<Option<Picture>, WebError>`
- `async fn insert_picture(pool: &SqlitePool, picture: &Picture) -> Result<Picture, WebError>`
- `struct Picture { url: String, photographer: String, created_at: String }`
  (`Serialize`, used as both row and JSON response shape)
- Handler: query latest → `Some` → `Json(picture)`; `None` → 404 via
  `WebError::NotFound` (staleness/freshness logic deferred to Phase 3)

**Verify**: `cargo test` — `#[sqlx::test]` asserting table exists post-migration;
integration test seeding a row (via exposed helper or direct pool query in
test) then hitting `/unsplash` returns it. Manual: `sqlx migrate run`
locally, insert a row with `sqlite3`, curl shows it.

---

## Phase 3: Live Unsplash fetch with 6-hour lazy refresh

Completes the feature: on request, if no row or the newest is older than 6
hours, fetch `GET {base_url}/photos/random?query=nature` with
`Authorization: Client-ID <key>`, parse, insert, and return; otherwise serve
cached row. Upstream failures map to a new 502 error variant.

**Files**: `src/app/error.rs`, `src/interfaces/handlers/unsplash/{web.rs,unsplash.rs}` (new)

**Key changes**:
- `WebError::External(String)` → HTTP 502, `eprintln!` logging like other variants
- `struct RandomPhotoResponse { urls: RandomPhotoUrls, user: RandomPhotoUser }`
  (+ nested `regular: String`, `name: String`) — serde Deserialize, matches
  live API shape (verify against one real call during dev)
- `async fn fetch_random(client: &Client, base_url: &str, api_key: &str)
   -> Result<Picture, WebError>` — non-2xx or parse failure → `External`;
  staleness compared Rust-side against `created_at` (single clock source)
- Handler flow: `latest_picture` → fresh (< 6h)? serve : `fetch_random` +
  `insert_picture` → serve

**Verify**: `cargo test` — integration test pointing `unsplash_base_url` at a
local stub server (tiny axum spawn in test helpers): stale/no row triggers
fetch + insert (row count grows), fresh row does not call upstream, upstream
500 maps to response 502. Manual: run with real key, curl twice quickly
(second is instant/cached), again after `UPDATE unsplash_pictures SET
created_at = datetime('now', '-7 hours')` triggers refetch.

---

## Testing Checkpoints

After **Phase 1**: app refuses to start without `UNSPLASH_API_KEY`; `/unsplash`
returns valid JSON with the three keys; all prior tests green.
After **Phase 2**: migration applies cleanly (`#[sqlx::test]`); endpoint
serves a seeded DB row and 404s when the table is empty.
After **Phase 3**: full lazy-refresh behavior verified against a stub
upstream (fetch-on-miss, cache-hit under 6h, 502 on upstream failure);
live smoke test optional before deploy. Remember `fly secrets set
UNSPLASH_API_KEY=...` at deploy time.

**Note on slicing**: the design's mock-strategy risk (CI has no real key) is
resolved by the base-url injection added to `AppState` in Phase 1 and used in
Phase 3's tests.
