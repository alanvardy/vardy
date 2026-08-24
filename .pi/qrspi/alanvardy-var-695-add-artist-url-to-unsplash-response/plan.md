# Implementation Plan

## Overview

Add a `photographer_url` field to the `GET /unsplash` JSON response, sourced from Unsplash `user.links.html`, persisted in the cache row, and serialized automatically via `Json<Picture>` — zero handler or route changes.

---

## Phase 1: Model & upstream source

Carry the artist profile link end-to-end from the upstream parse into the domain type. No DB migration yet (queries still select 3 columns — `FromRow` fails at runtime until Phase 2, which is the intended loud failure).

### Changes

#### 1. Add `photographer_url` field to `Picture`
**File**: `src/domain/picture.rs`
**Action**: modify

```rust
#[derive(Serialize, sqlx::FromRow)]
pub struct Picture {
    pub url: String,
    pub photographer: String,
    pub photographer_url: String,
    pub created_at: String,
}
```

#### 2. Add `RandomPhotoUserRich` and wire into upstream parse
**File**: `src/infra/unsplash.rs`
**Action**: modify

Add the new struct after `RandomPhotoUser`:

```rust
#[derive(Deserialize)]
struct RandomPhotoUserRich {
    html: String,
}
```

Add `links` field to `RandomPhotoUser`:

```rust
#[derive(Deserialize)]
struct RandomPhotoUser {
    name: String,
    links: RandomPhotoUserRich,
}
```

Update `fetch_random` return to populate the new field:

```rust
Ok(Picture {
    url: body.urls.regular,
    photographer: body.user.name,
    photographer_url: body.user.links.html,
    created_at: String::new(), // populated by the DB on insert
})
```

#### 3. Unit tests for upstream parse
**File**: `src/infra/unsplash.rs`
**Action**: modify — add `#[cfg(test)] mod tests` block at bottom of file

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_photographer_url_from_user_links_html() {
        let json = serde_json::json!({
            "urls": {"regular": "https://example.com/img.jpg"},
            "user": {
                "name": "Test Photographer",
                "links": {"html": "https://unsplash.com/@test"}
            }
        });
        let parsed: RandomPhotoResponse = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.user.links.html, "https://unsplash.com/@test");
    }

    #[test]
    fn missing_user_links_fails_parse() {
        let json = serde_json::json!({
            "urls": {"regular": "https://example.com/img.jpg"},
            "user": {"name": "Test Photographer"}
        });
        let err = serde_json::from_value::<RandomPhotoResponse>(json).unwrap_err();
        assert!(err.to_string().contains("links"));
    }
}
```

#### 4. Update test struct literal in `insert_picture_returns_row_with_created_at`
**File**: `src/interfaces/handlers/unsplash/json.rs`
**Action**: modify — add `photographer_url` field to the `Picture` literal (compile-forced by Phase 1 struct change)

```rust
let picture = Picture {
    url: "https://example.com/x.jpg".to_string(),
    photographer: "Someone".to_string(),
    photographer_url: "https://unsplash.com/@someone".to_string(),
    created_at: String::new(),
};
```

Also add an assertion after the existing ones:

```rust
assert_eq!(latest.photographer_url, "https://unsplash.com/@someone");
```

> **Note**: `latest()` still SELECTs only 3 columns (`url, photographer, created_at`) so `FromRow` will fail at runtime for `photographer_url`. This test will **fail** in Phase 1 — that's expected (the DB column doesn't exist yet). Phase 2 unblocks it.

### Verification

#### Automated
- [x] `cargo check --all-targets` passes (struct layout + serde parse compile)
- [x] `cargo test --lib` — new unit tests in `infra/unsplash.rs` pass; `insert_picture_returns_row_with_created_at` fails with `FromRow` error (expected)
- [x] `cargo fmt --all` clean
- [x] `cargo clippy --all-targets --all-features --locked -- -D warnings` clean

#### Manual
- [ ] Sanity: `cargo check` produces no warnings about unused fields

---

## Phase 2: Persistence

Add the DB column and update all three query column lists so `FromRow` succeeds and `photographer_url` round-trips through `create` → `latest`.

### Changes

#### 1. Migration
**File**: `migrations/<timestamp>_add_photographer_url.sql`
**Action**: create via CLI, then populate

```fish
sqlx migrate add add_photographer_url
```

Then write into the generated file:

```sql
ALTER TABLE unsplash_pictures ADD COLUMN photographer_url TEXT NOT NULL DEFAULT '';
```

#### 2. Update `latest()` SELECT
**File**: `src/app/picture.rs`
**Action**: modify line 29

```rust
"SELECT url, photographer, photographer_url, created_at FROM unsplash_pictures ORDER BY id DESC LIMIT 1",
```

#### 3. Update `create()` INSERT + RETURNING + bind
**File**: `src/app/picture.rs`
**Action**: modify lines 38-42

```rust
"INSERT INTO unsplash_pictures (url, photographer, photographer_url) VALUES (?, ?, ?) \
 RETURNING url, photographer, photographer_url, created_at",
