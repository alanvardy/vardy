# Implementation Plan

## Overview
Add a `dump` key-value store to the axum/sqlx service: `POST /dump/{key}` stores a JSON payload, `GET /dump/{key}` returns all stored payloads as `[{"id": ..., "body": ...}]` (`[]` with 200 for unknown keys), backed by a new `dumps` SQLite table and compile-time-checked sqlx macros with committed `.sqlx/` metadata.

**Deviation from structure.md (resolved):** axum 0.8 returns **400 BAD_REQUEST** (not 422) for syntactically invalid JSON bodies (`JsonSyntaxError` → 400; only *type* mismatches give 422, which can't happen when deserializing into `serde_json::Value`). Phase 2's invalid-JSON test asserts 400. Verified in `axum-0.8.9/src/json.rs`.

**Verified library facts used below:**
- sqlx 0.9 + `json` feature: `serde_json::Value` implements `Type<Sqlite>`/`Decode<Sqlite>` for TEXT columns → usable directly in `query_as!` type overrides.
- axum 0.8 registers duplicate paths with a panic → GET and POST on `/dump/{key}` **must be chained** on one `.route()` call (`.get(...).post(...)`), not two `.route()` calls.
- axum default features include `json`, so no axum feature changes needed.

---

## Phase 1: Dumps table + `GET /dump/{key}` returning `[]`

### Changes

#### 1. Create migration
**File**: `migrations/<timestamp>_create_dumps.sql`
**Action**: create via CLI (never hand-write):
```fish
sqlx migrate add create_dumps
```
Write into the generated empty file:
```sql
CREATE TABLE IF NOT EXISTS dumps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL,
    body TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_dumps_key ON dumps(key);
```

#### 2. Add dependencies
**File**: `Cargo.toml`
**Action**: modify
```toml
[dependencies]
# add:
serde_json = "1"
# change sqlx features line to include "json":
sqlx = { version = "0.9.0", features = ["sqlite", "runtime-tokio", "chrono", "migrate", "json"] }
```
(`json` feature is what makes `serde_json::Value` decode from SQLite TEXT columns.)

#### 3. Remove dead-code allowance from `AppState.db`
**File**: `src/app/state.rs`
**Action**: modify — drop the doc comment and `#[allow(dead_code)]`; production code now queries `db`:
```rust
#[derive(Clone)]
pub struct AppState {
    pub templates: minijinja::Environment<'static>,
    pub db: sqlx::SqlitePool,
}
```

#### 4. Make the shared HTTP test harness auto-migrate
**File**: `src/test/mod.rs`
**Action**: modify — in `start_app()`, run migrations against the pool before serving (insert between pool creation and router construction):
```rust
pub async fn start_app() -> SocketAddr {
    let db = crate::app::db::init("sqlite::memory:").await;
    sqlx::migrate!("./migrations").run(&db).await.expect("migrate");
    let state = crate::app::state::AppState {
        templates: crate::app::templates::init(),
        db,
    };
    // ... rest unchanged ...
}
```
(The `migrate!` macro embeds migrations at compile time; each `start_app()` call gets a fresh `sqlite::memory:` pool, so tests stay isolated.)

#### 5. Wire up the `dump` handler area
**File**: `src/interfaces/handlers/mod.rs`
**Action**: modify — add one line:
```rust
pub mod home;
pub mod singlethread;
pub mod dump;
```

**File**: `src/interfaces/handlers/dump/mod.rs`
**Action**: create — one line, matching existing areas:
```rust
pub mod web;
```

#### 6. Implement the GET handler
**File**: `src/interfaces/handlers/dump/web.rs`
**Action**: create
```rust
use axum::{extract::{Path, State}, Json};
use serde::Deserialize;
use sqlx::sqlite::SqliteRow;

use crate::app::error::WebError;
use crate::app::state::AppState;

#[derive(Debug, Deserialize)]
pub struct DumpEntry {
    pub id: i64,
    pub body: serde_json::Value,
}

pub async fn index(
    Path(key): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<Vec<DumpEntry>>, WebError> {
    let entries = sqlx::query_as!(
        DumpEntry,
        r#"SELECT id, body AS "body: serde_json::Value" FROM dumps WHERE key = ? ORDER BY id"#,
        key
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(entries))
}
```
Notes:
- The `"body: serde_json::Value"` type override is required because sqlx would otherwise infer TEXT → `String`. Requires the sqlx `json` feature (step 2).
- Unknown keys simply match zero rows → `[]` with 200 (design decision Q3=A).
- `ORDER BY id` gives insertion order ("no ordering guarantees beyond insertion order" per design).
- If `query_as!` rejects the struct (macro requires field-by-field construction), fall back to selecting into a private row struct of `(i64, String)` then mapping with `serde_json::from_str(&row.body)?` wrapped in a conversion — but try the form above first; it compiles with sqlx 0.9.
- `Deserialize` derive is unused by the response path but harmless and documents shape; if clippy flags it, drop the derive and keep the plain struct.

#### 7. Register the route
**File**: `src/interfaces/routes.rs`
**Action**: modify — add `post` to the routing import and one route line inside `routes()`:
```rust
use axum::{Router, routing::{get, post}};
// ...
        .route("/dump/{key}", post(handlers::dump::web::create))
```
(For Phase 1 only `post(...)` is registered pointing at a handler added in Phase 2 — **instead**, in Phase 1 register nothing yet OR register just the GET; see Phase 2 step 2 where both are merged onto one line. To avoid churn, in Phase 1 write:
```rust
        .route("/dump/{key}", get(handlers::dump::web::index))
```
and in Phase 2 change that single line to the chained form.)
axum 0.8 uses `{key}` syntax for path parameters — first use in this codebase.

#### 8. Generate offline query metadata
**Directory**: `.sqlx/` (new, committed)
**Action**: first `query_as!` usage requires offline data for `SQLX_OFFLINE=true` builds (Docker sets this):
```fish
set -x DATABASE_URL sqlite:data/vardy.db
sqlx database create
sqlx migrate run
cargo sqlx prepare
git add .sqlx
```
If codegen fails or `cargo sqlx prepare` is unavailable: verify `cargo install sqlx-cli --no-default-features --features sqlite` was run, and as a last resort check in the generated files manually under `.sqlx/` (format is stable JSON per-query) — never disable the macros instead.

### Verification

#### Automated
- [x] `cargo nextest run` passes — including new inline test in `dump/web.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::test::{start_app, test_client};
    use axum::http::StatusCode;

    #[tokio::test]
    async fn get_unknown_key_returns_empty_list() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/dump/nope"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()
                .get("content-type")
                .is_some_and(|v| v.to_str().unwrap().contains("application/json"))
        );
        let body = res.text().await.unwrap();
        assert_eq!(body, "[]");
    }
}
```
- [x] `SQLX_OFFLINE=true cargo nextest run` passes locally (confirms `.sqlx/` covers the new macro).
- [x] `cargo clippy --all-targets --all-features --locked -- -D warnings` passes (CI command).

#### Manual
- [ ] `sqlx migrate run && cargo run`, then `curl -i localhost:3000/dump/x` → HTTP 200, `content-type: application/json`, body `[]`.

---

## Phase 2: `POST /dump/{key}` storing JSON payloads

### Changes

#### 1. Implement the POST handler
**File**: `src/interfaces/handlers/dump/web.rs`
**Action**: modify — add import and function:
```rust
use axum::http::StatusCode;

pub async fn create(
    Path(key): Path<String>,
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Result<StatusCode, WebError> {
    let serialized = serde_json::to_string(&body).expect("serializing Value cannot fail");
    sqlx::query!("INSERT INTO dumps (key, body) VALUES (?, ?)", key, serialized)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::CREATED)
}
```
Notes:
- `Json` is a consuming extractor → it must come **last** in the argument list (after `Path` and `State`).
- Invalid JSON never reaches the handler: axum rejects with 400 before invoking it (see deviation note at top).
- 201 Created communicates resource creation; any 2xx satisfies the design contract ("returns success").

#### 2. Chain POST onto the existing route
**File**: `src/interfaces/routes.rs`
**Action**: modify — replace the Phase 1 route line (**do not add a second `.route("/dump/{key}", ...)` call — axum panics on duplicate paths**):
```rust
        .route("/dump/{key}", get(handlers::dump::web::index).post(handlers::dump::web::create))
```
Add `post` to the routing import if not already there from Phase 1 step 7.

### Verification

#### Automated
- [ ] `cargo nextest run` passes — including new inline tests appended to the existing `tests` module in `dump/web.rs`:

```rust
    #[tokio::test]
    async fn post_stores_and_get_returns_it() {
        let addr = start_app().await;
        let client = test_client();
        let payload = serde_json::json!({ "a": 1, "nested": { "b": [true, null] } });
        let res = client
            .post(format!("http://{addr}/dump/k"))
            .json(&payload)
            .send()
            .await
            .expect("request failed");
        assert!(res.status().is_success());

        let res = client
            .get(format!("http://{addr}/dump/k"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        let entries: Vec<crate::interfaces::handlers::dump::web::DumpEntry> =
            res.json().await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, 1);
        assert_eq!(entries[0].body, payload);
    }

    #[tokio::test]
    async fn multiple_posts_accumulate() {
        let addr = start_app().await;
        let client = test_client();
        for n in 0..3 {
            client
                .post(format!("http://{addr}/dump/acc"))
                .json(&serde_json::json!({ "n": n }))
                .send()
                .await
                .expect("request failed");
        }
        let res = client
            .get(format!("http://{addr}/dump/acc"))
            .send()
            .await
            .expect("request failed");
        let entries: Vec<crate::interfaces::handlers::dump::web::DumpEntry> =
            res.json().await.unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries.iter().map(|e| e.body["n"].as_i64().unwrap()).collect::<Vec<_>>(),
            vec![0, 1, 2] // insertion order
        );
    }

    #[tokio::test]
    async fn post_invalid_json_rejected() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .post(format!("http://{addr}/dump/bad"))
            .header("content-type", "application/json")
            .body("{not json")
            .send()
            .await
            .expect("request failed");
        // axum 0.8: malformed JSON syntax -> 400 (JsonSyntaxError)
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }
```

#### Manual
- [ ] `cargo run`, then:
  ```fish
  curl -X POST -H "Content-Type: application/json" -d '{"a":1}' localhost:3000/dump/x   # 201
  curl localhost:3000/dump/x                                                            # [{"id":1,"body":{"a":1}}]
  curl localhost:3000/dump/y                                                            # []
  ```

---

## Phase 3: Offline-build hardening + docs

### Changes

#### 1. Refresh `.sqlx/` with final queries
**Directory**: `.sqlx/`
**Action**: re-run with both queries present and confirm coverage:
```fish
set -x DATABASE_URL sqlite:data/vardy.db
cargo sqlx prepare
git add .sqlx
```
Expect two metadata files (one per macro: the `query_as!` SELECT and the `query!` INSERT).

#### 2. Document the `.sqlx/` commit policy
**File**: `README.md`
**Action**: modify — extend the existing bullet about compile-time-checked macros (README lines 13–15) to state that `.sqlx/` is committed and refreshed via `cargo sqlx prepare` after schema/query changes:
```markdown
- Compile-time-checked query macros (`query!` etc.) need either a reachable
  `DATABASE_URL` or committed offline metadata: set `SQLX_OFFLINE=true` and
  refresh metadata with `cargo sqlx prepare` after schema changes. The
  `.sqlx/` directory is committed so Docker builds (`SQLX_OFFLINE=true`)
  compile without a live database.
```

#### 3. CI check (only if needed)
**File**: `.github/workflows/ci.yml`
**Action**: inspect only — the existing clippy job (`cargo clippy --all-targets --all-features --locked -- -D warnings`) already exercises the macros offline once `.sqlx/` is committed (CI has no DATABASE_URL, so macros *must* resolve offline or fail loudly — no workflow change expected). Do not edit unless a gap is observed.

### Verification

#### Automated
- [ ] Clean-clone simulation: `git stash -u` is too risky — instead verify from a fresh checkout directory:
  ```fish
  git worktree add /tmp/vardy-clean HEAD
  cd /tmp/vardy-clean
  env -u DATABASE_URL SQLX_OFFLINE=true cargo clippy --all-targets --all-features --locked -- -D warnings
  env -u DATABASE_URL SQLX_OFFLINE=true cargo nextest run
  git worktree remove /tmp/vardy-clean
  ```
- [ ] Full suite still green: `cargo nextest run`

#### Manual
- [ ] `docker build -t vardy-dump-test .` succeeds end-to-end (builder stage compiles offline via committed `.sqlx/`; runtime stage applies both migrations via `sqlx migrate run`).
