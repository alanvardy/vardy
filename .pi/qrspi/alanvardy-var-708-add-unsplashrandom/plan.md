# Implementation Plan

## Overview

Add `GET /unsplash/random`: count rows in `unsplash_pictures`; if fewer than 5, fetch from Unsplash and insert; otherwise select a row via `ORDER BY RANDOM() LIMIT 1`. Same response shape, same rate-limit tier, same error paths as `/unsplash`.

---

## Phase 1: App-layer `random()` with DAO queries and tests

### Changes

#### 1. Add `count()` DAO query
**File**: `src/app/picture.rs`
**Action**: modify — add new `count` function after `create()` (before `#[cfg(test)]`)

```rust
pub async fn count(pool: &SqlitePool) -> sqlx::Result<i64> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM unsplash_pictures")
        .fetch_one(pool)
        .await
}
```

#### 2. Add `random_select()` DAO query
**File**: `src/app/picture.rs`
**Action**: modify — add after `count()`

```rust
pub async fn random_select(pool: &SqlitePool) -> sqlx::Result<Picture> {
    sqlx::query_as::<_, Picture>(
        "SELECT url, photographer, created_at FROM unsplash_pictures ORDER BY RANDOM() LIMIT 1",
    )
    .fetch_one(pool)
    .await
}
```

#### 3. Add `random()` app-layer function
**File**: `src/app/picture.rs`
**Action**: modify — add after `random_select()`

```rust
/// Return a random picture: fetch from Unsplash when fewer than 5 are
/// cached; otherwise select one from the local table.
pub async fn random(state: &AppState) -> Result<Picture, WebError> {
    if count(&state.db).await? < 5 {
        let picture = fetch_random(
            &state.http,
            &state.unsplash_base_url,
            &state.env.unsplash_api_key,
        )
        .await?;
        return Ok(create(&state.db, &picture).await?);
    }
    Ok(random_select(&state.db).await?)
}
```

#### 4. Add unit tests for DAO + `random()`
**File**: `src/app/picture.rs`
**Action**: modify — add new test functions inside the existing `#[cfg(test)] mod tests` block, after the existing `insert_picture_returns_row_with_created_at` test

```rust
#[sqlx::test]
async fn count_returns_zero_on_empty(pool: SqlitePool) {
    let c = count(&pool).await.expect("count");
    assert_eq!(c, 0);
}

#[sqlx::test]
async fn count_returns_seeded_row_count(pool: SqlitePool) {
    // Seed 3 rows
    for i in 0..3 {
        create(
            &pool,
            &Picture {
                url: format!("https://example.com/{i}.jpg"),
                photographer: format!("Photographer {i}"),
                created_at: String::new(),
            },
        )
        .await
        .expect("insert");
    }
    let c = count(&pool).await.expect("count");
    assert_eq!(c, 3);
}

#[sqlx::test]
async fn random_select_returns_a_valid_picture(pool: SqlitePool) {
    // Seed a few rows so random_select has something to pick
    for i in 0..2 {
        create(
            &pool,
            &Picture {
                url: format!("https://example.com/{i}.jpg"),
                photographer: format!("Photographer {i}"),
                created_at: String::new(),
            },
        )
        .await
        .expect("insert");
    }
    let pic = random_select(&pool).await.expect("random_select");
    assert!(!pic.url.is_empty());
    assert!(!pic.photographer.is_empty());
    assert!(!pic.created_at.is_empty());
}
```

#### 5. Add integration-level tests for `random()`
**File**: `src/app/picture.rs`
**Action**: modify — add these tests after the unit tests above. They use `start_unsplash_stub` and construct `AppState` manually to test `picture::random()` directly.