```

Add `.bind(&picture.photographer_url)` after the photographer bind:

```rust
.bind(&picture.url)
.bind(&picture.photographer)
.bind(&picture.photographer_url)
```

Final `create` function:

```rust
pub async fn create(pool: &SqlitePool, picture: &Picture) -> sqlx::Result<Picture> {
    let inserted = sqlx::query_as::<_, Picture>(
        "INSERT INTO unsplash_pictures (url, photographer, photographer_url) VALUES (?, ?, ?) \
         RETURNING url, photographer, photographer_url, created_at",
    )
    .bind(&picture.url)
    .bind(&picture.photographer)
    .bind(&picture.photographer_url)
    .fetch_one(pool)
    .await?;
    Ok(inserted)
}
```

### Verification

#### Automated
- [ ] `cargo sqlx prepare -- --tests` succeeds (offline metadata refreshed against migrated DB)
- [ ] `cargo check --all-targets` passes (FromRow column count matches everywhere)
- [ ] `cargo test --lib` — `insert_picture_returns_row_with_created_at` now passes (round-trip with `photographer_url`)
- [ ] `./scripts/test.sh` — gate passes (migration, prepare, check, clippy, tests)

#### Manual
- [ ] `sqlx migrate info` shows new migration as applied
- [ ] `echo "SELECT photographer_url FROM unsplash_pictures;" | sqlite3 test.db` shows the column exists

---

## Phase 3: Endpoint response & integration tests

Update the stub, add/update integration tests covering all four data paths, and document the new response key.

### Changes

#### 1. Update stub JSON to include `user.links.html`
**File**: `src/test/mod.rs`
**Action**: modify — in `start_unsplash_stub`, the success-path `Json(json!(...))` block

Change:
```rust
Json(json!({
    "urls": {"regular": "https://images.example.com/photo.jpg"},
    "user": {"name": "Stub Photographer"}
}))
```

To:
```rust
Json(json!({
    "urls": {"regular": "https://images.example.com/photo.jpg"},
    "user": {
        "name": "Stub Photographer",
        "links": {"html": "https://unsplash.com/@stub"}
    }
}))
```

#### 2. Update existing integration tests to assert `photographer_url` in body
**File**: `src/interfaces/handlers/unsplash/json.rs`
**Action**: modify — add `photographer_url` body assertions to existing tests

| Test | Change |
|------|--------|
| `unsplash_serves_seeded_row` | Raw INSERT has no `photographer_url` column → DB `DEFAULT ''`. Assert body contains `"photographer_url":""`. |
| `no_row_triggers_fetch_and_insert` | Stub now carries `https://unsplash.com/@stub`. Assert body contains `"photographer_url":"https://unsplash.com/@stub"`. |
| `fresh_row_does_not_call_upstream` | Raw INSERT → empty default. Assert body contains `"photographer_url":""`. |
| `stale_row_triggers_refetch` | Stale row (raw INSERT, no value) → refetch populates. Assert body contains `"photographer_url":"https://unsplash.com/@stub"`. |
| `second_request_within_window_is_cached` | Both responses include stub `photographer_url`. Assert first body contains it. |

Specific assertions to add:

