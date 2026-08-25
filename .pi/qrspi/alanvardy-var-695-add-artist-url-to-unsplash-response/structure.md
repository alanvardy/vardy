# Structure Outline

## Approach
Add a single additive response field `photographer_url` to `GET /unsplash`, following every existing pattern: strict upstream parse of `user.links.html`, persist it through the same `app`-layer chokepoint (`picture::create`/`latest`), keep the handler untouched (`Json<Picture>` serializes it automatically), and verify with `#[sqlx::test]`/`start_app_with` assertions on status + body. Work is sliced vertically by data-flow stage (source → persistence → endpoint).

---

## Phase 1: Model & upstream source
Carry the artist profile link end-to-end from the upstream parse into the domain type. No DB changes yet.

**Files**: `src/domain/picture.rs`, `src/infra/unsplash.rs`, `src/test/mod.rs`
**Key changes**:
- `Picture { url: String, photographer: String, photographer_url: String, created_at: String }` — new field; derive set unchanged (`Serialize`, `sqlx::FromRow`)
- `RandomPhotoUser { name: String, links: RandomPhotoUserRich }` — new nested field; private, non-optional `#[derive(Deserialize)]`
- `RandomPhotoUserRich { html: String }` — new private type (strict: missing `html` fails parse → 502)
- `fetch_random(client, base_url, api_key) -> Result<Picture, UnsplashError>` — sets `photographer_url: body.user.links.html` (alongside `url`, `photographer`; `created_at` still `String::new()`)

**Verify**: `./scripts/test.sh` passes; unit test in `infra/unsplash.rs` asserts `fetch_random` maps `user.links.html` into `photographer_url` and that a body missing `user.links` returns `Err(UnsplashError)` (malformed-parse path currently untested per research).

---

## Phase 2: Persistence
Persist the new value and read it back, staying in sync across all three column lists.

**Files**: `migrations/<new>_add_photographer_url.sql` (via `sqlx migrate add`), `src/app/picture.rs`
**Key changes**:
- Migration: `ALTER TABLE unsplash_pictures ADD COLUMN photographer_url TEXT NOT NULL DEFAULT ''`
- `latest(pool: &SqlitePool) -> sqlx::Result<Option<Picture>>` — SELECT column list `{url, photographer, photographer_url, created_at}`
- `create(pool: &SqlitePool, picture: &Picture) -> sqlx::Result<Picture>` — INSERT column list `{url, photographer, photographer_url}`; binds `url`, `photographer`, then `photographer_url` in `?` order; `RETURNING` list gains `photographer_url` (NOT DB-generated, unlike `created_at`)
- `seed_wallpaper(&db: &SqlitePool)` — left as-is; relies on `DEFAULT ''` for legacy rows

**Verify**: `./scripts/test.sh` (runs `cargo sqlx prepare`, so offline metadata refreshes — the FromRow compile check gates four-way sync); `#[sqlx::test]` in `app/picture.rs` round-trips `create` → `latest` and asserts the returned `photographer_url` equals the bound value.

---

## Phase 3: Endpoint response & integration tests
Prove the `/unsplash` JSON carries and serves the new key through all three paths (fresh, cached, legacy).

**Files**: `src/test/mod.rs`, `src/interfaces/handlers/unsplash/json.rs`, `ROUTES.md`
**Key changes**:
- `start_unsplash_stub(status)` — canned JSON gains `"user": {"name": ..., "links": {"html": "https://unsplash.com/@stub"}}`, so fresh-fetch tests get a real `photographer_url`
- New/adjusted `#[cfg(test)]` tests in `json.rs` — assert status **and** body contains `photographer_url`: fresh fetch persists + returns stub URL; cached fresh row returns seeded value with `call_count == 0`; stale row (>6h) refetches and repopulates; legacy row (seeded via current `seed_wallpaper`, no value) serves `photographer_url: ""` until stale refetch; missing `user.links` in stub → 502 `"bad gateway"`
- `ROUTES.md` — document the additive `photographer_url` response key for `GET /unsplash`

**Verify**: `./scripts/test.sh` passes; manual check — `GET /unsplash` recursively returns `{ "url", "photographer", "photographer_url", "created_at" }` with a resolvable artist profile URL on a first fetch and the same persisted URL on a subsequent cached request.

---

## Testing Checkpoints
- **After Phase 1**: `Picture` and upstream client model the URL; compile + unit tests green; DB still has no new column (feature disabled/enough to fail later phases loudly).
- **After Phase 2**: migration applies; schema/`query`/struct column lists are in sync (`cargo sqlx prepare` refreshed, no FromRow errors); create→latest round-trip carries `photographer_url`; `/unsplash` already serves the new field via `Json<Picture>` even though integration tests aren't updated yet.
- **After Phase 3**: all four data paths (fresh, cached, legacy-empty, malformed-strict) asserted with status + body; `ROUTES.md` documents the key; full `./scripts/test.sh` gate green.

Note (non-slicable): the handler is unchanged — `Json<Picture>` serializes every struct field, so exposing the field requires zero handler/route edits. The three phases remain independently valuable (types, persistence, endpoint proof) even if reordered.