```rust
#[tokio::test]
async fn random_below_threshold_fetches_and_inserts() {
    use crate::test::start_unsplash_stub;
    use crate::app::env::Env;
    use std::sync::Arc;

    let stub = start_unsplash_stub(axum::http::StatusCode::OK).await;
    let pool = SqlitePool::connect("sqlite::memory:").await.expect("pool");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");
    // table is empty after migration (no seed_wallpaper)

    let state = AppState {
        db: pool.clone(),
        http: reqwest::Client::new(),
        unsplash_base_url: stub.base_url.into(),
        env: Arc::new(Env {
            unsplash_api_key: "test-key".into(),
            database_url: "sqlite::memory:".into(),
            sentry_dsn: String::new(),
            enable_sentry: false,
            rate_limit_per_ms: 1,
            rate_limit_burst: 1_000_000,
        }),
        templates: crate::app::templates::init(),
        metrics: Arc::new(
            crate::infra::metrics::AppMetrics::new().expect("metrics"),
        ),
    };

    let picture = random(&state).await.expect("random should succeed");
    assert!(picture.url.contains("images.example.com"));
    assert_eq!(picture.photographer, "Stub Photographer");

    let c = count(&pool).await.expect("count");
    assert_eq!(c, 1);
    assert_eq!(
        stub.call_count.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn random_at_threshold_selects_without_upstream() {
    use crate::test::start_unsplash_stub;
    use crate::app::env::Env;
    use std::sync::Arc;

    let stub = start_unsplash_stub(axum::http::StatusCode::OK).await;
    let pool = SqlitePool::connect("sqlite::memory:").await.expect("pool");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");

    // Seed exactly 5 rows
    for i in 0..5 {
        create(
            &pool,
            &Picture {
                url: format!("https://example.com/{i}.jpg"),
                photographer: format!("Photographer {i}"),
                created_at: String::new(),
            },
        )
        .await
        .expect("insert");
    }
    let initial_count = count(&pool).await.expect("count");
    assert_eq!(initial_count, 5);

    let state = AppState {
        db: pool.clone(),
        http: reqwest::Client::new(),
        unsplash_base_url: stub.base_url.into(),
        env: Arc::new(Env {
            unsplash_api_key: "test-key".into(),
            database_url: "sqlite::memory:".into(),
            sentry_dsn: String::new(),
            enable_sentry: false,
            rate_limit_per_ms: 1,
            rate_limit_burst: 1_000_000,
        }),
        templates: crate::app::templates::init(),
        metrics: Arc::new(
            crate::infra::metrics::AppMetrics::new().expect("metrics"),
        ),
    };

    let picture = random(&state).await.expect("random should succeed");
    assert!(!picture.url.is_empty());
    assert!(!picture.photographer.is_empty());

    // Row count unchanged
    let final_count = count(&pool).await.expect("count");
    assert_eq!(final_count, 5);
    // No upstream call
    assert_eq!(
        stub.call_count.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
}

#[tokio::test]
async fn random_upstream_failure_returns_error() {
    use crate::test::start_unsplash_stub;
    use crate::app::env::Env;
    use std::sync::Arc;

    let stub =
        start_unsplash_stub(axum::http::StatusCode::INTERNAL_SERVER_ERROR).await;
    let pool = SqlitePool::connect("sqlite::memory:").await.expect("pool");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrate");
    // empty table → will try upstream

    let state = AppState {
        db: pool.clone(),
        http: reqwest::Client::new(),
        unsplash_base_url: stub.base_url.into(),
        env: Arc::new(Env {
            unsplash_api_key: "test-key".into(),
            database_url: "sqlite::memory:".into(),
            sentry_dsn: String::new(),
            enable_sentry: false,
            rate_limit_per_ms: 1,
            rate_limit_burst: 1_000_000,
        }),
        templates: crate::app::templates::init(),
        metrics: Arc::new(
            crate::infra::metrics::AppMetrics::new().expect("metrics"),
        ),
    };

    let result = random(&state).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        WebError::External(_) => {} // expected
        other => panic!("expected WebError::External, got {other:?}"),
    }
}
```

### Verification

#### Automated
- [x] `cargo test picture` passes all 9 tests (3 existing + 6 new)
- [x] `cargo check --all-targets` passes
- [x] `cargo clippy --all-targets --all-features --locked -- -D warnings` passes

#### Manual
- [ ] No test output warnings or panics

---

## Phase 2: HTTP handler + route registration

### Changes

#### 1. Add `random` handler
**File**: `src/interfaces/handlers/unsplash/json.rs`
**Action**: modify — add after `index` (after line 9), before `#[cfg(test)]`

```rust
pub async fn random(State(state): State<AppState>) -> Result<Json<Picture>, WebError> {
    Ok(Json(picture::random(&state).await?))
}
```

#### 2. Add handler integration tests
**File**: `src/interfaces/handlers/unsplash/json.rs`
**Action**: modify — add to the existing `#[cfg(test)] mod tests` block, after the last existing test

