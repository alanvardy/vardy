# Structure Outline

## Approach

Add `GET /unsplash/random`: if the `unsplash_pictures` table has fewer than 5 rows, fetch from Unsplash and insert; otherwise pick a row via `ORDER BY RANDOM() LIMIT 1`. Same response shape, same rate-limit tier, same error paths as `/unsplash`.

---

## Phase 1: App-layer `random()` with DAO queries and tests

Core business logic — count rows, conditionally fetch, insert, or select random — all callable from a test harness without standing up the HTTP router.

**Files**: `src/app/picture.rs`

**Key changes**:
- `async fn count(pool: &SqlitePool) -> sqlx::Result<i64>` — new DAO query: `SELECT COUNT(*) FROM unsplash_pictures`
- `async fn random_select(pool: &SqlitePool) -> sqlx::Result<Picture>` — new DAO query: `SELECT url, photographer, created_at FROM unsplash_pictures ORDER BY RANDOM() LIMIT 1`
- `pub async fn random(state: &AppState) -> Result<Picture, WebError>` — new app-layer function:
  - calls `count(&state.db)`
  - if < 5: `fetch_random(…)` → `create(&state.db, &picture)` → return
  - if ≥ 5: `random_select(&state.db)` → return
  - upstream failures bubble as `WebError::External` (existing `From<UnsplashError>` impl)

**Unit tests** (`#[cfg(test)] mod tests` in `picture.rs`, following existing `#[sqlx::test]` pattern):
1. `count_returns_zero_on_empty` — seed nothing, assert `count() == 0`
2. `count_returns_seeded_row_count` — seed N rows, assert `count() == N`
3. `random_select_returns_a_valid_picture` — seed several rows, call `random_select`, assert returned `Picture` has non-empty `url`/`photographer`/`created_at`

**Integration-level tests** (use `start_unsplash_stub`, construct `AppState` manually or via a focused helper, call `picture::random()` directly):
4. `random_below_threshold_fetches_and_inserts` — clear table (0 rows), call `random()`, assert upstream called (call_count == 1), row count == 1, returned body matches stub
5. `random_at_threshold_selects_without_upstream` — seed exactly 5 rows, call `random()`, assert upstream NOT called (call_count == 0), row count stays 5, body is valid JSON
6. `random_upstream_failure_returns_error` — stub returns 500, call `random()`, assert `WebError::External` (maps to 502)

**Verify**: `cargo test picture` passes all new tests; existing `/unsplash` tests still pass.

---

## Phase 2: HTTP handler + route registration

Expose Phase 1’s `picture::random()` via `GET /unsplash/random`, wired into the existing unsplash rate-limit tier. Integration tests hit the real HTTP endpoint.

**Files**: `src/interfaces/handlers/unsplash/json.rs`, `src/interfaces/routes.rs`, `ROUTES.md`

**Key changes**:
- `pub async fn random(State(state): State<AppState>) -> Result<Json<Picture>, WebError>` — new handler in `json.rs`, identical pattern to `index` (line 7–9): call `picture::random(&state).await?`, wrap in `Ok(Json(…))`
- Route: add `.route("/unsplash/random", get(handlers::unsplash::json::random))` to the existing `unsplash_tier` Router in `routes.rs` — inside the same `.route(…)` chain, so both `/unsplash` and `/unsplash/random` share the `UNSPLASH_TIER_*` budget
- `ROUTES.md`: add a `### GET /unsplash/random` section (same format as existing `/unsplash` block), documenting: response shape, 502 on upstream failure, shared unsplash tier rate-limiting

**Integration tests** (inline in `json.rs` `#[cfg(test)]`, following existing patterns — `start_app_with`, `clear_pictures`, `test_client()`, stub `call_count`, row-count assertions):
1. `random_below_threshold_fetches_and_inserts` — HTTP GET with 0 rows → 200, body matches stub, row count == 1, call_count == 1
2. `random_at_threshold_no_upstream` — seed 5 rows → 200, no upstream call (call_count == 0), row count stays 5
3. `random_upstream_failure_502` — stub returns 500 → 502, body `"bad gateway"`
4. `random_shares_unsplash_tier` — hit both `/unsplash` and `/unsplash/random` rapidly under a tight tier budget, assert 429s appear with `retry-after` + `"too many requests"`

**Verify**: `./scripts/test.sh` passes; existing `/unsplash` tests unaffected; `ROUTES.md` entries match routing behavior.

---

## Testing Checkpoints

| After Phase 1 | `cargo test picture` — all DAO + app-layer tests green; `random()` logic verified in isolation |
| After Phase 2 | `./scripts/test.sh` — full gate: fmt, clippy, all tests, no forgotten TODOs; `/unsplash/random` works end-to-end over HTTP |