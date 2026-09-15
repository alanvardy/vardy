# Implementation Summary

## Commits

| Phase | Commit | Description |
|-------|--------|-------------|
| 1     | 49575aa | Swap SingleThread phone screenshots for the VAR-1012 set |
| 2     | 267853b | Add iPad screenshot, retire stale Apple Watch list shot |

Both pushed to `origin/alanvardy-var-1012-add-newer-screenshots`.

## Automated Checks

- [x] Phase 1: `./scripts/test.sh` passes (fmt, sqlx prepare, check, CSS drift, clippy, nextest 112/112, TODO grep)
- [x] Phase 1: `file static/singlethread-shot-*.jpg` reports 601x1306 (main, swipe), 282x612 (settings)
- [x] Phase 1: `git diff --stat static/site.css` empty (no CSS drift)
- [x] Phase 1: `cargo nextest run --locked index_serves_ok_html` passes (2/2, incl. the two new copy assertions)
- [x] Phase 2: `./scripts/test.sh` passes (113/113 nextest, incl. the new `retired_singlethread_watch_list_shot_returns_404` and the extended caching test)
- [x] Phase 2: `file` reports `singlethread-shot-ipad.jpg` = JPEG 640x480, `singlethread-watch-detail.png` = PNG 359 x 440 (54 KB — under the 400 KB limit)
- [x] Phase 2: `ls static/singlethread-watch-list.png` → no such file (git rm committed)
- [x] Phase 2: `git diff --exit-code -- static/site.css` passes (no class change, no CSS regeneration needed)
- [x] Phase 2: `cargo nextest run --locked singlethread` passes 13/13
- [x] Phase 2: `rg -n "singlethread-watch-list" templates/ src/` — see note below

### Notes / observations

- **rg check nuance (plan-internal inconsistency, supervisor-ruled):** plan.md change 5 mandates adding a 404 test whose request URL must contain `singlethread-watch-list.png`, so the automated item "`rg ...` returns nothing" can never be literally satisfied in this commit. The check's intent (no lingering serving/display references) is proven: template div, `?v=` assertion, and serving-table row are all gone, and the 404 test proves the asset is retired. The sole `rg` hit is `src/interfaces/routes.rs:252` — the intentional test URL. Item checked off with an inline note.
- **Bootstrap needed on fresh checkouts:** `test.db` did not exist in the checkout; `cargo sqlx prepare -- --tests` cannot open a nonexistent SQLite file. The worker applied `migrations/*.sql` to a fresh gitignored `test.db` (matching the "created on first boot" convention). A future fresh checkout needs the same bootstrap (or `SQLX_OFFLINE=true`).
- Pre-existing unrelated dirt (`DELETEME` deletion, untracked `.pi/orksorksorks/` dir) left untouched per plan; staging was by explicit path only, never `git add -A`.

## Manual Verification Items (from the plan)

- [ ] Phase 1: `cargo run`, open `/singlethread`; the first row shows: dark "Pick up milk" (caption *One reminder at a time*), light **Settings** list (caption *Settings*), dark "Clean bathroom" with the three-button bar (caption *Complete or skip*). No broken images.
- [ ] Phase 1: Each of the three `<img src>` in the served HTML ends `?v=` followed by 12 hex chars, and the three hashes differ from each other and from `origin/main`'s values.
- [ ] Phase 2: `cargo run`, open `/singlethread`; the first row now shows **four** cards: dark "Pick up milk", light Settings list, dark "Clean bathroom" action bar, and the **landscape iPad** card (caption *On iPad*) — the iPad card must not be stretched or letterboxed.
- [ ] Phase 2: The **On your wrist** section shows exactly **one** card: the Apple Watch with Skip / Reschedule / Delete, and the heading is still correct for what it contains.
- [ ] Phase 2: `/static/singlethread-watch-list.png` returns 404 in the browser while `/singlethread` still renders 200 with no panic.
- [ ] Phase 2: The newly added iPad and watch `<img src>` values each end in a fresh `?v=` 12-hex hash.