- `unsplash_serves_seeded_row`: `assert!(body.contains(r#""photographer_url":"""#));`
- `no_row_triggers_fetch_and_insert`: `assert!(body.contains("https://unsplash.com/@stub"));`
- `fresh_row_does_not_call_upstream`: `assert!(body.contains(r#""photographer_url":"""#));`
- `stale_row_triggers_refetch`: `assert!(body.contains("https://unsplash.com/@stub"));`
- `second_request_within_window_is_cached`: `assert!(first_body.contains("https://unsplash.com/@stub"));`

#### 3. New test: missing `user.links` in stub → 502
**File**: `src/interfaces/handlers/unsplash/json.rs`
**Action**: modify — add new `#[tokio::test]` after `upstream_failure_is_502`

```rust
#[tokio::test]
async fn malformed_upstream_json_missing_user_links_is_502() {
    // Spawn a stub that returns JSON without `user.links`
    let call_count = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&call_count);
    let app = Router::new().route(
        "/photos/random",
        get(move || {
            let count = Arc::clone(&count);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Json(json!({
                    "urls": {"regular": "https://images.example.com/photo.jpg"},
                    "user": {"name": "Stub Photographer"}
                }))
                .into_response()
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service()).await.expect("server");
    });
    let base_url = format!("http://{addr}");

    let (app_addr, db) = start_app_with(&base_url).await;
    clear_pictures(&db).await;

    let res = test_client()
        .get(format!("http://{app_addr}/unsplash"))
        .send()
        .await
        .expect("request");
    assert_eq!(res.status(), 502);
    let body = res.text().await.expect("body");
    assert_eq!(body, "bad gateway");
}
```

Also add the necessary imports at the top of the test module:

```rust
use axum::{Json, Router, routing::get};
use axum::response::IntoResponse;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
```

These imports are already available — `axum`, `serde_json::json`, `Arc`, `AtomicUsize`, `Ordering` are all used in the existing test module. `Router`, `routing::get`, `IntoResponse` may need explicit imports.

#### 4. Update `ROUTES.md`
**File**: `ROUTES.md`
**Action**: modify — the `GET /unsplash` response section

Change the response line from:
```
- Response: `200 OK` — `application/json` `{ "url": ..., "photographer": ..., "created_at": ... }`
```

To:
```
- Response: `200 OK` — `application/json` `{ "url": ..., "photographer": ..., "photographer_url": ..., "created_at": ... }`
```

### Verification

#### Automated
- [ ] `./scripts/test.sh` passes (full gate: fmt, prepare, check, css, clippy, test, todo-grep)
- [ ] All 10+ tests in `json.rs` pass — confirm via `cargo nextest run` output showing test names and status
- [ ] New `malformed_upstream_json_missing_user_links_is_502` test passes

#### Manual
- [ ] `cargo run` → `curl -s http://localhost:3000/unsplash | jq` shows `photographer_url` field with a real Unsplash profile URL on first fetch
- [ ] Second `curl -s http://localhost:3000/unsplash | jq` returns the same `photographer_url` (cached)

---

## Testing Checkpoints

- **After Phase 1**: `cargo check` green. Unit tests in `infra/unsplash.rs` pass. `insert_picture_returns_row_with_created_at` fails at runtime (missing column — expected; Phase 2 unblocks).
- **After Phase 2**: Full `./scripts/test.sh` green. All existing tests pass. `insert_picture_returns_row_with_created_at` now passes with `photographer_url` round-trip.
- **After Phase 3**: Full `./scripts/test.sh` green. All four data paths (fresh, cached, legacy-empty, malformed-strict) asserted with status + body.

### Deviation from structure

The structure places Phase 1 changes exclusively in `src/domain/picture.rs`, `src/infra/unsplash.rs`, and `src/test/mod.rs`. However, adding `photographer_url` to the `Picture` struct forces the struct literal in `insert_picture_returns_row_with_created_at` (`src/interfaces/handlers/unsplash/json.rs`) to be updated for compilation. That test literal update is included in Phase 1 above. The test will still **fail at runtime** in Phase 1 because the DB column doesn't exist yet — Phase 2 unblocks it. This is the intended "fail loudly" behavior described in the structure.