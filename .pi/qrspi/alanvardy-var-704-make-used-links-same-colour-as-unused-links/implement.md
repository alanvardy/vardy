# Implementation Summary

## Commits

| Phase | Commit | Description |
|-------|--------|-------------|
| 1     | a317f9c | Make used links same colour as unused links (branch placeholder; `css/site.css` was already present in the base, so the restore was a no-op — the file was never actually missing on this branch) |
| 2     | a600f3e | Phase 2: Add global anchor rules and remove nav-scoped overrides |
| 3     | 94c2c34 | Phase 3: Remove redundant utility classes from templates |

## Automated Checks

- [x] `./scripts/build-css.sh` succeeds with no errors
- [x] `git diff --exit-code -- static/site.css` — zero drift (builder output matches committed output)
- [x] `./scripts/test.sh` — full gate green: format, cargo check, CSS build/drift, clippy, 64 tests pass, no forgotten TODOs
- [x] Global `a` / `a:visited` / `a:hover` rules present in `css/site.css`; `nav a` / `nav a:hover` overrides removed
- [x] `templates/home.html` GitHub and LinkedIn anchors stripped of `no-underline` / `hover:text-accent`

## Manual Verification Items (from the plan)

### Phase 1
- [ ] No visual change when running `cargo run` — site looks identical to production

### Phase 2
- [ ] `cargo run`, open home page
- [ ] **Nav links** (Home, SingleThread) render in accent orange (`#fb923c`), not the previous `--color-text` muted colour
- [ ] **Content links** (GitHub, LinkedIn) render in accent orange (no longer blue/purple)
- [ ] **Visited links** on all pages show the **same orange** as unvisited — no browser purple anywhere (confirm in devtools: inspect computed `color`, search for `:visited` purple)
- [ ] **Hover** on any link brightens to `--color-accent-strong` (`#fdba74`)
- [ ] No underline on any link in default or hover state

### Phase 3
- [ ] `cargo run`, open home page
- [ ] GitHub and LinkedIn links still render in accent orange with **no underline** (now provided by global `a` rules, not utility classes)
- [ ] Hover still brightens (provided by global `a:hover` rule)
- [ ] Inspect rendered HTML in devtools → the `<a>` class attributes now read `class="flex items-center gap-2 py-2"` — no `no-underline` or `hover:text-accent`
- [ ] SingleThread page unchanged — no regressions

## Notes for Review

- `css/site.css` was already present in the base of this branch (the `DELETEME` placeholder commit `a317f9c` became the Phase 1 commit because the "restore" was effectively already satisfied). Phase 2 then applied the actual global-anchor changes.
- No Rust code, routes, `ROUTES.md`, or migration changes were needed, as planned.
