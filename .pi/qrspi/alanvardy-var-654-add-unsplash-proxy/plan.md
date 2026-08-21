# Implementation Plan — VAR-654 Unsplash Proxy

## Overview
`GET /unsplash` returns JSON `{ url, photographer, created_at }` for the newest row in a new
`unsplash_pictures` SQLite table, lazily refreshing from the Unsplash API when the row is
older than 6 hours. `UNSPLASH_API_KEY` is loaded panic-fast at startup via a new `Env`
module (pattern from `../api`); the upstream base URL is injected into `AppState` so tests
can point it at a local stub server.

**Verification commands used throughout** (from `.github/workflows/ci.yml`):
- `cargo nextest run`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`

---

## Phase 1: Config, dependencies, and a stub JSON endpoint

### Changes

#### 1. Dependencies
**File**: `Cargo.toml`
**Action**: modify

Move `reqwest` from `[dev-dependencies]` to `[dependencies]` (prod deps are visible to
tests, so the dev entry is simply removed) and add serde/serde_json:

```toml
[dependencies]
# ... existing entries unchanged ...
reqwest = { version = "0.13", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Delete the `[dev-dependencies]` `reqwest` line. (`chrono` is added in Phase 2.)

#### 2. Env template
**File**: `.env_template`
**Action**: modify

```
DATABASE_URL=sqlite:data/vardy.db
UNSPLASH_API_KEY=
```

#### 3. Env module
**File**: `src/app/env.rs`
**Action**: create

Mirrors `../api/src/app/env.rs` (module doc + panic-fast `get_string_env`), minimal:

```rust
//! Stores environment variables and verifies that they are available at startup
//! Set them for production with `fly secrets set KEY=VALUE`
//! Set them locally in `.env`

pub struct Env {
    pub unsplash_api_key: String,
}

impl Env {
    pub fn init() -> Env {
        let unsplash_api_key = get_string_env("UNSPLASH_API_KEY");
        Env { unsplash_api_key }
    }
}

fn get_string_env(key: &str) -> String {
    match std::env::var(key) {
        Ok(value) if !value.is_empty() => value,
        _ => panic!("Missing environment variable: {key}"),
    }
}
```

#### 4. Module declaration
**File**: `src/app/mod.rs`
**Action**: modify — add `pub mod env;` (alphabetical: first line, before `pub mod db;`).

#### 5. App state
**File**: `src/app/state.rs`
**Action**: modify

```rust
#[derive(Clone)]
pub struct AppState {
    pub templates: minijinja::Environment<'static>,
    pub db: sqlx::SqlitePool,
    pub unsplash_api_key: Arc<str>,
    /// Overridable so tests can point at a local stub server.
    pub unsplash_base_url: Arc<str>,
}
```

- Add `use std::sync::Arc;`
- **Remove** the `#[allow(dead_code)]` attribute and the "unused until…" doc comment on
  `db` — Phase 2 uses it.

#### 6. Main bootstrap
**File**: `src/main.rs`
**Action**: modify

`Env::init()` first thing; thread values into `AppState`. `DATABASE_URL` keeps its
existing hardcoded default (changing it is out of scope).

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env = app::env::Env::init();
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/vardy.db".to_string());
    let state = app::state::AppState {
        templates: app::templates::init(),
        db: app::db::init(&database_url).await,
        unsplash_api_key: env.unsplash_api_key.into(),
        unsplash_base_url: UNSPLASH_BASE_URL.into(),
    };
    // ... rest unchanged ...
}

const UNSPLASH_BASE_URL: &str = "https://api.unsplash.com";
```

#### 7. Test helper
**File**: `src/test/mod.rs`
**Action**: modify

`start_app()` must construct the two new fields (real key not needed — the stub handler
never reads it):

```rust
let state = crate::app::state::AppState {
    templates: crate::app::templates::init(),
    db: crate::app::db::init("sqlite::memory:").await,
    unsplash_api_key: "test-key".into(),
    unsplash_base_url: "https://api.unsplash.com".into(),
};
```

(Phase 2 generalizes this into `start_app_with`; do not add that yet.)

#### 8. Handler module
**Files**: `src/interfaces/handlers/mod.rs`, `src/interfaces/handlers/unsplash/mod.rs`
**Action**: modify / create

- `handlers/mod.rs`: add `pub mod unsplash;`
- `handlers/unsplash/mod.rs` (new): `pub mod web;`

#### 9. Stub handler
**File**: `src/interfaces/handlers/unsplash/web.rs`
**Action**: create

```rust
use axum::{Json, extract::State};
use serde_json::{Value, json};

use crate::app::error::WebError;
use crate::app::state::AppState;

