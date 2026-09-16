# Done

- **Branch / head SHA**: `alanvardy-var-1012-add-newer-screenshots` @ `9a01f49`
  (pushed to origin; every push on this branch was a fast-forward, so
  `--force-with-lease` was never needed).
- **Mechanical checks**: `./scripts/test.sh` **PASS** — fmt, `cargo sqlx
  prepare -- --tests`, `cargo check --all-targets`, CSS-drift (`git diff
  --exit-code -- static/site.css` clean), `cargo clippy --all-targets
  --all-features --locked -- -D warnings`, `cargo nextest run` **112/112**,
  TODO/FIXME grep. No warnings flagged. No `Cargo.lock` change, so no
  `cargo audit` was needed. (113 on the pre-follow-up revision; net −1 because
  the retired-asset 404 test was deleted and its replacement case folded into
  the existing serving-table test.)
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
  committed (`45a9907`). `static/site.css` is unchanged.

## Follow-up 1 outside the original plan: migration idempotency (`424ccff`)

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
crashing; `./scripts/test.sh` passes with no `.sqlx` or `site.css` drift.

**Tradeoff accepted:** `IF NOT EXISTS` no-ops silently against a table of a
different shape, so a stale local database must be *reset*, not migrated. The
local `test.db` was reset (backup of the pre-change file at
`/tmp/test.db.pre-idempotency.bak`).

## Follow-up 2: the Apple Watch shot was reinstated (`eb61e84`)

The operator supplied a newer Apple Watch image (an incoming PNG in
`~/Downloads`) to sit **before** the actions shot. Its content is a reminder
("Pick up milk", dated) with a green check (Complete) and an orange slashed
circle (Skip) — i.e. exactly what the previously-retired filename
`singlethread-watch-list.png` described, so it was re-added under that name.

Consequences handled in the same commit:

- The `retired_singlethread_watch_list_shot_returns_404` test from `267853b` was
  **deleted** — it asserted the opposite of the new truth and would have failed.
- The asset returned to the serving table in `src/interfaces/routes.rs` as
  `image/png` (verified from the bytes: PNG, 410x502), and a `?v=` assertion plus
  the alt-text assertion were re-added to `index_serves_ok_html`.
