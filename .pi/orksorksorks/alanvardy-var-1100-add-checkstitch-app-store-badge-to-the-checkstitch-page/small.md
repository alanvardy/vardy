# Task

Add the App Store download badge to the `/checkstitch` page now that CheckStitch is live on the App Store (store URL `https://apps.apple.com/ca/app/checkstitch/id6811602356`), mirroring exactly how the SingleThread page does it.

1. **Template** — `templates/checkstitch.html`: replace the `{# No App Store badge: CheckStitch has no App Store presence yet. #}` comment with the badge block (same markup/Tailwind classes as SingleThread: `flex justify-center mt-6` wrapper, `<a href="…store url…" target="_blank" rel="noopener noreferrer">`, `<img src="{{ asset_url('app-store.svg') }}" alt="Download CheckStitch on the App Store" width="180" height="60">`), placed in the hero after the platform-badge row AND again after the closing CTA line — two occurrences, same as SingleThread.
2. **Tests** — `src/interfaces/handlers/checkstitch/web.rs`, `index_serves_ok_html`: invert the existing absence assertions (lines ~175–178: `count()==0` on `app-store.svg`, `!contains("apps.apple.com")`, `!contains("app-store.svg")`) to the SingleThread pattern (`singlethread/web.rs` ~line 219): store href appears **twice**, `/static/app-store.svg?v=` appears **twice**, and `target="_blank"` is present.
3. **FAQ copy** — `src/interfaces/handlers/checkstitch/web.rs` line ~24: update "CheckStitch is being prepared for its App Store release…" to state it is live on the store.

Pin the same store URL (`/ca/` region for consistency with SingleThread) in both template and test. `static/app-store.svg` already exists and is shared — no new asset. Out of scope: any hero-copy/pricing/screenshot changes.

## Why SMALL

A–F all hold: single domain (checkstitch), only 2 files touched, follows the exact existing SingleThread pattern; 0–2 unknowns; no schema/migration; no new subsystem, no shared/convention code; no design decision (region choice pre-resolved to `/ca/`); few local tests. The only human choice — `/us/` vs `/ca/` in the URL — is explicitly recommended in the ticket and pinned in both template and test, so no separate sign-off is needed.

## Key files (if the recon found any)

- `templates/checkstitch.html` (line 25 comment → badge block; plus a second badge after the closing CTA)
- `src/interfaces/handlers/checkstitch/web.rs` (line 24 FAQ copy; lines 175–178 test assertions to invert)
- Reference pattern: `templates/singlethread.html` and `src/interfaces/handlers/singlethread/web.rs` (~line 219); shared `static/app-store.svg`

Note: this worktree has a deleted `DELETEME` file in `git status` and an open draft PR #78 against the ticket — the small step should check/reconcile those (orphan cleanup + possible in-progress work) against this task before implementing.