# Design — VAR-682: Adopt Tailwind CSS v4 via standalone CLI

## Current State

Zero-build Rust web app (axum + minijinja). Styling is a single hand-rolled
`static/site.css` (198 lines) with a dark-only `:root` palette (`--bg #121212`,
`--surface #1e1e1e`, `--text #e0e0e0`, `--muted #9e9e9e`, `--accent #7aa2f7`,
`static/site.css:1-7`), ~30 semantic classes, and one `@media (max-width: 48rem)`
block (`static/site.css:181-198`). No npm/node/PostCSS anywhere; the Dockerfile
builder stage has no asset step (`Dockerfile:8-17`).

Asset pipeline: `assets::asset_url` returns `/static/{file}?v={sha256-12hex}`,
hashed once at startup into a `OnceLock` map, panicking on unknown files
(`src/app/assets.rs:37-44`). `/static` is served via `ServeDir` with overriding
`Cache-Control: public, max-age=31536000, immutable`
(`src/interfaces/routes.rs:30-36`). Correctness depends entirely on URLs
changing when file contents change — the built CSS must land at
`static/site.css` so `asset_url('site.css')` (`templates/layout.html:7`) keeps
working and all fingerprint/cache tests keep passing.

Known debt this task touches:
- `home.html` hard-codes 4 unversioned `/static/` URLs (wave.svg :4,
  alanvardy.jpg :28, github.svg :26, linkedin.svg :31), bypassing cache
  busting; handler tests assert them verbatim (`home/web.rs:45-50`).
- `.card` defined but unused (`site.css:27-32`); `.st-watch` used but unstyled
  (`singlethread.html:45`); `static/quill.png` referenced by nothing.
- Three prior design docs (VAR-657, VAR-664, VAR-670) rejected build steps and
  npm tooling. **VAR-682 supersedes those decisions** (recorded in
  `alanvardy-var-664.../design.md`).

## Desired End State

- Templates styled with **Tailwind v4 utility classes only** (no component
  classes); `site.css` becomes a Tailwind v4 CSS-first source file
  (`@import "tailwindcss"` + `@theme` tokens) compiled by the pinned standalone
  CLI into `static/site.css`.
- A **complete visual redesign**: new layout, spacing, and typography — dark
  theme identity retained (near-black background, light text, blue accent) via
  `@theme` tokens, but no obligation to replicate current markup or spacing.
- The committed `static/site.css` is always in sync with source (drift gate in
  `scripts/test.sh`); Docker rebuilds it in the builder stage.
- All images referenced through `asset_url()`; hard-coded URLs gone.
- Dead CSS/assets removed. All tests pass; `./scripts/test.sh` green.

Verification: `./scripts/test.sh` passes; `curl /static/site.css` returns 200
`text/css` with immutable caching; pages render the redesigned UI correctly at
mobile and desktop widths; grep finds no hard-coded `/static/` URLs in
templates and no `@apply`/component classes in templates.

## Patterns to Follow

- **Versioned asset URLs via `asset_url`** for every static reference —
  `templates/singlethread.html:11,21,24` is the pattern; hard-coded
  `/static/...` in `home.html:4,26,28,31` is the anti-pattern to eliminate
  (VAR-668 design.md:48-55 says the same).
- **Fail-fast asset discipline**: new static files must exist under `static/`
  before first render — `asset_url` panics on unknown names
  (`src/app/assets.rs:41-42`). Build CSS before running the app/tests.
- **Pinned toolchain culture**: mirror `rust-toolchain.toml` pinning and
  `clippy --locked` determinism — pin the Tailwind CLI version + sha256.
- **Gates chained in `scripts/test.sh`** with `&&` (`scripts/test.sh:1-19`);
  add the CSS build + drift check there, not only in CI.
- **Inline `#[cfg(test)]` tests** asserting both status and body
  (`src/interfaces/routes.rs:188-211` style) for any new behavior.
- **ROUTES.md `---` block convention** if `/static` docs change
  (`AGENTS.md:44-46`).
- Do **not** follow: per-request hashing, graceful degradation on missing
  assets, manual version bumps (VAR-670 "Patterns NOT to Follow").

## Design Decisions

1. **Pure utility classes in templates** (no `@utility`/component layer).
   Verbose but idiomatic Tailwind v4; one styling mental model; no custom-CSS
   indirection. Repeated combos are accepted duplication.
2. **Commit the built `static/site.css`**; add a drift gate to
   `scripts/test.sh` (rebuild → `git diff --exit-code static/site.css`).
   Tests/CI keep working with zero tooling; Docker still builds fresh.
3. **Complete visual redesign**, not a pixel-faithful port. Dark identity
   (`--color-bg`, `--color-surface`, `--color-text`, `--color-muted`,
   `--color-accent`) defined once in `@theme`; layout/spacing/typography are
   free. Use Tailwind default breakpoints (drop the 48rem media query).
4. **Fix hard-coded home page images**: migrate `home.html:4,26,28,31` to
   `asset_url()`; update the verbatim assertions in `home/web.rs:45-50` to
   assert the `?v=` shape (mirroring `singlethread/web.rs:46-50`).
5. **Pinned CLI + sha256 verification**: `ARG TAILWIND_VERSION=v4.x.y` +
   checksum-verified download in the Dockerfile builder stage; same pinned
   curl command documented for local dev (macOS arm64 / linux x64).
6. **Cleanup in scope**: delete `.card` rule, `.st-watch` wrapper class,
   `static/quill.png`, and any redesign-orphaned assets/classes.
7. **Supersede prior no-build-step decisions**: note in this doc (done above);
   no edits to old design docs.

## What We're NOT Doing

- No Node/npm/package.json — standalone CLI binary only.
- No light mode, `prefers-color-scheme`, or theme toggle.
- No JavaScript, no external fonts.
- No changes to the fingerprinting scheme, `ServeDir` setup, or
  `Cache-Control` headers (VAR-670 stands).
- No build-time fingerprinting or CI-only asset pipeline.
- No new routes; no handler/logic changes beyond template context if any.
- No migration of old design docs; supersession recorded here only.

## Open Risks

- **Redesign verification is subjective** — no pixel baseline exists. Mitigate:
  manual review at mobile + desktop widths before PR; screenshots in PR
  description.
- **CLI download in Docker build** needs network at image build time; Fly
  remote builds must reach github.com releases. If blocked, vendor the binary
  (follow-up decision).
- **Drift gate ordering**: `cargo nextest run` reads `static/site.css`; the
  rebuild must run before tests so a stale committed file fails the diff check
  loudly rather than serving stale styles silently.
- **Tailwind class detection**: v4 scans source files automatically; template
  paths (`templates/`) must be discoverable from repo root where the CLI runs.
  Verify content detection includes `.html`.
- **Test churn**: any test asserting on removed class names or markup
  (`home/web.rs`, `singlethread/web.rs` body assertions) must be updated in
  the same change; expect a wider-than-usual diff in tests.
