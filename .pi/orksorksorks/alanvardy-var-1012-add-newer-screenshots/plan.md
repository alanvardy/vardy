# Implementation Plan

## Overview

Replace the SingleThread page screenshots with the five newer VAR-1012 images,
mapping image → slot by **verified image content** rather than by ticket order,
and restructuring the second screenshot row because the new set has only one
Apple Watch shot plus one iPad landscape shot (the page currently claims two
Apple Watch shots). Asset bytes are the payload; `asset_url()` re-hashes at
startup, so `?v=` cache keys refresh themselves.

### Verified image inventory (do not re-derive)

Source files live in `~/Downloads`. Content was confirmed by opening each image,
not inferred from filenames.

| # | Source file | Geometry | Content | Target slot |
|---|---|---|---|---|
| 1 | `F2C642A5-AC11-4251-ACD9-0754CEF497FC_1_105_c.jpeg` | 601×1306 | iPhone dark: "Pick up milk", one reminder + gear | `singlethread-shot-main.jpg` |
| 2 | `F409E32A-C612-4F76-9C70-D65A312D182F_1_105_c.jpeg` | 601×1306 | iPhone dark: "Clean bathroom" + Complete/mic/Skip bar | `singlethread-shot-swipe.jpg` |
| 3 | `AF8D1A8C-FB43-42B2-B719-9939E02BD7E5_4_5005_c.jpeg` | 282×612 | iPhone light: **Settings** list | `singlethread-shot-settings.jpg` |
| 4 | `70EA8DBF-0A47-4B7C-B340-F120929A994D_4_5005_c.jpeg` | 359×440 | **Apple Watch**: "Reminder" + Skip / Reschedule / Delete | `singlethread-watch-detail.png` |
| 5 | `6F6D3A52-8405-486D-93E9-C7AA338E6DCB_1_105_c.jpeg` | 1024×768 | **iPad landscape**: "Clean the bathroom" + action bar | **new** `singlethread-shot-ipad.jpg` |

`medium.md`'s literal order (→ main, settings, swipe, watch-list, watch-detail)
is contradicted by the bytes: it would put the Settings list in the "Swipe to
complete" slot, the action-bar phone shot in the "Settings" slot, and the iPad
landscape shot under the "On your wrist" heading. This plan uses the content
mapping above (user-approved Option A/A/A).

### Decisions already made (do not re-litigate)

