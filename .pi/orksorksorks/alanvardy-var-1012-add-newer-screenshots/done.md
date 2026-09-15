# Done

- **Branch / head SHA**: `alanvardy-var-1012-add-newer-screenshots` @ `d9a26df`
  (pushed to origin; `--force-with-lease` available but the last push was a
  fast-forward).
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
- **Remaining manual items**: the plan's browser checks are still open and
  require `cargo run` —
  1. `/singlethread` first row: dark "Pick up milk" (*One reminder at a time*),
     light-ish **Settings** list (*Settings*), dark "Clean bathroom" action bar
     (*Complete or skip*), landscape iPad card (*On iPad*), no broken images.
  2. Wrapped-card centering at tablet width (~768-968px) looks right (this is
     what the `justify-center` addition addresses).
  3. **On your wrist** shows exactly one card (Skip / Reschedule / Delete).
  4. `/static/singlethread-watch-list.png` returns 404 in the browser while
     `/singlethread` still renders 200 with no panic.
  5. Each screenshot `<img src>` ends in a fresh 12-hex `?v=` hash.

Image contents were independently re-verified against the template `alt` text
during review: `settings.jpg` = Settings list, `swipe.jpg` = "Clean bathroom"
with complete/mic/skip bar, `ipad.jpg` = landscape action bar, `watch-detail.png`
= Skip/Reschedule/Delete, `main.jpg` = "Pick up milk". All formats match their
extensions (iPad JPEG 640x480; main/swipe JPEG 601x1306; settings JPEG 282x612;
watch-detail PNG 359x440).
