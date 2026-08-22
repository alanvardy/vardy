# Structure Outline

## Approach
Rebuild `/singlethread` as a static marketing page by landing renamed screenshot assets in `static/` first (asset_url panics otherwise), then building the page top-down in template+CSS+test slices using new page-scoped `st-*` classes on existing dark-theme tokens. No DB, no handler/context changes; all images via versioned `{{ asset_url(...) }}`.

## Phase 1: Assets land with serving tests
Copy and rename the five screenshots into `static/` (kebab-case lowercase). Nothing references them yet, so no template risk — but the static pipeline can be fully verified end-to-end.

**Files**: `static/singlethread-shot-main.png`, `static/singlethread-shot-settings.png`, `static/singlethread-shot-swipe.png`, `static/singlethread-watch-list.png`, `static/singlethread-watch-detail.png`, `src/interfaces/routes.rs` (tests only)

**Key changes**:
- New tests mirroring existing patterns in `routes.rs`: each PNG served with status 200, `content-type: image/png`, and `cache-control: public, max-age=31536000, immutable`

**Verify**: `./scripts/test.sh` passes; manually confirm files exist and hashes resolve (`curl localhost:<port>/static/singlethread-shot-main.png?v=<hash>` returns the image).

---

## Phase 2: Hero section
Replace the current icon+card body with the hero: h1/tagline text beside `singlethread-shot-main.png`, stacking at `max-width: 48rem`. Update handler assertions for the removed "single line of work" copy in the same slice so tests stay green.

**Files**: `templates/singlethread.html`, `static/site.css`, `src/interfaces/handlers/singlethread/web.rs`

**Key changes**:
- `.st-hero { display: flex; ... }` / `.st-hero-text` / `.st-hero-shot` — new classes, flex pattern cloned from `.home-columns` (`site.css:57-62`), stacked inside the existing media query block
- `<img src="{{ asset_url('singlethread-shot-main.png') }}" alt="...">` — first versioned reference to a Phase 1 asset
- Handler test: drop `"single line of work"` assertion; add tagline phrase + `?v=` URL assertion for the hero image

**Verify**: `./scripts/test.sh` passes; manual: resize browser below 48rem — hero stacks with image above/below text per design.

---

## Phase 3: Screenshot row + Watch pair
Add the three-phone responsive screenshot row (main/settings/swipe) and the Apple Watch subsection pairing the two small watch images side by side.

**Files**: `templates/singlethread.html`, `static/site.css`, `src/interfaces/handlers/singlethread/web.rs`

**Key changes**:
- `.st-shots { display: flex; gap; flex-wrap: wrap }` / `.st-shot img { border-radius; border: 1px solid ... }` — portrait-style treatment per `.portrait` precedent (`site.css:64-68`)
- `.st-watch-pair` — two-up small-image layout
- Four more `{{ asset_url('singlethread-*.png') }}` references with descriptive `alt`s
- Test: assert all four remaining versioned image URLs appear in body

**Verify**: `./scripts/test.sh` passes; manual: check rounded corners/borders on screenshots, watch images sit side by side, row wraps on narrow viewports.

---

## Phase 4: Feature prose sections + final copy
Fill in the marketing copy sections: "Why it helps", "Everything you need, nothing you don't", "Thoughtful by design", closing paragraph + "Your reminders. One at a time. In order. At your pace." tagline line. Copy is frozen at this point — this is the last phase allowed to touch wording.

**Files**: `templates/singlethread.html`, `static/site.css`, `src/interfaces/handlers/singlethread/web.rs`

**Key changes**:
- Reuse/clone `.section-heading` muted-heading style for the three section titles
- Bullet-list styling under `.st-*` scope (no color literals; `--muted`/`--accent` tokens only)
- Handler test rewritten around stable phrases: tagline, "One at a time", all three section headings

**Verify**: `./scripts/test.sh` passes; manual read-through of rendered page for copy completeness against the provided text.

---

## Testing Checkpoints
- **After Phase 1**: All five PNGs served via `/static` with correct content-type and immutable caching; zero template changes yet — page renders exactly as before.
- **After Phase 2**: Page has hero with versioned hero image; old copy assertions fully replaced; mobile stacking works.
- **After Phase 3**: Every one of the five assets is referenced via `asset_url` and asserted in the handler test.
- **After Phase 4**: Full copy present; `./scripts/test.sh` green end-to-end; final live-server visual review. ROUTES.md needs no edit (content-only change, confirmed in research).

## Notes
- No phase touches DB, routes, metrics labels, or the render context — this work cannot fail in ways that break other pages; `home/web.rs:49` nav assertion is the only cross-file coupling and is untouched.
- If page weight feels heavy at visual review (design risk #1), downscaling exports is an asset-only swap thanks to hash-based cache busting — handle as follow-up, not a phase here.
