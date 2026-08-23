# Structure Outline

## Approach
Introduce the Tailwind v4 standalone CLI as a pinned, checksum-verified build step that
compiles a CSS-first source file (`@import "tailwindcss"` + `@theme`) into the committed
`static/site.css`, gated by a rebuild-and-diff drift check in `scripts/test.sh` and mirrored
in the Dockerfile builder stage. Once the pipeline is proven invisible (all existing tests
green), redesign pages one at a time — each phase swaps one template to pure utility
classes, removes its legacy CSS, updates its handler-test body assertions, and ends with a
full `./scripts/test.sh` run.

Source/output split: **input** `css/site.css` (new, repo root — outside `static/` so the raw
source is never served), **output** `static/site.css` (committed, fingerprinted by the
existing `asset_url` mechanism, untouched).

---

## Phase 1: Tailwind pipeline with zero visual change

Delivers the toolchain end-to-end: pinned CLI → compiled CSS → committed artifact → drift
gate → Docker build. The source file initially contains the *current* hand-rolled rules
appended after `@import "tailwindcss"`, plus `@theme` tokens mirroring the existing palette,
so every existing test stays green and the rendered site is visually identical.

**Files**: `css/site.css` (new), `static/site.css` (now generated, committed),
`scripts/test.sh`, `Dockerfile`, `ROUTES.md` (no change expected — confirm `/static`
section still accurate)
**Key changes**:
- `scripts/build-css.sh` (new) — downloads/pins CLI (`TAILWIND_VERSION=v4.x.y` +
  sha256 per platform: macOS arm64 / linux x64), runs `tailwindcss -i css/site.css -o static/site.css --minify`
- `scripts/test.sh`: insert `./scripts/build-css.sh && git diff --exit-code static/site.css`
  **before** `cargo nextest run` (drift gate ordering risk from design.md)
- `Dockerfile` builder stage: `ARG TAILWIND_VERSION=v4.x.y`, checksum-verified download,
  build step before `COPY . .` artifacts reach runtime (which copies `static/` verbatim)
- Source `css/site.css`: `@import "tailwindcss"; @theme { --color-bg: #121212; --color-surface:
  #1e1e1e; --color-text: #e0e0e0; --color-muted: #9e9e9e; --color-accent: #7aa2f7; }` +
  current rules verbatim
- No Rust/template changes — `asset_url('site.css')` picks up the new hash automatically

**Verify**: `./scripts/test.sh` passes (including the new drift gate); `docker build .`
succeeds; `curl localhost:PORT/static/site.css` → 200, `text/css`,
`max-age=31536000, immutable`; manual diff of rendered home/singlethread pages vs. current
(no visual change).

---

## Phase 2: Route all home-page images through `asset_url`

Small independent slice fixing the cache-busting debt: migrate the four hard-coded URLs in
`home.html` to the versioned global and flip the verbatim test assertions to the `?v=`
shape. Purely mechanical; de-risks Phase 3's larger template rewrite.

**Files**: `templates/home.html`, `src/interfaces/handlers/home/web.rs`
**Key changes**:
- `home.html:4,26,28,31`: `src="/static/wave.svg"` → `src="{{ asset_url('wave.svg') }}"`
  (same for alanvardy.jpg, github.svg, linkedin.svg)
- `home/web.rs:45-50`: assertions change from exact unversioned strings to contains
  `/static/<name>?v=` + 12-hex shape (mirroring `singlethread/web.rs:46-50`)

**Verify**: `cargo nextest run` passes; manual: view-source of `/` shows `?v=` on all five
images.

---

## Phase 3: Redesign the home page with Tailwind utilities

First real vertical slice of the redesign: rewrite `home.html` markup with pure utility
classes (new layout/spacing/typography, dark identity from `@theme` tokens), delete the
home-only legacy rules from `css/site.css`, update handler body assertions.

**Files**: `templates/home.html`, `css/site.css` (+ regenerated `static/site.css`),
`src/interfaces/handlers/home/web.rs`
**Key changes**:
- `home.html`: class attributes become Tailwind utilities only (`bg-bg text-text`,
  `bg-accent`, etc.) — no `.home`, `.home-columns`, `.portrait`, `.invite-list`,
  `.invite-icon`, `.wave` remnants
- `css/site.css`: remove `.home*`, `.portrait`, `.section-heading`, `.invite-*`, `.wave`
  rules (keep `.section-heading` temporarily — singlethread still uses it until Phase 4)
- `home/web.rs`: body assertions rewritten for new markup; keep asserting status + body

**Verify**: `cargo nextest run` passes; `rg 'class="[^"]*\b(home|portrait|invite-)' templates/`
empty; manual review at mobile + desktop widths; screenshots in PR description.

---

## Phase 4: Redesign the SingleThread page and finish the CSS cutover

Second redesign slice: same treatment for `singlethread.html`, which removes the last
legacy consumers and lets the source file shed all remaining hand-rolled rules, including
the 48rem media query (superseded by Tailwind default breakpoints).

**Files**: `templates/singlethread.html`, `css/site.css` (+ regenerated `static/site.css`),
`src/interfaces/handlers/singlethread/web.rs`
**Key changes**:
- `singlethread.html`: all `st-*` classes replaced by utilities; drop the unstyled
  `.st-watch` wrapper (Decision 6)
- `css/site.css`: remove `.st-*` rules and the entire `@media (max-width: 48rem)` block;
  file is now only `@import`, `@theme`, and any few global element defaults (body/base)
- `singlethread/web.rs`: body assertions rewritten

**Verify**: `cargo nextest run` passes; `rg 'st-|section-heading|@media' css/site.css`
empty; `./scripts/test.sh` full gate green; manual mobile/desktop review + screenshots.

---

## Phase 5: Dead-code sweep and supersession bookkeeping

Final cleanup slice: remove orphaned assets, confirm the "no component classes / no
hard-coded static URLs" invariants hold everywhere, and record that VAR-682 supersedes the
VAR-657/VAR-664/VAR-670 no-build-step decisions (design doc already notes this — verify
nothing else needs updating).

**Files**: `static/quill.png` (delete), `.gitignore` if needed, `ROUTES.md` (confirm no
changes), PR description (screenshots)
**Key changes**:
- Delete `static/quill.png`; grep confirms no template/test references remain
- Invariant checks: `rg '/static/' templates/` returns only `{{ asset_url(...) }}` forms;
  `rg '@apply' css/ templates/` empty; no unused `.card` rule anywhere

**Verify**: `./scripts/test.sh` green end-to-end on a clean checkout; `grep -E 'TODO|FIXME'`
gate passes; manual: deploy preview or local server, click through both pages.

---

## Testing Checkpoints

- **After Phase 1**: All pre-existing tests pass unmodified; drift gate active in
  `test.sh`; Docker image builds; site visually unchanged. *Resume point: pipeline done,
  templates still legacy.*
- **After Phase 2**: Home handler tests assert `?v=` shapes; all template images versioned.
- **After Phase 3**: Home redesigned; home-only legacy CSS gone; singlethread untouched and
  still rendering correctly.
- **After Phase 4**: Both pages on pure utilities; `css/site.css` reduced to import + theme
  + minimal base; media query gone.
- **After Phase 5**: No dead assets/classes; full gate green from clean checkout — ready
  for PR review with screenshots.
