# Implementation Summary

## Commits
| Phase | Commit | Description |
|-------|--------|-------------|
| 1     | 6b6f567 | Phase 1: Model & upstream source |
| 2     | c5a034c | Phase 2: Persistence |
| 3     | d89fd76 | Phase 3: Endpoint response & integration tests |

## Automated Checks
- [x] Add `photographer_url` field to `Picture` domain struct (`src/domain/picture.rs`) serializes automatically via `Json<Picture>`
- [x] `RandomPhotoUserRich` struct added; `RandomPhotoUser.links` wired into upstream parse in `src/infra/unsplash.rs`
- [x] Upstream parse unit tests (`parse_photographer_url_from_user_links_html`, `missing_user_links_fails_parse`) pass
- [x] Migration `0005_add_photographer_url.sql` adds the column with `NOT NULL DEFAULT ''`
- [x] `latest()` SELECT and `create()` INSERT/RETURNING updated to include `photographer_url`; round-trip test `insert_picture_returns_row_with_created_at` passes
- [x] `src/test/mod.rs` stub updated to include `user.links.html`
- [x] Existing integration tests updated to assert `photographer_url` in body across all four data paths
- [x] New `malformed_upstream_json_missing_user_links_is_502` test passes
- [x] `ROUTES.md` `GET /unsplash` response updated to include `photographer_url`
- [x] `./scripts/test.sh` full gate passes — 67/67 tests pass, fmt/clippy/prepare/TODO-grep clean

## Manual Verification Items (from the plan)
- [ ] Sanity: `cargo check` produces no warnings about unused fields
- [ ] `cargo run` → `curl -s http://localhost:3000/unsplash | jq` shows `photographer_url` field with a real Unsplash profile URL on first fetch
- [ ] Second `curl -s http://localhost:3000/unsplash | jq` returns the same `photographer_url` (cached)

## Observations
- The `DELETEME` file is a pre-existing artifact from the branch base commit (bbdc1de), unrelated to this feature; left untouched as outside the plan scope.
