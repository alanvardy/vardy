# Structure Outline

## Approach

Three vertical slices, each crossing CSS source → build output → test gate →
visual verification. Every phase produces a committed, pipeline-passing state
that can be deployed independently. Phase 1 restores the build pipeline; Phase 2
delivers the bug fix; Phase 3 cleans up dead utility classes.

---

## Phase 1: Restore `css/site.css` and Verify Pipeline Health

Restore the missing Tailwind v4 source file verbatim from `ee48110`. Rebuild
`static/site.css` to confirm the pipeline is healthy and the output matches
exactly what's committed at HEAD. No visual change — this phase proves the
foundation is sound before we touch styling.

**Files**: `css/site.css` (new), `static/site.css` (rebuilt, should be identical)

**Key changes**:
- `css/site.css` — restored verbatim from `git show ee48110:css/site.css`
- `static/site.css` — regenerated via `./scripts/build-css.sh`; must be
  byte-identical to HEAD (no drift)

**Verify**:
```fish
./scripts/build-css.sh                    # must succeed
git diff --exit-code -- static/site.css    # no drift — output matches HEAD
./scripts/test.sh                          # full gate green
```

---

## Phase 2: Add Global Anchor Rules and Remove Nav-Scoped Overrides

Add a global `a`, `a:visited`, and `a:hover` ruleset to `@layer base` using
`var(--color-accent)`, then remove the now-redundant `nav a` / `nav a:hover`
rules. Rebuild `static/site.css` and commit both source + output. After this
phase, **every anchor on every page** renders in accent orange, visited links
are indistinguishable from unvisited, and hover brightens to
`--color-accent-strong`. The bug is fixed.

**Files**: `css/site.css` (edited), `static/site.css` (rebuilt)

**Key changes**:

In `@layer base`:
- **Add**:
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
- **Remove**:
  ```css
  nav a {
      color: var(--color-text);
      text-decoration: none;
  }

  nav a:hover {
      color: var(--color-accent-strong);
  }
  ```

**Verify**:
```fish
./scripts/build-css.sh                    # rebuild with new rules
git add css/site.css static/site.css && git commit -m "..."
./scripts/test.sh                          # CSS drift gate green; all tests pass
```

Manual: `cargo run`, open home page. Nav links = orange, content links =
orange, visited content links = same orange (not purple), hover = brighter
orange. Inspect devtools → no `:visited` purple anywhere.

---

## Phase 3: Remove Redundant Utility Classes from Templates

Strip `no-underline` and `hover:text-accent` from the two content anchors in
`home.html` — the global `a` rule now covers both. Rebuild
`static/site.css` so Tailwind's scanner drops any now-unused utility.
Visually identical to Phase 2; this is a cleanup pass.

**Files**: `templates/home.html` (edited), `static/site.css` (rebuilt)

**Key changes**:

In `templates/home.html`, lines 23-24 (GitHub link) and 31-32 (LinkedIn link):
- **Before**:
  ```html
  class="flex items-center gap-2 py-2 no-underline hover:text-accent"
  ```
- **After**:
  ```html
  class="flex items-center gap-2 py-2"
  ```

`static/site.css` regenerated → `hover:text-accent` utility may disappear from
the compiled output if no other element uses it (`text-accent` is still used
on `singlethread.html:71` and stays). No other files change.

**Verify**:
```fish
./scripts/build-css.sh                    # rebuild after template edit
git add templates/home.html static/site.css && git commit -m "..."
./scripts/test.sh                          # CSS drift gate green; all tests pass
```

Manual: `cargo run`, confirm GitHub and LinkedIn links still render orange with
no underline and brighten on hover. Inspect rendered HTML → `no-underline` and
`hover:text-accent` gone from the `<a>` class attributes.

---

## Testing Checkpoints

| After Phase | Assertion |
|---|---|
| 1 | `./scripts/test.sh` passes. `css/site.css` exists on disk. Pipeline healthy. |
| 2 | `./scripts/test.sh` passes. All links orange; `:visited` = `:link` colour; hover brightens. Bug fixed. |
| 3 | `./scripts/test.sh` passes. Templates clean. Visually identical to Phase 2. |