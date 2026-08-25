# Design Discussion

## Current State

The repo serves `GET /unsplash` (`src/interfaces/routes.rs:31-34`), which
returns a single cached Unsplash picture, refreshed every 6 hours.

- **Handler**: `src/interfaces/handlers/unsplash/json.rs:7-9` — `json::index`
  calls `picture::current(&state)`.
- **App layer**: `src/app/picture.rs:12-25` `current()` — fetches the single
  latest row via `latest()` (`picture.rs:27-34`); if absent or stale (≥6h,
  `src/domain/picture.rs:16-21`), calls `fetch_random` (infra) then `create`
  (`picture.rs:36-45` INSERT … RETURNING). Returns one `Picture`.
- **Infra**: `src/infra/unsplash.rs:29-61` `fetch_random` — calls Unsplash
  `GET /photos/random` with query `nature`, maps `urls.regular` → `url`,
  `user.name` → `photographer`, `created_at: String::new()` (DB fills it).
- **Domain type**: `src/domain/picture.rs:9-13` `Picture { url, photographer,
  created_at }` — derives `Serialize`, `sqlx::FromRow`. No `id` field.
- **Rate limiting**: `UNSPLASH_TIER_PER_MS = 200` / `UNSPLASH_TIER_BURST = 5`
  (`src/app/rate_limit.rs:75-76`), applied via `tiered_routes` at
  `routes.rs:31-34`. Independent budget from the global limiter.
- **Tests**: Integration tests in `json.rs:11-230` use `start_unsplash_stub`
  (`src/test/mod.rs:153-183`), seed/clear helpers, stub `call_count`, and
  row-count assertions. Rate-limit test (`json.rs:191-230`) verifies
  concurrent 429s.
- **No randomness primitives**: No `rand` in direct deps; SQLite
  `ORDER BY RANDOM()` is available but unused.
- **Staleness only applies to the single latest row** — `current()` checks
  staleness of the most recent insert. There is no "pick one of N" logic.

## Desired End State

`GET /unsplash/random` returns a `Picture` JSON body (`{ url, photographer,
created_at }`) matching the existing `/unsplash` response shape. Behavior:

1. **Count rows** in `unsplash_pictures`. If count < 5:
   - Call the Unsplash API (`fetch_random`), insert the result via
     `create()`, return the new `Picture`. This grows the cache.
2. **If count ≥ 5**: pick one row at random via `ORDER BY RANDOM() LIMIT 1`
   and return it. No upstream call.
3. No staleness check — the threshold-triggered refetch is the freshness
   mechanism.

**Verification**: tests will assert:
- Fewer than 5 rows → upstream fetch + insert + correct body.
- 5 rows → no upstream call, valid body, row count stays at 5.
- Upstream failure → 502 "bad gateway" (via existing `WebError` path).
- Rate-limit tier trips as expected (same tier as `/unsplash`).

## Patterns to Follow

- **Response shape**: `Json<Picture>` with `{ url, photographer, created_at }`
  — exact same `Picture` type and serialization as `/unsplash`
  (`src/domain/picture.rs:9-13`).
- **Handler pattern**: handler calls app-layer function, maps error via
  `WebError`, wraps result in `Json`. Follow `json.rs:7-9` exactly.
- **App-layer function**: new `pub async fn random(state: &AppState) ->
  Result<Picture, WebError>` in `src/app/picture.rs`, following the `current()`
  pattern (`picture.rs:12-25`): DAO queries → condition → optional infra call
  → create → return.
- **DAO queries**: `sqlx::query_as::<_, Picture>` with explicit column lists
  (no `SELECT *`), matching `latest()` at `picture.rs:28-29` and `create()`
  at `picture.rs:37-38`.
- **INSERT via `create()`**: reuse the existing `create(pool, &picture)`
  (`picture.rs:36`) — one source of truth for insertion and `RETURNING`.
- **Error chokepoint**: all handler errors go through `WebError::into_response`
  (`src/app/error.rs:25-66`). Upstream failures become `WebError::External`
  → 502 (`error.rs:30-33`). Rate-limit trips become
  `WebError::TooManyRequests` → 429 (`error.rs:55-61`).
- **Route registration**: add `/unsplash/random` to the existing
  `unsplash_tier` Router in `routes.rs:31-34` — same tier budget, same
  `get()` pattern. Register the new handler via `handlers::unsplash::json::random`.
- **Tests inline**: `#[cfg(test)] mod tests` at the bottom of
  `json.rs`, following existing pattern (`json.rs:11-230`). Use
  `start_app_with`, `clear_pictures`, `start_unsplash_stub`, `test_client()`,
  row-count assertions, and stub `call_count` — exactly as existing tests do.