- No CSS class was introduced (the sibling card's classes were reused verbatim),
  so `static/site.css` is byte-identical; `.sqlx` metadata is unchanged.
- `plan.md`/`implement.md` still describe the shot as retired. Those are
  point-in-time records of the superseded revision, so they were deliberately
  left untouched rather than rewritten.

Delta review (fresh context, one bounded reviewer): **no blockers, no fixes
worth doing now**. Noted and declined: adding `width`/`height` to the new `img`
(the sibling watch cards also omit them, so touching one and not the other would
be inconsistent — a whole-row change, out of scope). The reviewer's assumption
that these `.pi/` docs are untracked is wrong for this repo (they are committed
deliverables), which is why this section exists.

Verified on a live boot: `/singlethread` → 200 with the wrist row rendering
`watch-list` first then `watch-detail`; `/static/singlethread-watch-list.png` →
200 `image/png`, `public, max-age=31536000, immutable`, fresh hash
`bcd52a89e5d2` (distinct from `watch-detail`'s `d33e4fbb2ef1`).

## Follow-up 3: the iPad shot moved to its own section and grew 3x (`c183d2a`, `997c0f3`)

Requested by the operator. The iPad figure left the phone row and now has its
own `On iPad` subsection between the phone row and `On your wrist`:

```html
<h2 class="heading-subsection">On iPad</h2>
<div class="flex justify-center">
    <div class="card w-full max-w-2xl p-3">
        <img src="{{ asset_url('singlethread-shot-ipad.jpg') }}" alt="Completing or skipping a reminder on iPad"
             width="1024" height="768" class="w-full rounded-lg border border-neutral-700">
    </div>
</div>
```

- **3x size** is `max-w-2xl` = 42rem = 672px, against the row's former
  `max-w-[14rem]` = 224px. Verified in the compiled CSS
  (`--container-2xl:42rem`), not assumed.
- This introduced a new Tailwind utility, so `static/site.css` was regenerated
  and committed in the same commit (`c183d2a`), per the repo's CSS-drift rule.
- **The re-export mattered more than the size change.** The asset had been
  deliberately downscaled to 640x480 in Phase 2, when it rendered at 224px. At
  the new size (~646 CSS px, ~1292 device px on a 2x display) that upscaled
  roughly 2x and looked soft. It was re-encoded from the 1024x768 original at
  quality 82 (135 KB) via `sips`.
- `width`/`height` were added so the browser reserves the space instead of
  shifting ~646px as the image loads. The other screenshots still omit these
  (they render at ~224px, where the shift is minor).

The phone row is back to three figures. `./scripts/test.sh` passes 112/112 with
no CSS or `.sqlx` drift. Live boot confirmed: the section order is phone row →
`On iPad` → `On your wrist`; the served `<img>` carries `width="1024"
height="768"`; the asset is served as `image/jpeg` at 1024x768; and its `?v=`
hash moved from `39907a32fa97` to `6e293777ffc7` (the other six hashes are
unchanged), so the cache is busted correctly.

## Follow-up 4: screenshots interspersed with the copy (`ad12591`)

Requested by the operator: the iPad shot now sits beside **"Everything you need,
nothing you don't"** (text left, image right) and the Apple Watch shots beside
**"Built for quiet productivity"** (images left, copy right). The standalone
`On iPad` / `On your wrist` subheadings were removed — the `alt` text already
names the platform, so they were redundant once the images joined real sections.

Both rows reuse the hero's **existing** responsive pattern rather than inventing
one: `flex flex-col md:flex-row gap-8 md:items-center` with `md:flex-1` on the
text column and `md:w-2/5` (iPad) / `md:w-1/2` (watch pair) plus `md:flex-none`
on the image column. Only three utilities were new (`md:items-center`,
`md:w-2/5`, `md:w-1/2`), so `static/site.css` was regenerated and committed in
the same commit.

Verified on a live boot: the iPad image is the second child of its row (so it
renders right) and the watch images are the first child of theirs (so they
render left); a heading/image outline of the served HTML confirms
`shot-ipad.jpg` now falls under "Everything you need, nothing you don't" and the
two watch shots under "Built for quiet productivity", with no orphaned
`On iPad`/`On your wrist` headings left anywhere in `templates/`. On mobile both
rows stack (iPad below its copy, watch shots above theirs).

A test assertion for the now-deleted `On iPad` heading was removed from
`index_serves_ok_html`; the image assertions for all six screenshots are
unchanged and still pass. Gate 112/112.

## Follow-up 5: captions, and stacking until `lg` (`9a01f49`)

Two operator requests.

**Captions.** The iPad shot and both Apple Watch shots are now
`<figure class="card m-0 ...">` with a `figcaption` styled like the phone row's
(`text-muted text-sm mt-2 text-center`): *On iPad*, *Complete or skip*, and
*Skip, reschedule, or delete*. `m-0` is not cosmetic — Preflight is not imported,
so `<figure>` keeps its `1em 40px` UA margin and would otherwise indent inside
the flex column. All six figures on the page were confirmed to carry it. Three
caption assertions were added to `index_serves_ok_html`.

**Breakpoints.** Both interspersed rows moved from `md:` to `lg:` (64rem =
1024px), so the pictures no longer sit side-to-side with the copy once the
viewport hits the 768px `md` breakpoint — they stay stacked and full-width until
1024px. Rationale: the page container is capped at 48rem (704px of content), so
a 768px viewport produced very cramped columns (a 164px-wide watch card).
Changing the breakpoint does not enlarge the columns at `lg:`, since the
container is width-capped either way; it only keeps tablets on the stacked
layout.

Side effects checked: the now-unused `md:items-center`, `md:w-2/5` and
`md:w-1/2` utilities dropped out of `static/site.css`, while the hero keeps its
own `md:flex-row` / `md:flex-1` / `md:flex-none` / `md:order-none`. Served markup
confirmed: six captions in document order, both rows carrying
`flex flex-col lg:flex-row gap-8 lg:items-center`, and the only surviving `md:`
usages being the hero's and the layout chrome's. Gate 112/112.

Not changed: the two Apple Watch pictures still sit side-by-side with *each
other* at every width (they're a pair inside the same column). If the request
was about those two rather than the copy/picture split, that is a one-line
change to `flex-col lg:flex-row` on their wrapper.

## Manual items

The app was booted locally (`cargo run`) to close out the blockers:

- ✅ 1. `/singlethread` renders 200; three phone-row cards, then the iPad shot in
  the "Everything you need, nothing you don't" row, then the watch pair in the
  "Built for quiet productivity" row.
- ⚠️ 2. Responsive reflow still needs a human eye: the two interspersed rows now
  switch from stacked to side-by-side at 64rem (1024px) rather than 48rem, and the
  columns are ~40-50% of the 704px content width.
- ✅ 3. **On your wrist** removed as a section; its two cards now render on the
  left of the "Built for quiet productivity" row.
- ✅ 5. Fresh, distinct 12-hex `?v=` hashes on all seven assets (`icon
  5dcf8f2d7c29`, `ipad 39907a32fa97`, `main 04e95b2e5d15`, `settings
  0ce3c27799b9`, `swipe 7884e3262192`, `watch-list bcd52a89e5d2`,
  `watch-detail d33e4fbb2ef1`).
- ✅ `Content-Type` for `singlethread-shot-ipad.jpg` is `image/jpeg`;
  `singlethread-watch-list.png` is `image/png`.
- ️ 4. Superseded: this item originally checked that the retired watch-list
  asset 404s. It no longer applies — the asset is live again.

Image contents were independently re-verified against the template `alt` text
during review: `settings.jpg` = Settings list, `swipe.jpg` = "Clean bathroom"
with complete/mic/skip bar, `ipad.jpg` = landscape action bar, `watch-list.png`
= reminder with Complete/Skip, `watch-detail.png` = Skip/Reschedule/Delete,
`main.jpg` = "Pick up milk". All formats match their extensions (iPad JPEG
1024x768; main/swipe JPEG 601x1306; settings JPEG 282x612; watch-list PNG
410x502; watch-detail PNG 359x440).