# Implementation Summary

## Commits
| Phase | Commit | Description |
|-------|--------|-------------|
| 1     | 81642ac | Core credit line — both pages, happy path |
| 2     | f616c6e | Edge-case hardening |

## Automated Checks
- [x] `./scripts/test.sh` passes (format, sqlx prepare, check, build-css, clippy -D warnings, nextest, forgotten-TODO grep)
- [x] `cargo test` / `cargo nextest` — 68/68 tests pass
- [x] `index_serves_ok_html` (home) passes with new credit assertions
- [x] `index_serves_ok_html` (singlethread) passes with new credit assertions
- [x] `git diff --exit-code -- static/site.css` passes (regenerated CSS deterministic/committed)
- [x] `index_shows_credit_as_text_when_no_photographer_url` (home) passes
- [x] `index_shows_credit_as_text_when_no_photographer_url` (singlethread) passes
- [x] `index_still_renders_when_wallpaper_fetch_fails` (home) asserts no `Photo by` in body
- [x] `index_still_renders_when_wallpaper_fetch_fails` (singlethread) asserts no `Photo by` in body

## Manual Verification Items (from the plan)
- [ ] `cargo run` → open `http://localhost:3000/` — credit pill visible bottom-right with semi-transparent dark backdrop, "Photo by Wallpaper Photographer" linked, "on Unsplash" text
- [ ] `cargo run` → open `http://localhost:3000/singlethread` — same credit pill visible
- [ ] Link opens photographer's Unsplash profile in a new tab (`target="_blank"`)
- [ ] `cargo run` → all pages render with credit (happy path from Phase 1 still holds)
- [ ] No regression: `/dump` and `/health` do not include the credit (they don't extend `layout.html`)

## Notes / Deviations
- **Git base reconciliation**: The origin branch for this task had been force-pushed with a cleaner history that dropped an unrelated `unsplashrandom` feature. The base was reset to `origin/alanvardy-var-699-give-credit-to-unsplash-artist` (authoritative) before implementing, so `src/app/picture.rs` uses `crate::infra::unsplash::fetch_random` and the plan applied cleanly.
- **Phase 1 href assertion adaptation**: The plan asserted `href="https://unsplash.com/@test"`, but minijinja HTML-escapes `/` → `&#x2f;` in attribute contexts. The assertion was adapted to the codebase's established convention `href="https:&#x2f;&#x2f;unsplash.com&#x2f;@test"` — a non-structural, intent-preserving change documented by the existing wallpaper test.
- `plan.md` was reconstructed during Phase 1 (it had been absent after the earlier interrupted run); the automated items were re-checked accordingly.