```rust
#[tokio::test]
async fn random_below_threshold_fetches_and_inserts() {
    let stub = start_unsplash_stub(axum::http::StatusCode::OK).await;
    let (addr, db) = start_app_with(&stub.base_url).await;
    clear_pictures(&db).await;
    // No seed — 0 rows → threshold fetch

    let res = test_client()
        .get(format!("http://{addr}/unsplash/random"))
        .send()
        .await
        .expect("request to /unsplash/random should succeed");
    assert_eq!(res.status(), 200);
    assert!(
        res.headers()
            .get("content-type")
            .is_some_and(|v| v.to_str().unwrap().contains("application/json"))
    );
    let body = res.text().await.expect("body");
    assert!(body.contains("https://images.example.com/photo.jpg"));
    assert!(body.contains("Stub Photographer"));

    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM unsplash_pictures")
        .fetch_one(&db)
        .await
        .expect("count");
    assert_eq!(count, 1);
    assert_eq!(stub.call_count.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn random_at_threshold_no_upstream() {
    let stub = start_unsplash_stub(axum::http::StatusCode::OK).await;
    let (addr, db) = start_app_with(&stub.base_url).await;
    clear_pictures(&db).await;
    // Seed exactly 5 rows
    for i in 0..5 {
        sqlx::query("INSERT INTO unsplash_pictures (url, photographer) VALUES (?, ?)")
            .bind(format!("https://example.com/{i}.jpg"))
            .bind(format!("Photographer {i}"))
            .execute(&db)
            .await
            .expect("seed insert");
    }

    let res = test_client()
        .get(format!("http://{addr}/unsplash/random"))
        .send()
        .await
        .expect("request to /unsplash/random should succeed");
    assert_eq!(res.status(), 200);
    let body = res.text().await.expect("body");
    // Body contains one of the seeded URLs
    assert!((0..5).any(|i| body.contains(&format!("https://example.com/{i}.jpg"))));
    assert!((0..5).any(|i| body.contains(&format!("Photographer {i}"))));

    assert_eq!(stub.call_count.load(std::sync::atomic::Ordering::SeqCst), 0);
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM unsplash_pictures")
        .fetch_one(&db)
        .await
        .expect("count");
    assert_eq!(count, 5);
}

#[tokio::test]
async fn random_upstream_failure_502() {
    let stub = start_unsplash_stub(axum::http::StatusCode::INTERNAL_SERVER_ERROR).await;
    let (addr, db) = start_app_with(&stub.base_url).await;
    clear_pictures(&db).await;

    let res = test_client()
        .get(format!("http://{addr}/unsplash/random"))
        .send()
        .await
        .expect("request to /unsplash/random should succeed");
    assert_eq!(res.status(), 502);
    let body = res.text().await.expect("body");
    assert_eq!(body, "bad gateway");
}

#[tokio::test]
async fn random_shares_unsplash_tier() {
    use axum::http::StatusCode;
    use std::sync::atomic::Ordering;

    let stub = start_unsplash_stub(axum::http::StatusCode::OK).await;
    let (addr, _pool) =
        crate::test::start_app_with_rate_limits(&stub.base_url, 1, 1_000_000).await;
    clear_pictures(&_pool).await;
    let client = test_client();
    // UNSPLASH_TIER_BURST = 5; fire 20 concurrent GETs at /unsplash/random
    let handles: Vec<_> = (0..20)
        .map(|_| {
            let client = client.clone();
            let url = format!("http://{addr}/unsplash/random");
            tokio::spawn(async move { client.get(url).send().await.expect("request failed") })
        })
        .collect();
    let mut ok = 0;
    let mut limited = 0;
    for handle in handles {
        let res = handle.await.expect("join");
        match res.status() {
            StatusCode::OK => ok += 1,
            StatusCode::TOO_MANY_REQUESTS => {
                limited += 1;
                assert!(res.headers().get("retry-after").is_some());
                assert_eq!(res.text().await.unwrap(), "too many requests");
            }
            status => panic!("unexpected status {status}"),
        }
    }
    assert!(ok >= 1, "at least one request should succeed");
    assert!(limited >= 5, "tier should trip");
    assert!(
        stub.call_count.load(Ordering::SeqCst) < 20,
        "stub should see fewer calls than requests"
    );
}
```

#### 3. Register route in `unsplash_tier`
**File**: `src/interfaces/routes.rs`
**Action**: modify — add `/unsplash/random` to the existing `unsplash_tier` Router at line 32

Change line 32 from:
```rust
        Router::new().route("/unsplash", get(handlers::unsplash::json::index)),
```
to:
```rust
        Router::new()
            .route("/unsplash", get(handlers::unsplash::json::index))
            .route("/unsplash/random", get(handlers::unsplash::json::random)),
```

#### 4. Document the new route
**File**: `ROUTES.md`
**Action**: modify — add a new `### GET /unsplash/random` section after the existing `/unsplash` block (after the `---` separator on line 103)

```markdown
### GET /unsplash/random

Returns a random Unsplash photo (JSON). If fewer than 5 pictures are cached,
fetches from Unsplash and inserts a new row; otherwise picks a random cached
row. No staleness timeout — the 5-row threshold controls when the cache is
refilled.

- Response: `200 OK` — `application/json` `{ "url": ..., "photographer": ..., "created_at": ... }`
- Errors: `500` via `WebError` (database failure), `502` via `WebError` (upstream failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.
- Rate limit: also subject to the same stricter dedicated tier as `/unsplash` (see
  `UNSPLASH_TIER_*` in `src/app/rate_limit.rs`) nested inside the global budget.

---
```

### Verification

#### Automated
- [ ] `./scripts/test.sh` passes (fmt, check, clippy, nextest, forgotten TODOs)
- [ ] `cargo test unsplash` — all existing `/unsplash` tests still pass
- [ ] `cargo test random` — all new `/unsplash/random` tests pass

#### Manual
- [ ] `ROUTES.md` entries match the route behavior described in the code

---

## Testing Checkpoints

| After Phase 1 | `cargo test picture` — all DAO + app-layer tests green; `random()` logic verified in isolation |
| After Phase 2 | `./scripts/test.sh` — full gate: fmt, clippy, all tests, no forgotten TODOs; `/unsplash/random` works end-to-end over HTTP |