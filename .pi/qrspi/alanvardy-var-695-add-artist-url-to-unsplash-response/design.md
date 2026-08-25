# Design Discussion — Add artist URL to `/unsplash` response (VAR-695)

## Current State

`GET /unsplash` returns a cached random nature photo from the Unsplash API. The
response is a `Picture` serialized as JSON with three fields:

- `url` — from upstream `urls.regular` (`src/infra/unsplash.rs:55`) or the DB `url` column (`src/app/picture.rs:29`).
- `photographer` — from upstream `user.name` (`src/infra/unsplash.rs:56`) or the DB `photographer` column (`src/app/picture.rs:29`).
- `created_at` — single-sourced by SQLite `datetime('now')` on insert; `RETURNING` populates it (`src/app/picture.rs:36-41`, `migrations/0003_unsplash_pictures.sql`), while the upstream client sets it to `String::new()` (`src/infra/unsplash.rs:57`).

Data flow: handler `Json<Picture>` (`src/interfaces/handlers/unsplash/json.rs:7-8`) → `picture::current` (`src/app/picture.rs:12-25`) → either `latest()` cached row or `fetch_random` (`src/infra/unsplash.rs:29-58`) + `create()` persistence.

The upstream client only models `urls.regular`, `user.name` (`src/infra/unsplash.rs:5-19`) — there is **no** representation of the artist's profile link. No `artist_url`/`photographer_url` field exists in the struct, table, queries, or handler anywhere.

## Desired End State

The `/unsplash` JSON response also carries the artist's Unsplash profile URL, sourced from the upstream `user.links.html` field, persisted on the cached row, and serialized as a new top-level JSON key (designated `photographer_url`).

Verification:
- Fresh fetch from the stub populates `photographer_url` from upstream and persists it; returning it in the JSON body.
- A cached row (seed or existing) returns the persisted `photographer_url` without hitting the network.
- A legacy row (pre-migration, empty value) serves an empty `photographer_url` until a refetch (>6h stale) repopulates it.
- Missing `user.links.html` upstream fails the whole parse and returns 502 (strict behavior, matching existing fields).
- HTTP status and body are both asserted; route doc in `ROUTES.md` updated.

## Patterns to Follow

- **App→infra chokepoint**: `interfaces` reaches the Unsplash client only through the sanctioned re-export `pub use crate::infra::unsplash::fetch_random` at `src/app/picture.rs:3`. Keep the new field flowing through `picture::create`/`latest`, never adding new `infra` types to handlers. Follow this (do not bypass `app`).
- **DB is single source for `created_at` only**: `created_at` is DB-populated; `photographer_url` is *not* — it comes from upstream. So it must be bound on insert and carried through both `SELECT` and `INSERT ... RETURNING` (unlike `created_at`). See sync requirement below.
- **Strict upstream parsing**: existing fields are non-optional `String` with no `#[serde(default)]`; a missing field fails the entire parse → `UnsplashError` → `WebError::External` → HTTP 502 (`src/infra/unsplash.rs:50-52`, `src/app/error.rs:30-33`, `error.rs:50-53`). Follow the same strict pattern for `user.links.html` (decision 2).
- **`.bind` order mirrors `?` placeholders**: `create()` binds `url`, `photographer` in the order of the column list (`src/app/picture.rs:36-41`). Add the new value in matching order.
- **Column lists appear in three places and must stay in sync** (research cross-cutting): `latest()` SELECT (`src/app/picture.rs:29`), `create()` INSERT+RETURNING (`src/app/picture.rs:38-39`), and the migrations DDL. Adding a column touches all three plus the `Picture` struct — keep them aligned.
- **Test conventions**: inline tests in `#[cfg(test)] mod tests` (`src/interfaces/handlers/unsplash/json.rs`); `#[sqlx::test]` for DB logic, `#[tokio::test]` + `start_app_with`/`test_client` for integration; `clear_pictures` resets the shared `seed_wallpaper` row; assert both status and body. Update the stub at `src/test/mod.rs:153-186`.

### Patterns NOT to follow

- Do **not** add a `#[serde(default)]` option-leness to the new field — it would diverge from the strict required-field behavior of every existing field and silently allow a missing attribution URL (decision 2, reversed).
- Do **not** recreate the `unsplash_pictures` table or write a hand-curated data backfill; an additive `ALTER TABLE ... ADD COLUMN` is sufficient for the single-row cache (decision 3).

## Design Decisions

1. **Field & JSON key name — `photographer_url`**: matches the existing `photographer` field and response key (`src/domain/picture.rs:10`); JSON becomes `{ url, photographer, photographer_url, created_at }`. Consistent with the current naming over the task title's "artist."

2. **Strict required upstream parse**: `RandomPhotoUser` gains `links: RandomPhotoUserRich { html: String }` (private, `#[derive(Deserialize)]`), non-optional. Missing `user.links.html` fails the parse → serde error → `UnsolvedError("unsplash response parse failed: ...")` → 502. Guarantees every served photo carries an attribution URL; matches existing field behavior. **Rejected**: `#[serde(default)]` tolerant fallback (divergent and silently lenient).

3. **New migration `ALTER TABLE unsplash_pictures ADD COLUMN photographer_url TEXT NOT NULL DEFAULT ''`**, created via `sqlx migrate add`. Existing cached rows get an empty string default; they serve for up to the 6h stale window, then refetch. No backfill needed for a single-row cache. **Rejected**: DROP/CREATE (heavier, loses cache needlessly) and nullable column (unneeded distinction).

4. **Persist via existing `create()` path**: `photographer_url` is bound on INSERT and returned via `RETURNING`, alongside `url`/`photographer`. It is NOT DB-generated (unlike `created_at`), so `fetch_random` populates it from upstream before insert, and `latest()` reads it back. Keeps all writes through the `app` chokepoint.

## What We're NOT Doing

- No new endpoint, route, or parameter. `GET /unsplash` only, response-additive.
- No change to the rate-limiting tiers, `created_at` staleness logic (`Picture::is_stale`, `src/domain/picture.rs:16-22`), or max-age constant.
- No change to error mappings in `src/app/error.rs`; `WebError` is untouched.
- No backfill/ERM-listing of existing rows; empty-URL is transient.
- No new field on any other struct or endpoint (homepage, singlethread, dump, static site).
- No `Deserialize`/`Clone`/`Debug` added to `Picture` — only `Serialize` + `sqlx::FromRow` remain, matching today.
- The response's existing keys are unchanged (backward compatible — additive only).

## Open Risks

- **Legacy-row window**: a pre-migration cached row serves `photographer_url = ""` for up to 6h after deploy. Acceptable; mitigated by decision 3, but if seamless continuity is required, note we intentionally did not backfill.
- **Unstubbed upstream shape**: the real Unsplash `/photos/random` response includes `user.links.html`, but this repo's stub (`json` at `src/test/mod.rs:160-169`) will be updated to emit it. If real upstream ever omits `user.links`, the endpoint 502s (strict behavior, decision 2) — acceptable and consistent.
- **Four-way sync**: forgetting to update one of `latest()` / `create()` / DDL / struct breaks the sqlx `FromRow` compile check. The offline query metadata must be refreshed (`cargo sqlx prepare` / `./scripts/test.sh`) after the migration.
- **Test deltas**: existing assertions (`json.rs` tests and `seed_wallpaper` INSERTs in `src/test/mod.rs:135-143`) that insert direct rows without `photographer_url` rely on the column `DEFAULT ''`; they must not be expected to carry a value (only new fetch path and stub-injected values do).