- **The watch row keeps one card.** There is only one Apple Watch image in the
  new set (#4), so `singlethread-watch-list.png` is **retired**: the `static/`
  file is deleted and its `<div>` removed. Heading stays `On your wrist`, which
  is then accurate.
- **The iPad shot gets a new filename** (`singlethread-shot-ipad.jpg`) and a
  fourth `<figure>` in the existing phone row — no misleading `.png` watch name.
- **Real format conversion** so extension, served `Content-Type`, and tests stay
  truthful: JPEG→PNG for the `.png` slot, and the iPad shot is downscaled.
- **No Tailwind class edits.** The new `<figure>` reuses the existing
  `card m-0 flex-1 basis-[10rem] max-w-[14rem] p-3` / `w-full rounded-lg border
  border-neutral-700` classes verbatim. Reusing them means `static/site.css`
  needs **no regeneration**. (If you find yourself adding a new arbitrary
  utility, stop: run `./scripts/build-css.sh` and commit `static/site.css` in
  the same commit, or the gate's drift check fails.)

### Constraints the recon established

- Only three files reference these assets: `templates/singlethread.html`
  (L43–66), `src/interfaces/handlers/singlethread/web.rs` (L190–195),
  `src/interfaces/routes.rs` (L215–229). **`ROUTES.md` needs no change** — no
  route path or parameter changes.
- `asset_url()` **panics on an unknown asset** and hashes `static/` on first use.
  Therefore a referenced file must exist on disk before any test runs: apply the
  template edit and the file create/delete **together** before running the gate.
  Never run `cargo nextest` mid-phase.
- `Content-Type` is chosen by **file extension** (`ServeDir`, `routes.rs:52–57`),
  so a JPEG byte-stream in a `.png` file would serve `image/png` while passing
  the test — the reason we convert rather than rename bytes.
- **No byte/hash/golden checks exist** on these assets; the `?v=` hash is never
  asserted, only the `?v="` prefix. Swapping bytes cannot break a pinned digest.
- `./scripts/test.sh` gate = `cargo fmt --all` → `cargo sqlx prepare -- --tests`
  → `cargo check --all-targets` → `./scripts/build-css.sh` + `git diff
  --exit-code -- static/site.css` → `cargo clippy --all-targets --all-features
  --locked -- -D warnings` → `cargo nextest run` → a TODO/FIXME grep. It needs
  `.env` with `DATABASE_URL`.

### Working-tree pre-flight

The branch already has unrelated dirt: `DELETEME` is deleted (` D DELETEME`) and
`.pi/orksorksorks/<branch>/` is untracked. **Leave both alone.** Stage and commit
each phase by explicit path (`git add <paths>`), and never `git add -A` /
`git add .`.

`static/*` files are version-controlled, so git is the backup for the
overwritten bytes — no `.bak` copies needed. To back out a bad copy:
`git checkout -- static/singlethread-shot-main.jpg`.

---

## Phase 1: Phone row — swap the three iPhone screenshots

Thinnest end-to-end slice: the three phone slots get the correct new bytes and
the two strings that stopped being true get fixed. No filenames change, so no
test in the repo can break — the assertions are filename-only, the expected
content types are unchanged, and the `?v=` hashes are not pinned.

### Changes

#### 1. Replace the three phone screenshot bytes

**File**: `static/singlethread-shot-main.jpg`, `static/singlethread-shot-swipe.jpg`, `static/singlethread-shot-settings.jpg`
**Action**: modify (overwrite bytes)

All three sources are already JPEG, so a byte copy is correct — **do not
re-encode** and do not resize these three.

```bash
cp ~/Downloads/F2C642A5-AC11-4251-ACD9-0754CEF497FC_1_105_c.jpeg static/singlethread-shot-main.jpg
cp ~/Downloads/F409E32A-C612-4F76-9C70-D65A312D182F_1_105_c.jpeg static/singlethread-shot-swipe.jpg
cp ~/Downloads/AF8D1A8C-FB43-42B2-B719-9939E02BD7E5_4_5005_c.jpeg static/singlethread-shot-settings.jpg
```

Confirm each landed: `file static/singlethread-shot-*.jpg` must report
`601x1306` (main, swipe) and `282x612` (settings).

#### 2. Fix the two now-false descriptions

**File**: `templates/singlethread.html`
**Action**: modify

The settings image is a full settings **list**, not a sheet; and the swipe-slot
image is a **dark-mode** screen showing complete/skip **buttons**, not a
light-mode swipe gesture. Change exactly these two strings:

```html
<!-- L48: settings alt -->
alt="The SingleThread settings list"

<!-- L53–55: swipe alt + figcaption -->
alt="Completing or skipping a reminder in dark mode"
<figcaption class="text-muted text-sm mt-2 text-center">Complete or skip</figcaption>
```

The `main.jpg` slot's `alt="One reminder at a time on iPhone"` / caption
`One reminder at a time` remain accurate — leave them.

#### 3. Lock the copy in with assertions

**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify

In `index_serves_ok_html` (starts L152), immediately after the existing
screenshot assertions (L190–195), add the two copy assertions. This matches the
file's established convention of asserting rendered strings (cf. the existing
`assert!(body.contains("stored on your device"))`). Neither string contains a
character that minijinja autoescapes, so no escaping is needed here.

```rust
        // copy corrects to match the VAR-1012 screenshots: the settings slot is
        // a list screen, and the action screen is dark mode with buttons
        assert!(body.contains(r#"alt="The SingleThread settings list""#));
        assert!(body.contains("Complete or skip"));
```

### Verification

#### Automated
- [x] `./scripts/test.sh` passes (fmt, sqlx prepare, check, CSS drift, clippy, nextest, TODO grep)
- [x] `file static/singlethread-shot-main.jpg static/singlethread-shot-swipe.jpg static/singlethread-shot-settings.jpg` reports `601x1306`, `601x1306`, `282x612`
- [x] `git diff --stat static/site.css` is empty (no CSS drift from this phase)
- [x] `cargo nextest run --locked index_serves_ok_html` passes

#### Manual
- [ ] `cargo run`, open `/singlethread`; the first row shows: dark "Pick up milk" (caption *One reminder at a time*), light **Settings** list (caption *Settings*), dark "Clean bathroom" with the three-button bar (caption *Complete or skip*). No broken images.
- [ ] Each of the three `<img src>` in the served HTML ends `?v=` followed by 12 hex chars, and the three hashes differ from each other and from `origin/main`'s values.

#### Commit
- [x] `git add static/singlethread-shot-main.jpg static/singlethread-shot-swipe.jpg static/singlethread-shot-settings.jpg templates/singlethread.html src/interfaces/handlers/singlethread/web.rs && git commit -m "Swap SingleThread phone screenshots for the VAR-1012 set"`

---

## Phase 2: Wrist row — one watch shot, plus the iPad shot as a fourth phone-row figure

Removes the retired `singlethread-watch-list.png` (no new image is a Watch list
screen), converts the real Watch actions image into the `.png` slot as true PNG,
and gives the iPad landscape image a correctly-named slot and figure. This is the
only phase that touches the test surface.

### Changes

#### 1. Create the iPad asset and convert the watch slot

**File**: `static/singlethread-shot-ipad.jpg`, `static/singlethread-watch-detail.png`
**Action**: create / modify

`sips` is the macOS system tool already present at `/usr/bin/sips` — no repo
script and no new dependency is introduced. Downscale the iPad shot so a
1024×768 photo does not sit in the repo at full size; the card renders it at
≤224 px wide.

```bash
# iPad landscape (1024x768) -> new slot, downscaled to 640x480
sips -s format jpeg -Z 640 ~/Downloads/6F6D3A52-8405-486D-93E9-C7AA338E6DCB_1_105_c.jpeg --out static/singlethread-shot-ipad.jpg
# Apple Watch actions (359x440) -> real PNG, because the slot filename is .png
sips -s format png ~/Downloads/70EA8DBF-0A47-4B7C-B340-F120929A994D_4_5005_c.jpeg --out static/singlethread-watch-detail.png
```

Then verify the conversions produced genuine formats:

```bash
file static/singlethread-shot-ipad.jpg static/singlethread-watch-detail.png
```

Expect `JPEG image data ... 640x480` and `PNG image data ... 359 x 440`. If the
PNG exceeds ~400 KB, re-run with `sips -Z 880` before the format conversion and
re-check. **If `file` still reports JPEG for the `.png` path, the conversion
failed — do not continue**, because the served `Content-Type` would then lie
about the bytes.

#### 2. Retire the Watch list asset

**File**: `static/singlethread-watch-list.png`
**Action**: delete

```bash
git rm static/singlethread-watch-list.png
```

Its `<div>` in the template is removed in change 3 below. Do this **in the same
working state** as that template edit — with the file gone and the reference
still present, `asset_url()` panics and every page test fails.

#### 3. Template: add the iPad figure, collapse the wrist row to one card

**File**: `templates/singlethread.html`
**Action**: modify

Add a fourth figure to the existing phone row, inside the row's `</div>`,
immediately after the swipe figure's `</figure>` (the swipe figure now ends near
L56). Reuse the neighbouring classes verbatim — this is what keeps
`static/site.css` untouched:

