# Implementation Summary

A new `/checkstitch` marketing page was added to the axum web app, mirroring
the SingleThread page (same layout, hero, platform badges, screenshot gallery,
feature sections, FAQ widget, shared Tailwind classes) but with CheckStitch
copy, CheckStitch screenshots, and **no App Store badge** at either the hero or
closing CTA. Additive only: one handler module, one template, one route, one
nav link, eight static assets (icon + seven screenshots), one `ROUTES.md`
block, and updated nav assertions.

## Commits

| Phase | Commit | Description |
|-------|--------|-------------|
| 1     | `e87de45` | feat: add CheckStitch page skeleton and nav link |
| 2     | `83765a0` | feat: add CheckStitch screenshots and feature sections |
| 3     | `380e2bd` | feat: add CheckStitch FAQ |

All on branch `alanvardy-var-1057-add-checkstitch-page`, pushed to origin. One
clean commit per phase; no commit includes `plan.md`/`.pi` artifacts (those are
parent-owned).

> **Adaptation from Phase 1 (approved):** the plan originally asked to add the
> `html_escape` test helper in Phase 1 even though it would be unused until
> Phase 3. An unused private helper trips rustc `dead_code`, which the gate's
> clippy `-D warnings` step promotes to an error — failing the very
> `./scripts/test.sh` the plan lists as the Phase 1 pass-fail requirement. Per
> supervisor approval (Option B), `html_escape` was deferred to Phase 3 where
> the FAQ tests use it, and plan.md was updated to reflect this.

## Automated Checks

- [x] Phase 1: `cargo sqlx database create && cargo sqlx migrate run` (fresh worktree prerequisite)
- [x] Phase 1: `cargo nextest run checkstitch` (3 tests)
- [x] Phase 1: `cargo nextest run home contact` (nav assertions)
- [x] Phase 1: `./scripts/test.sh` passes, incl. `static/site.css` CSS-drift check
- [x] Phase 2: `cargo nextest run checkstitch` (incl. screenshot-source asserts)
- [x] Phase 2: `cargo nextest run immutable_caching` (both pages)
- [x] Phase 2: `cargo nextest run missing_checkstitch_screenshot`
- [x] Phase 2: `./scripts/test.sh` passes, incl. CSS-drift check
- [x] Phase 2: `file static/checkstitch-shot-*.jpg static/checkstitch-watch-*.png` — iPhone 603×1306, iPad 1024×767, Watch 410×502
- [x] Phase 3: `cargo nextest run checkstitch` (all page + FAQ tests, 14)
- [x] Phase 3: `./scripts/test.sh` passes end to end (127 tests, clippy `-D warnings`, CSS-drift, TODO grep)
- [x] Phase 3: `git status --short` shows no modification to `static/site.css` (proof no new Tailwind class)
- [x] Phase 3: `git log --oneline -3` shows one commit per phase

## Static / Generated Assets

- `static/checkstitch-icon.png` — 256×256 (copied from CheckStitch `icon_256x256.png`)
- Phone JPEGs (603×1306): `checkstitch-shot-main.jpg`, `checkstitch-shot-edit.jpg`, `checkstitch-shot-settings.jpg`
- iPad JPEGs (1024×767): `checkstitch-shot-ipad.jpg`, `checkstitch-shot-ipad-edit.jpg`
- Watch PNGs (410×502, as supplied): `checkstitch-watch-list.png`, `checkstitch-watch-create.png`

## Manual Verification Items (from the plan)

- [ ] `cargo run` then `curl -s localhost:<port>/checkstitch | head` shows `<title>CheckStitch</title>`, the nav link, no `apps.apple.com` and no `app-store.svg`
- [ ] `curl -s localhost:<port>/checkstitch | grep -c 'checkstitch-icon.png'` returns 1
- [ ] `/` and `/contact` still render, now with a CheckStitch nav entry
- [ ] `cargo run`, then open `/checkstitch`: hero icon, four platform badges, no App Store button, 3 phone cards, 2 iPad cards, 2 watch cards, all captions legible
- [ ] Browser devtools: each `static/checkstitch-*` request returns 200 with a `?v=` hash suffix
- [ ] `curl -sI localhost:<port>/static/checkstitch-shot-ipad.jpg` shows `cache-control: max-age=31536000`
- [ ] `cargo run`, open `/checkstitch`: the FAQ renders 4 categories and 14 collapsible `<details>` items; each opens and closes without JavaScript
- [ ] The closing CTA reads "One checklist in. A list of reminders out." and no App Store badge follows it
- [ ] `/` and `/contact` nav shows Home, SingleThread, CheckStitch, Contact; the active page is highlighted on each