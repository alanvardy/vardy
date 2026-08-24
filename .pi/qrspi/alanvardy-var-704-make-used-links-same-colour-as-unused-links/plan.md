# Implementation Plan

## Overview

Restore the missing `css/site.css` Tailwind v4 source, add global anchor
rules so every link renders in accent orange (visited == unvisited), then
clean up now-redundant utility classes from `home.html` links. Three
commits, each passing `./scripts/test.sh`.

---

## Phase 1: Restore `css/site.css` and Verify Pipeline Health

### Changes

#### 1. `css/site.css` — restore from `ee48110`
**File**: `css/site.css`
**Action**: create (new file)

```bash
mkdir -p css
git show ee48110:css/site.css > css/site.css
```

> The file contents are the verbatim Tailwind v4 source from the last-known-good commit.
> Do not edit yet — changes happen in Phase 2.

#### 2. `static/site.css` — rebuild
**File**: `static/site.css`
**Action**: regenerate via build script

```fish
./scripts/build-css.sh
```

The script downloads Tailwind v4.3.3 CLI, then compiles `css/site.css -o static/site.css --minify`.
Output must be byte-identical to what is currently committed at HEAD (the drift check enforces this).

### Verification

#### Automated
- [x] `./scripts/build-css.sh` succeeds with no errors
- [x] `git diff --exit-code -- static/site.css` — zero diff (output matches HEAD exactly)
- [x] `./scripts/test.sh` — all phases pass: format, sqlx prepare, check, CSS build, CSS drift, clippy, tests, forgotten TODOs

#### Manual
- [ ] Confirm `css/site.css` exists on disk and matches the content from `git show ee48110:css/site.css`
- [ ] No visual change when running `cargo run` — site looks identical to production

---

## Phase 2: Add Global Anchor Rules and Remove Nav-Scoped Overrides

### Changes

#### 1. `css/site.css` — add global `a` rules, remove `nav a` / `nav a:hover`
**File**: `css/site.css`
**Action**: modify

In the `@layer base` block, add after the `body` rule block:

```css
    a {
        color: var(--color-accent);
        text-decoration: none;
    }

    a:visited {
        color: var(--color-accent);
    }

    a:hover {
        color: var(--color-accent-strong);
        text-decoration: none;
    }
```

**Remove** the `nav a` and `nav a:hover` blocks entirely:

```css
    nav a {
        color: var(--color-text);
        text-decoration: none;
    }

    nav a:hover {
        color: var(--color-accent-strong);
    }
```

> After this edit, `@layer base` still has `*, ::before, ::after`, `body`, `.wallpaper`,
> `.page`, `.container`, and `nav` — just without the `nav a` / `nav a:hover` children.

This is the only file change for this phase.

#### 2. `static/site.css` — rebuild
**File**: `static/site.css`
**Action**: regenerate via `./scripts/build-css.sh`

The compiled output will now contain the new `a`, `a:visited`, and `a:hover` rules
in `@layer base`, and `nav a` / `nav a:hover` will be gone. The byte change will
automatically rotate the `?v=` content hash.

### Verification

#### Automated
- [x] `./scripts/build-css.sh` succeeds
- [x] `./scripts/test.sh` — full gate green (CSS drift check passes since we're committing both source + output)

#### Manual
- [ ] `cargo run`, open home page
- [ ] **Nav links** (Home, SingleThread) render in accent orange (`#fb923c`), not the previous `--color-text` muted colour
- [ ] **Content links** (GitHub, LinkedIn) render in accent orange (no longer blue/purple)
- [ ] **Visited links** on all pages show the **same orange** as unvisited — no browser purple anywhere (confirm in devtools: inspect computed `color`, search for `:visited` purple)
- [ ] **Hover** on any link brightens to `--color-accent-strong` (`#fdba74`)
- [ ] No underline on any link in default or hover state

---

## Phase 3: Remove Redundant Utility Classes from Templates

### Changes

#### 1. `templates/home.html` — strip `no-underline` and `hover:text-accent` from content anchors
**File**: `templates/home.html`
**Action**: modify

Two `<a>` tags need identical edits:

**Line ~23** (GitHub link) and **Line ~31** (LinkedIn link):

Before:
```html
           class="flex items-center gap-2 py-2 no-underline hover:text-accent">
```

After:
```html
           class="flex items-center gap-2 py-2">
```

> The global `a` rule in `@layer base` now provides `text-decoration: none` and
> `a:hover { color: var(--color-accent-strong) }`, making these utilities redundant.
> Layout utilities (`flex items-center gap-2 py-2`) are preserved — only colour
> and decoration classes are removed.

This is the only file change for this phase.

#### 2. `static/site.css` — rebuild
**File**: `static/site.css`
**Action**: regenerate via `./scripts/build-css.sh`

Tailwind's class scanner runs across `templates/*.html`. After removing
`hover:text-accent` and `no-underline` from `home.html`, the scanner may drop
those utility classes from the compiled output if no other template uses them:

- `no-underline` — no other template uses it, so it **should disappear** from `static/site.css`
- `hover:text-accent` — no other template uses it, so it **should disappear**. Note: `text-accent` (the non-hover variant) is still used on `singlethread.html:71` and on list markers, so it stays.
- `text-accent` — stays (used on `singlethread.html`)
- All other utilities — unchanged

### Verification

#### Automated
- [x] `./scripts/build-css.sh` succeeds
- [x] `./scripts/test.sh` — full gate green

#### Manual
- [ ] `cargo run`, open home page
- [ ] GitHub and LinkedIn links still render in accent orange with **no underline** (now provided by global `a` rules, not utility classes)
- [ ] Hover still brightens (provided by global `a:hover` rule)
- [ ] Inspect rendered HTML in devtools → the `<a>` class attributes now read `class="flex items-center gap-2 py-2"` — no `no-underline` or `hover:text-accent`
- [ ] SingleThread page unchanged — no regressions

---

## Testing Checkpoints

| After Phase | Assertion |
|---|---|
| 1 | `./scripts/test.sh` passes. `css/site.css` exists on disk. Pipeline healthy. |
| 2 | `./scripts/test.sh` passes. All links orange; `:visited` = `:link` colour; hover brightens. Bug fixed. |
| 3 | `./scripts/test.sh` passes. Templates clean. Visually identical to Phase 2. |

## Notes

- **No Dockerfile changes needed** — the builder stage's inline `tailwindcss` invocation automatically picks up the restored `css/site.css` and new rules.
- **No Rust code changes** — `asset_url()`, routes, templates module, and cache headers are all untouched.
- **No new tests to add** — the existing CSS drift gate (`git diff --exit-code -- static/site.css`) catches any source/output mismatch, and the manual verification steps cover visual correctness.
- **No `ROUTES.md` changes** — no routes or parameters change.