```html
    <figure class="card m-0 flex-1 basis-[10rem] max-w-[14rem] p-3">
        <img src="{{ asset_url('singlethread-shot-ipad.jpg') }}" alt="One reminder at a time on iPad"
             class="w-full rounded-lg border border-neutral-700">
        <figcaption class="text-muted text-sm mt-2 text-center">On iPad</figcaption>
    </figure>
```

Then replace the entire wrist block (currently L60–71) — two cards — with the
single remaining card, and correct its alt text to the actions actually shown:

```html
<h2 class="heading-subsection">On your wrist</h2>
<div class="flex justify-center gap-6">
    <div class="card max-w-[12rem] p-3">
        <img src="{{ asset_url('singlethread-watch-detail.png') }}" alt="Apple Watch showing Skip, Reschedule, and Delete actions"
             class="w-full rounded-lg border border-neutral-700">
    </div>
</div>
```

#### 4. Update the HTML assertions

**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify

In `index_serves_ok_html`, replace the retired asset's assertion (L194) with the
new iPad slot, keep the `watch-detail` assertion (L195), and add the corrected
watch alt. Before:

```rust
        assert!(body.contains(r#"<img src="/static/singlethread-watch-list.png?v="#));
        assert!(body.contains(r#"<img src="/static/singlethread-watch-detail.png?v="#));
```