pub async fn index(State(_state): State<AppState>) -> Result<Json<Value>, WebError> {
    Ok(Json(json!({
        "url": "https://example.com/placeholder.jpg",
        "photographer": "placeholder",
        "created_at": "1970-01-01 00:00:00"
    })))
}
```

`Result<Json<Value>, WebError>` works because `WebError` already implements
`IntoResponse` — no error-type changes needed in this phase.

#### 10. Route registration
**File**: `src/interfaces/routes.rs`
**Action**: modify — add after the `/singlethread` route:

```rust
.route("/unsplash", get(handlers::unsplash::web::index))
```

### Verification

#### Automated
- [x] `cargo nextest run` passes — all existing tests green
- [x] New tests in `web.rs` `#[cfg(test)] mod tests` pass:
  - `unsplash_returns_json`: `start_app()`, GET `/unsplash`, assert status 200,
    `content-type` contains `application/json`, body contains `"url"`, `"photographer"`,
    and `"created_at"`
- [x] `cargo fmt --all -- --check` passes
- [x] `cargo clippy --all-targets --all-features --locked -- -D warnings` passes

#### Manual
- [ ] `unset UNSPLASH_API_KEY; cargo run` → panics with
  `Missing environment variable: UNSPLASH_API_KEY` before binding the port
- [ ] `set -x UNSPLASH_API_KEY dummy; cargo run` then
  `curl -i localhost:3000/unsplash` → 200, `content-type: application/json`,
  body has the three placeholder keys

---

## Phase 2: SQLite cache table and read-through endpoint

### Changes

#### 1. Migration
**File**: `migrations/<timestamp>_unsplash_pictures.sql`
**Action**: create via CLI

Run `sqlx migrate add unsplash_pictures` (creates an empty timestamped file under
`migrations/`), then write:

```sql
CREATE TABLE unsplash_pictures (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  url TEXT NOT NULL,
  photographer TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

*Fallback if the CLI is unavailable*: create the file by hand named
`0002_unsplash_pictures.sql` (sqlx accepts any `<version>_<description>.sql`; version
must sort after the existing `0001_placeholder.sql`).

#### 2. Dependencies
**File**: `Cargo.toml`
**Action**: modify — add `chrono` (needed in Phase 3 for staleness; add here so the
`Picture`/query code lands complete):

```toml
chrono = "0.4"
```

(sqlx's `chrono` feature is already enabled; this makes the crate directly usable.)

#### 3. Test helper generalization
**File**: `src/test/mod.rs`
**Action**: modify

Phase 2 tests need the pool (to seed rows); Phase 3 needs base-url injection. Generalize
now, keeping `start_app()` as a thin wrapper so existing call sites are untouched:

```rust
use sqlx::SqlitePool;

pub async fn start_app() -> std::net::SocketAddr {
    start_app_with("https://api.unsplash.com").await.0
}

pub async fn start_app_with(unsplash_base_url: &str) -> (std::net::SocketAddr, SqlitePool) {
    let state = crate::app::state::AppState {
        templates: crate::app::templates::init(),
        db: crate::app::db::init("sqlite::memory:").await,
        unsplash_api_key: "test-key".into(),
        unsplash_base_url: unsplash_base_url.into(),
    };
    let db = state.db.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let router: Router = crate::interfaces::routes::routes().with_state(state);
    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service()).await.expect("server");
    });
    (addr, db)
}
```

#### 4. Handler: Picture type + DB helpers + read-through logic
**File**: `src/interfaces/handlers/unsplash/web.rs`
**Action**: modify — replace the stub body with the real shape (still no upstream call):

```rust
use axum::{Json, extract::State};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::app::error::WebError;
use crate::app::state::AppState;

#[derive(Serialize)]
pub struct Picture {
    pub url: String,
    pub photographer: String,
    pub created_at: String,
}

pub async fn index(State(state): State<AppState>) -> Result<Json<Picture>, WebError> {
    match latest_picture(&state.db).await? {
        Some(picture) => Ok(Json(picture)),
        None => Err(WebError::NotFound),
    }
}

