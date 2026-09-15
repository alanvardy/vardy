# Done

- **Branch / head SHA**: `alanvardy-var-1012-add-newer-screenshots` @ `424ccff`
  (pushed to origin; every push on this branch was a fast-forward, so
  `--force-with-lease` was never needed).
- **Mechanical checks**: `./scripts/test.sh` **PASS** — fmt, `cargo sqlx
  prepare -- --tests`, `cargo check --all-targets`, CSS-drift (`git diff
  --exit-code -- static/site.css` clean), `cargo clippy --all-targets
  --all-features --locked -- -D warnings`, `cargo nextest run` **113/113**,
  TODO/FIXME grep. No warnings flagged. No `Cargo.lock` change, so no
  `cargo audit` was needed.
- **Review outcome**:
  - **Blockers: none.**
  - **Fixes worth doing now — applied** (commit `d9a26df`): corrected the
    ungrammatical comment `// copy corrects to match…` →
    `// copy corrections to match…` (`src/interfaces/handlers/singlethread/
    web.rs:192`).
  - **Optional improvements — applied** (same commit): added
    `justify-center` to the phone screenshot row so four cards wrap centered
    (the class already exists in `static/site.css`, so no CSS regeneration and
    no drift); sharpened the iPad `alt` from "One reminder at a time on iPad"
    (near-duplicate of the iPhone `alt`) to "Completing or skipping a reminder
    on iPad".
  - **Declined**: adding `.pi/` to `.gitignore` — this repo deliberately tracks
    `.pi/orksorksorks/<branch>/` artifacts (36 such files on `main`).
  - **Confirmed as non-issues**: `ROUTES.md` correctly untouched (no route path
    or parameter changed); removing the retired asset's assertion is not a
    coverage regression (`asset_url()` panics on a registered-but-missing
    asset); the new 404 test is meaningful because `ServeDir` is mounted with no
    fallback (`src/interfaces/routes.rs:57-61`).
- **Repo hygiene**: the branch-start `DELETEME` placeholder was deleted and
  committed (`45a9907`). Repo-wide grep confirms the only remaining
  `singlethread-watch-list` reference is the intentional 404-test URL
  (`src/interfaces/routes.rs:252`). `static/site.css` is unchanged.

## Follow-up outside the original plan: migration idempotency (`424ccff`)

`cargo run` was failing locally with `table unsplash_pictures already exists`.
Cause: the local `test.db` had a schema built by hand-applying migration SQL,
so `_sqlx_migrations` recorded only migrations 1-2 while the schema already had
migration 3's table, leaving 3/4/5 "pending". Migration 3 was not idempotent and
collided with the existing table on the next boot. Tests never caught it because
`src/test/mod.rs` uses `sqlite::memory:` and `#[sqlx::test]` uses temp databases.

Fix: every migration is now idempotent. SQLite has no `ALTER TABLE ... ADD COLUMN
IF NOT EXISTS`, so `0005` could not be made re-runnable in place — instead
`photographer_url` moved into `0003`'s `CREATE TABLE IF NOT EXISTS` and `0005`
was retired. Rewriting an applied migration was safe only because `fly.toml`
declares no `[mounts]`: the production SQLite file sits on the machine's
ephemeral rootfs and is recreated on every deploy, so no existing ledger is
validated against the old checksums.

Verified: `cargo sqlx migrate info` shows 4/4 installed; each migration file
re-applied individually with `sqlite3` exits 0; a simulation of the original
failure mode (hand-applied table, empty ledger) now recovers instead of
crashing; `./scripts/test.sh` still **113/113** with no `.sqlx` or `site.css`
drift.

**Tradeoff accepted:** `IF NOT EXISTS` no-ops silently against a table of a
different shape, so a stale local database must be *reset*, not migrated. The
local `test.db` was reset (backup of the pre-change file at
`/tmp/test.db.pre-idempotency.bak`).

## Manual items

The app was booted locally (`cargo run`) to close out the blocker:

- ✅ 1. `/singlethread` renders 200; four phone-row cards incl. the iPad one.
- ⚠️ 2. Wrapped-card centering at tablet width still needs a human eye.
- ✅ 3. **On your wrist** shows exactly one card.
- ✅ 4. `/static/singlethread-watch-list.png` → 404 while `/singlethread` → 200,
  no panic.
- ✅ 5. Fresh, distinct 12-hex `?v=` hashes on all six assets
  (`icon 5dcf8f2d7c29`, `ipad 39907a32fa97`, `main 04e95b2e5d15`,
  `settings 0ce3c27799b9`, `swipe 7884e3262192`, `watch-detail d33e4fbb2ef1`);
  the retired asset appears zero times in the served HTML.
- ✅ `Content-Type` for `singlethread-shot-ipad.jpg` is `image/jpeg`.

Image contents were independently re-verified against the template `alt` text
during review: `settings.jpg` = Settings list, `swipe.jpg` = "Clean bathroom"
with complete/mic/skip bar, `ipad.jpg` = landscape action bar, `watch-detail.png`
= Skip/Reschedule/Delete, `main.jpg` = "Pick up milk". All formats match their
extensions (iPad JPEG 640x480; main/swipe JPEG 601x1306; settings JPEG 282x612;
watch-detail PNG 359x440).
