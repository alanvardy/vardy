# Structure Outline

## Approach
Add a `dump` handler area following the existing two-file pattern (`dump/mod.rs` + `dump/web.rs`), backed by a new `dumps` SQLite table and compile-time-checked sqlx macros (`.sqlx/` committed). Slice vertically: Phase 1 delivers the **GET** endpoint end-to-end (migration → pool/harness → SELECT handler → route), Phase 2 adds the **POST** write path, Phase 3 hardens offline builds and docs. After Phase 1 the app is independently useful (unknown keys return `[]`); after Phase 2 the full contract works.

---

## Phase 1: Dumps table + `GET /dump/{key}` returning `[]`

Delivers the read path across every layer: migration creates the table, the shared HTTP test harness learns to migrate, and the first JSON route serves an (always-empty-for-now) list.

**Files**: `migrations/<ts>_create_dumps.sql` (via `sqlx migrate add create_dumps`), `src/app/state.rs`, `src/test/mod.rs`, `src/interfaces/handlers/mod.rs`, `src/interfaces/handlers/dump/mod.rs`, `src/interfaces/handlers/dump/web.rs`, `src/interfaces/routes.rs`, `.sqlx/` (new), `Cargo.toml`

**Key changes**:
```rust
// migration: CREATE TABLE IF NOT EXISTS dumps (
//   id INTEGER PRIMARY KEY AUTOINCREMENT, key TEXT NOT NULL, body TEXT NOT NULL)
//            + CREATE INDEX ... ON dumps(key)

pub struct DumpEntry { pub id: i64, pub body: serde_json::Value }        // web.rs

pub async fn index(Path(key): Path<String>, State(state): State<AppState>)
    -> Result<Json<Vec<DumpEntry>>, WebError>                            // web.rs

// start_app(): add sqlx::migrate!("./migrations").run(&pool).await before serving
```
- Add `serde_json` dependency (axum `json` feature already implicitly on).
- Remove `#[allow(dead_code)]` from `AppState.db` — now used by production code.
- First `query_as!` usage → run `cargo sqlx prepare` and commit `.sqlx/`.

**Verify**: `cargo nextest run` passes, including new test `get_unknown_key_returns_empty_list` (real HTTP: `GET /dump/nope` → 200, `content-type: application/json`, body `[]`). Manual: `sqlx migrate run && cargo run`, then `curl localhost:3000/dump/x` → `[]`.

---

## Phase 2: `POST /dump/{key}` storing JSON payloads

Completes the contract end-to-end: JSON bodies persist via INSERT and flow back through the existing GET as `[{"id": ..., "body": ...}]`. Multiple POSTs accumulate.

**Files**: `src/interfaces/handlers/dump/web.rs`, `src/interfaces/routes.rs`

**Key changes**:
```rust
pub async fn create(Path(key): Path<String>, State(state): State<AppState>,
    Json(body): Json<serde_json::Value>) -> Result<StatusCode, WebError>
    // INSERT INTO dumps (key, body) VALUES (?, ?) via query!,
    // body serialized with serde_json::to_string
```
- Register `.route("/dump/{key}", post(handlers::dump::web::create))`.
- Extend `DumpEntry` deserialization: `body` parsed back via `serde_json::from_str` (already shaped for this in Phase 1).

**Verify**: `cargo nextest run` passes, including new tests: `post_stores_and_get_returns_it` (POST arbitrary JSON → 2xx; GET → `[{"id":1,"body":<original>}]`), `multiple_posts_accumulate`, `post_invalid_json_rejected` (422). Manual: `curl -X POST -d '{"a":1}' localhost:3000/dump/x` then `curl localhost:3000/dump/x`.

---

## Phase 3: Offline-build hardening + docs

*Note: this phase is deliberately not a feature slice — it's the CI/deployability gate the design flags as its top risk (uncommitted `.sqlx/` breaks `SQLX_OFFLINE=true` Docker builds). Kept separate so Phases 1–2 stay purely functional.*

**Files**: `.sqlx/` (refresh), `README.md`, `.github/workflows/ci.yml` (only if a check needs adding)

**Key changes**:
- Re-run `cargo sqlx prepare` with final queries; confirm `.sqlx/` covers all macros.
- README: document `.sqlx/` commit policy alongside existing `SQLX_OFFLINE` notes.

**Verify**: `SQLX_OFFLINE=true cargo clippy --all-targets -- -D warnings` and `SQLX_OFFLINE=true cargo nextest run` pass from a clean clone (simulating Docker). Manual: docker build succeeds.

---

## Testing Checkpoints
- **After Phase 1**: `dumps` table exists; test harness auto-migrates; `GET /dump/<any>` → `[]` with 200 JSON; `.sqlx/` committed; `cargo nextest run` green.
- **After Phase 2**: Full round-trip works — POST persists, GET returns accumulated `{"id", "body"}` entries; invalid JSON → 422; unknown key still `[]`.
- **After Phase 3**: Clean-clone offline build (`SQLX_OFFLINE=true`) compiles, tests pass, clippy `-D warnings` clean, Docker image builds.