- **Layering**: `interfaces` → `app` → `infra`. The new handler calls
  `picture::random()`, not `infra::unsplash::*` directly
  (`src/app/picture.rs:3-4` is the sanctioned re-export boundary).
- **No new dependencies**: no `rand` crate. Use SQLite `ORDER BY RANDOM()`
  for random selection, following the principle of using what's available
  (research confirms `ORDER BY RANDOM()` is valid in SQLite and unused
  elsewhere in the codebase, so no pattern conflict).

**Anti-patterns to avoid**:
- Do NOT add a `SELECT *` query — the codebase consistently lists columns
  (`picture.rs:29`, `picture.rs:37-38`).
- Do NOT bind `created_at` on insert — let the DB default fill it
  (`picture.rs:37-38`).
- Do NOT return bare status-code tuples — always use `WebError`.
- Do NOT add a new `rand` dependency for one query.

## Design Decisions

1. **Random selection: SQLite `ORDER BY RANDOM() LIMIT 1`** — no new
   dependency needed (research Q5 confirms no `rand` in direct deps). The
   `unsplash_pictures` table is bounded small (threshold 5). A single
   `query_as::<_, Picture>` with `ORDER BY RANDOM() LIMIT 1` is trivial and
   follows the same `query_as` pattern as `latest()`.

2. **Handler placement: Add to existing `json.rs`** — the new handler is
   tightly related to `index` (same response shape, same domain, same
   `Picture` type). Keeping them together avoids module sprawl. The
   `#[cfg(test)]` block already shows patterns for multiple test helpers;
   a second handler's tests fit naturally below the existing ones.

3. **App-layer function: New `random()` in `picture.rs`** — `current()` has
   distinct semantics (staleness-driven, single-row). Adding a mode parameter
   would couple two different caching strategies. A separate `pub async fn
   random(state: &AppState) -> Result<Picture, WebError>` keeps each
   function's logic self-contained while reusing `fetch_random`, `create`,
   and the DAO query pattern. It follows the same signature pattern as
   `current()`.

4. **Rate-limit tier: Same tier as `/unsplash`** — both endpoints hit the
   same Unsplash API resource and the same database table. Sharing the tier
   budget (`UNSPLASH_TIER_PER_MS = 200`, `UNSPLASH_TIER_BURST = 5`) means
   hitting `/unsplash` and `/unsplash/random` together can't exceed the
   combined budget, which is correct (they share the upstream API). Add the
   route to the existing `unsplash_tier` Router at `routes.rs:31-34`.

5. **Stub server: Reuse as-is** — `start_unsplash_stub(status)` at
   `src/test/mod.rs:153-183` returns valid JSON with consistent fields; the
   new endpoint consumes those fields identically. `call_count` tracking
   works unchanged for asserting "did / didn't hit upstream."

## What We're NOT Doing

- **No staleness-based refresh**: `/unsplash/random` does not check
  `created_at` or `MAX_AGE_HOURS`. The threshold-based refetch (count < 5)
  replaces staleness as the freshness mechanism.
- **No dedup logic**: if `fetch_random` returns a photo already in the
  table, we insert it anyway. The count threshold (< 5 inserts, then stop)
  makes this a minor concern — at worst there are 5 distinct photos and
  duplicates are harmless.
- **No migrations**: the `unsplash_pictures` table schema is unchanged.
- **No new dependencies**: no `rand` crate, no new infra module.
- **No changes to `/unsplash`**: `current()` and the existing handler are
  untouched.
- **No pagination, filtering, or query parameters**: the endpoint takes no
  input.
- **No `id` field in the response**: `Picture` has no `id` (research Q2) and
  we're not adding one.

## Open Risks

- **`ORDER BY RANDOM()` performance**: negligible for a table capped at ~5
  rows in practice, but if the threshold were ever raised significantly,
  this query could become expensive. Mitigated by the bounded nature of the
  caching strategy.
- **Race condition on count/fetch**: if two concurrent requests both see
  count < 5, both fetch from Unsplash and both insert — we temporarily
  exceed 5 rows. Harmless: subsequent requests still pick randomly from all
  rows. The threshold logic is eventually consistent.
- **`created_at` format**: `fetch_random` sets `created_at: String::new()`
  and the DB fills it via `DEFAULT (datetime('now'))`. This matches the
  existing pattern (`picture.rs:37-38`) and `is_stale()` is not called on
  rows from `/unsplash/random`, so format mismatch is a non-issue for this
  endpoint.