async fn latest_picture(pool: &SqlitePool) -> Result<Option<Picture>, WebError> {
    let picture = sqlx::query_as::<_, Picture>(
        "SELECT url, photographer, created_at FROM unsplash_pictures ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(picture)
}

async fn insert_picture(pool: &SqlitePool, picture: &Picture) -> Result<Picture, WebError> {
    let inserted = sqlx::query_as::<_, Picture>(
        "INSERT INTO unsplash_pictures (url, photographer) VALUES (?, ?) \
         RETURNING url, photographer, created_at",
    )
    .bind(&picture.url)
    .bind(&picture.photographer)
    .fetch_one(pool)
    .await?;
    Ok(inserted)
}
```

`query_as::<_, Picture>` requires `sqlx::FromRow`; derive it on `Picture`
(`#[derive(Serialize, sqlx::FromRow)]`) — column names match fields. `insert_picture`
is added now (used by Phase 3) but only exercised by a unit test in this phase; add
`#[allow(dead_code)]` is **not** needed if a direct test calls it (see below).

### Verification

#### Automated
- [ ] `cargo nextest run` passes
- [ ] New `#[sqlx::test]` in `web.rs`: `unsplash_pictures_table_exists` — query
  `sqlite_master` for the table name (pattern from `src/app/db.rs:39-48`)
- [ ] New `#[sqlx::test]`: `insert_picture_returns_row_with_created_at` — call
  `insert_picture`, assert returned `created_at` is non-empty and re-reading via
  `latest_picture` round-trips url/photographer
- [ ] New integration tests (via `start_app_with`):
  - `unsplash_serves_seeded_row`: insert a row into the returned pool
    (`sqlx::query("INSERT INTO unsplash_pictures (url, photographer) VALUES (?, ?)")`),
    GET `/unsplash` → 200, body contains the seeded url and photographer,
    `content-type: application/json`
  - `unsplash_returns_404_when_empty`: no rows → GET `/unsplash` → 404
- [ ] `cargo fmt --all -- --check` and clippy command pass

#### Manual
- [ ] `sqlx migrate run` applies cleanly against `sqlite:data/vardy.db`
  (`sqlx migrate info` shows the new migration applied)
- [ ] `sqlite3 data/vardy.db "INSERT INTO unsplash_pictures (url, photographer) VALUES ('https://example.com/x.jpg', 'Someone');"`
  then `cargo run` + `curl localhost:3000/unsplash` → serves that row;
  after `DELETE FROM unsplash_pictures;` → curl returns 404

---

## Phase 3: Live Unsplash fetch with 6-hour lazy refresh

### Changes

#### 1. Error variant
**File**: `src/app/error.rs`
**Action**: modify

- Add variant `External(String)` to `WebError` (no `#[allow(dead_code)]` needed —
  constructed in prod code now)
- Add match arm in `IntoResponse`:

```rust
WebError::External(message) => {
    eprintln!("external error: {message}");
    (StatusCode::BAD_GATEWAY, "bad gateway").into_response()
}
```

- Add unit test: `external_error_is_502` —
  `WebError::External("boom".into()).into_response()` has status
  `StatusCode::BAD_GATEWAY`.

#### 2. Unsplash client module
**File**: `src/interfaces/handlers/unsplash/unsplash.rs`
**Action**: create

```rust
use reqwest::Client;
use serde::Deserialize;

use super::web::Picture;
use crate::app::error::WebError;

#[derive(Deserialize)]
struct RandomPhotoResponse {
    urls: RandomPhotoUrls,
    user: RandomPhotoUser,
}

#[derive(Deserialize)]
struct RandomPhotoUrls {
    regular: String,
}

#[derive(Deserialize)]
struct RandomPhotoUser {
    name: String,
}

/// Fetch a random nature photo from the Unsplash API.
/// Non-2xx status or parse failure maps to `WebError::External` (HTTP 502).
pub async fn fetch_random(
    client: &Client,
    base_url: &str,
    api_key: &str,
) -> Result<Picture, WebError> {
    let response = client
        .get(format!("{base_url}/photos/random"))
        .query(&[("query", "nature")])
        .header("Authorization", format!("Client-ID {api_key}"))
        .send()
        .await
        .map_err(|e| WebError::External(format!("unsplash request failed: {e}")))?;

    if !response.status().is_success() {
        return Err(WebError::External(format!(
            "unsplash returned status {}",
            response.status()
        )));
    }

    let body: RandomPhotoResponse = response
        .json()
        .await
        .map_err(|e| WebError::External(format!("unsplash response parse failed: {e}")))?;

    Ok(Picture {
        url: body.urls.regular,
        photographer: body.user.name,
        created_at: String::new(), // populated by the DB on insert
    })
}
```

**During implementation**: make one live call
(`curl -H "Authorization: Client-ID $UNSPLASH_API_KEY" "https://api.unsplash.com/photos/random?query=nature"`)
and confirm the JSON paths `urls.regular` and `user.name`; adjust the structs if the
shape differs.

#### 3. Handler flow with staleness check
**File**: `src/interfaces/handlers/unsplash/web.rs`
**Action**: modify

Replace `index` and add a staleness helper (Rust-side comparison; both timestamps are
UTC — `created_at` is written by SQLite's `datetime('now')`, compared against
`Utc::now().naive_utc()`):

```rust
use chrono::{Duration, Utc};

const MAX_AGE_HOURS: i64 = 6;

pub async fn index(State(state): State<AppState>) -> Result<Json<Picture>, WebError> {
    if let Some(picture) = latest_picture(&state.db).await?
        && !is_stale(&picture)
    {
        return Ok(Json(picture));
    }
    let client = reqwest::Client::new();
    let picture = fetch_random(&client, &state.unsplash_base_url, &state.unsplash_api_key).await?;
    let picture = insert_picture(&state.db, &picture).await?;
    Ok(Json(picture))
}

fn is_stale(picture: &Picture) -> bool {
    chrono::NaiveDateTime::parse_from_str(&picture.created_at, "%Y-%m-%d %H:%M:%S")
        .map(|created_at| Utc::now().naive_utc() - created_at > Duration::hours(MAX_AGE_HOURS))
        .unwrap_or(true) // unparseable timestamp → treat as stale, force refresh
}
```

Note the let-chains (`edition = "2024"`, matches existing style in `db.rs`).

#### 4. Module declaration
**File**: `src/interfaces/handlers/unsplash/mod.rs`
**Action**: modify — becomes:

```rust
pub mod unsplash;
pub mod web;
```

#### 5. Stub upstream server for tests
**File**: `src/test/mod.rs`
**Action**: modify — add a tiny axum stub:

```rust
use axum::{Json, http::StatusCode, routing::get};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

pub struct UnsplashStub {
    pub base_url: String,
    pub call_count: Arc<AtomicUsize>,
}

/// Spawn a local stub of `GET /photos/random`. Returns canned JSON for any
/// success `status`; the status code is returned verbatim so tests can
/// simulate upstream failures (e.g. 500).
pub async fn start_unsplash_stub(status: StatusCode) -> UnsplashStub {
    let call_count = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&call_count);
    let app = Router::new().route(
        "/photos/random",
        get(move || {
            let count = Arc::clone(&count);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                if status.is_success() {
                    Json(json!({
                        "urls": {"regular": "https://images.example.com/photo.jpg"},
                        "user": {"name": "Stub Photographer"}
                    }))
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                }
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.expect("server");
    });
    UnsplashStub { base_url: format!("http://{addr}"), call_count }
}
```

(The handler returns either `Json<...>` or `StatusCode`; axum unifies both as
`IntoResponse` — if the closure's return type needs help, annotate it as
`impl IntoResponse` or box the two arms into a common `Response`.)

### Verification

#### Automated
- [ ] `cargo nextest run` passes — all prior tests green
- [ ] New integration tests in `web.rs` tests module (each uses
  `start_unsplash_stub(...)` + `start_app_with(stub.base_url)`):
  - `no_row_triggers_fetch_and_insert`: stub 200, no rows → GET → 200, body contains
    `https://images.example.com/photo.jpg` and `Stub Photographer`; row count in pool
    is 1; `call_count` is 1
  - `fresh_row_does_not_call_upstream`: insert row with
    `created_at = datetime('now')` directly into the pool → GET → 200 with the seeded
    values; `call_count` is 0; row count unchanged
  - `stale_row_triggers_refetch`: insert row with
    `created_at = datetime('now', '-7 hours')` → GET → 200 with stub values;
    `call_count` is 1; row count is 2
  - `upstream_failure_is_502`: stub 500, no rows → GET → 502
  - `second_request_within_window_is_cached`: stub 200, empty table → two GETs; both
    200 with identical bodies, `call_count` is 1, row count 1
- [ ] `cargo fmt --all -- --check` and clippy command pass

#### Manual
- [ ] `set -x UNSPLASH_API_KEY <real-key>; cargo run`
- [ ] `curl localhost:3000/unsplash` → real photo URL + photographer (first call takes
      a few hundred ms)
- [ ] `curl localhost:3000/unsplash` again immediately → instant, same body (cached)
- [ ] `sqlite3 data/vardy.db "UPDATE unsplash_pictures SET created_at = datetime('now', '-7 hours');"`
      then curl → new photo (refetch triggered)
- [ ] Before merge/deploy: `fly secrets set UNSPLASH_API_KEY=<real-key>` (documented in
      `../api/src/app/env.rs:1-3` workflow)

---

## Testing Checkpoints

- **Phase 1**: app refuses to start without `UNSPLASH_API_KEY`; `/unsplash` returns
  valid JSON with the three keys; all prior tests green.
- **Phase 2**: migration applies cleanly (`#[sqlx::test]`); endpoint serves a seeded DB
  row and 404s when the table is empty.
- **Phase 3**: lazy-refresh behavior verified against the stub upstream
  (fetch-on-miss, cache-hit under 6h, refetch when stale, 502 on upstream failure);
  live smoke test before deploy.

**Mock strategy** (design risk): CI has no real key — resolved by the
`unsplash_base_url: Arc<str>` field on `AppState` (Phase 1) consumed by Phase 3's
`start_unsplash_stub`.