After:

```rust
        assert!(body.contains(r#"<img src="/static/singlethread-shot-ipad.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/singlethread-watch-detail.png?v="#));
        assert!(body.contains("Apple Watch showing Skip, Reschedule, and Delete actions"));
```

#### 5. Update the asset-serving table and add the retirement sad path

**File**: `src/interfaces/routes.rs`
**Action**: modify

In `singlethread_screenshots_are_served_with_immutable_caching` (L215), the
`cases` array gains the iPad slot in place of the retired file. Before:

```rust
            ("/static/singlethread-shot-swipe.jpg", "image/jpeg"),
            ("/static/singlethread-watch-list.png", "image/png"),
            ("/static/singlethread-watch-detail.png", "image/png"),
```

After:

```rust
            ("/static/singlethread-shot-swipe.jpg", "image/jpeg"),
            ("/static/singlethread-shot-ipad.jpg", "image/jpeg"),
            ("/static/singlethread-watch-detail.png", "image/png"),
```

Then add the sad path directly after that test, asserting the retired asset is
genuinely gone rather than silently still served. The tests module already has
`use crate::test::{start_app, test_client};` (L75) and `StatusCode` in scope, and
`ServeDir` answers a missing file with 404:

```rust
    #[tokio::test]
    async fn retired_singlethread_watch_list_shot_returns_404() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/static/singlethread-watch-list.png"))
            .send()
            .await
            .expect("request should complete");
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
```

### Verification

#### Automated
- [x] `./scripts/test.sh` passes (fmt, sqlx prepare, check, CSS drift, clippy, nextest, TODO grep)
- [x] `file static/singlethread-shot-ipad.jpg static/singlethread-watch-detail.png` reports `JPEG image data ... 640x480` and `PNG image data ... 359 x 440`
- [x] `ls static/singlethread-watch-list.png` reports no such file
- [x] `git diff --exit-code -- static/site.css` passes (no class change, so no CSS regeneration)
- [x] `cargo nextest run --locked singlethread` passes, including the new `retired_singlethread_watch_list_shot_returns_404` and the extended caching test
- [x] `rg -n "singlethread-watch-list" templates/ src/` returns nothing (sole hit is the intentional 404-test URL in routes.rs:252, per supervisor ruling)

#### Manual
- [ ] `cargo run`, open `/singlethread`; the first row now shows **four** cards: dark "Pick up milk", light Settings list, dark "Clean bathroom" action bar, and the **landscape iPad** card (caption *On iPad*) — the iPad card must not be stretched or letterboxed.
- [ ] The **On your wrist** section shows exactly **one** card: the Apple Watch with Skip / Reschedule / Delete, and the heading is still correct for what it contains.
- [ ] `/static/singlethread-watch-list.png` returns 404 in the browser while `/singlethread` still renders 200 with no panic.
- [ ] The newly added iPad and watch `<img src>` values each end in a fresh `?v=` 12-hex hash.

#### Commit
- [x] `git add static/singlethread-shot-ipad.jpg static/singlethread-watch-detail.png templates/singlethread.html src/interfaces/handlers/singlethread/web.rs src/interfaces/routes.rs && git commit -m "Add iPad screenshot, retire stale Apple Watch list shot"` — the `git rm` of the retired file is already staged from change 2.

---

## Out of scope

- `ROUTES.md`: no route path or parameter changes, so no update is required.
- Making the iPad card wider than `max-w-[14rem]`, or any other Tailwind class
  change: would require regenerating and committing `static/site.css`. Reuse the
  existing classes.
- Re-encoding or resizing the three phone JPEGs beyond the copies in Phase 1.
- Repairs to the unrelated `DELETEME` deletion and the untracked artifacts dir.
- Any refactor of `asset_url`, the static-serving layer, or the test helpers.