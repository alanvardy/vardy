# Implementation Plan

## Overview

Two CSS-only responsive fixes at the `md` breakpoint (768px): the contact form column spans full width on mobile, and the wallpaper background + credit bubble are hidden below 768px. Implemented entirely via Tailwind utility classes in templates, with regenerated `static/site.css` committed to satisfy the CSS drift gate.

---

## Phase 1: Template markup — utility class additions

Add four utility classes across two templates. These drive the Tailwind utility scan; Stage 2 regenerates the compiled CSS from them.

### Changes

#### 1. Contact form column — full-width on mobile
**File**: `templates/contact.html`
**Action**: modify

**Line 18**, current:
```html
    <div class="md:flex-1">
```
Change to:
```html
    <div class="w-full md:w-auto md:flex-1">
```

`w-full` stretches the form column to full container width in the stacked mobile layout (where `items-start` on the wrapper prevents cross-axis stretching). `md:w-auto` restores auto-sizing at ≥768px so `md:flex-1` controls the column split.

#### 2. Wallpaper div — hidden on mobile
**File**: `templates/layout.html`
**Action**: modify

**Line 14**, current:
```html
    <div class="wallpaper" aria-hidden="true" {% if wallpaper_url %}style="background-image: url('{{ wallpaper_url }}')"{% endif %}></div>
```
Change to:
```html
    <div class="wallpaper hidden md:block" aria-hidden="true" {% if wallpaper_url %}style="background-image: url('{{ wallpaper_url }}')"{% endif %}></div>
```

`hidden` (`display: none`) on all viewports; `md:block` (`display: block`) at ≥768px. The `{% if wallpaper_url %}` guard on the inline `style` attribute is unchanged.

#### 3. Credit bubble — hidden on mobile
**File**: `templates/layout.html`
**Action**: modify

**Lines 18**, current:
```html
    <div class="fixed bottom-3 right-3 px-3 py-1.5 rounded bg-black/50 text-sm">
```
Change to:
```html
    <div class="hidden md:block fixed bottom-3 right-3 px-3 py-1.5 rounded bg-black/50 text-sm">
```

Same `hidden md:block` pattern. The `{% if photographer %}` guard (line 15) still controls content presence — no credit bubble markup is emitted at all when photographer is empty.

### Verification

#### Manual
- [ ] `git diff -- templates/` shows exactly 3 hunks with 4 utility-class additions and no other template changes
- [ ] `templates/contact.html:18`: `w-full md:w-auto md:flex-1`
- [ ] `templates/layout.html:14`: `wallpaper hidden md:block`
- [ ] `templates/layout.html:18`: `hidden md:block fixed bottom-3 ...`

#### Automated
- None at this stage — `scripts/test.sh` will fail on the CSS drift check because `static/site.css` hasn't been regenerated yet. Proceed to Phase 2.

---

## Phase 2: CSS regeneration — compile and commit `static/site.css`

Run the Tailwind CLI to regenerate `static/site.css` from the updated templates, then commit the result.

### Changes

#### 1. Regenerate compiled CSS
**File**: `static/site.css`
**Action**: regenerate via `scripts/build-css.sh`

Run:
```fish
./scripts/build-css.sh
```

This scans `templates/` for utility classes and emits them into the compiled output. Expected new or updated entries:

- `@layer utilities`: `hidden` (`display: none`), `block` (`display: block`), `w-full` (`width: 100%`), `w-auto` (`width: auto`) — these likely already exist in the compiled output as base utilities
- `md:` variant group (`@media (min-width:48rem)`): `md:block` (`display: block`), `md:w-auto` (`width: auto`) — these may be **new** since no current template uses `md:block` or `md:w-auto`
- The `asset_url` content hash (`?v=<12-hex>`) changes because the file bytes changed

### Verification

#### Automated
- [ ] `./scripts/build-css.sh && git diff --exit-code -- static/site.css` passes (exit 0) — confirms the regenerated file matches what was just built

---

## Phase 3: Full test gate — verify nothing is broken

Run the project's complete test gate. Includes CSS drift check (Phase 2's regenerated file must be committed), type check, clippy, tests, and forgotten-TODO grep.

### Changes

**Files**: None changed — verification only.

### Verification

#### Automated
- [ ] `./scripts/test.sh` passes (exit 0)

This runs in order:
1. `cargo fmt --all`
2. `cargo sqlx prepare -- --tests`
3. `cargo check --all-targets`
4. `./scripts/build-css.sh && git diff --exit-code -- static/site.css` (CSS drift gate)
5. `cargo clippy --all-targets --all-features --locked -- -D warnings`
6. `cargo nextest run`
7. `! rg -i -s -g '*.rs' 'FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO\s|todo\s' src`

All existing tests pass unchanged:
- Contact handler tests: `get_contact_returns_200_with_form`, POST success, honeypot rejection, resend 502, 429 rate limit
- Home handler tests: wallpaper + linked credit, inline `url()` in style attribute, no-URL photographer plain text, fetch-failure suppression
- Singlethread handler tests: same three wallpaper scenarios, FAQ `<details>` structure, asset versioned `src`s, nav anchors, negative legacy-class checks
- Route tests: `/static/site.css` served with `Cache-Control` and `text/css`
- Asset tests: `asset_url` produces 12-hex-char version, panics on unknown files, deterministic hashing
- Template tests: `asset_url` resolves in minijinja template rendering

#### Manual
- [ ] No clippy warnings, no forgotten TODOs, no test failures
- [ ] `git status` shows only the three changed files: `templates/contact.html`, `templates/layout.html`, `static/site.css`

---

## What This Change Touches (and Doesn't)

| Layer | Changed? | Why |
|-------|----------|-----|
| Schema / migrations | No | No new tables, columns, or data |
| Store / repository | No | No data-access methods needed |
| Service / business logic | No | No processing or validation changes |
| Handlers / routes | No | Responsive behavior is CSS-only |
| Templates | **Yes** | Phase 1: `contact.html` + `layout.html` |
| Compiled CSS | **Yes** | Phase 2: `static/site.css` regenerated |

---

## Testing Checkpoints

- [x] **Phase 1**: `git diff -- templates/` shows only the four utility-class additions in `templates/contact.html` and `templates/layout.html`
- [ ] **Phase 2**: `./scripts/build-css.sh && git diff --exit-code -- static/site.css` passes (exit 0)
- [ ] **Phase 3**: `./scripts/test.sh` passes (exit 0, all tests green, no CSS drift)