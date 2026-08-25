# Implementation Plan

## Overview

Add a standalone `css-drift` job to `.github/workflows/ci.yml` that rebuilds `static/site.css` via `scripts/build-css.sh` and fails if the committed artifact is out of date. The job runs in parallel with existing jobs — no `needs:`.

## Phase 1: Add `css-drift` job to CI workflow

### Changes

#### 1. Add `css-drift` job entry
**File**: `.github/workflows/ci.yml`
**Action**: modify

Add a new job entry after the `clippy` job (or after any sibling — order doesn't matter since they all run in parallel). Insert it before the end of the `jobs:` map.

```yaml
  css-drift:
    name: CSS Drift Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - name: Rebuild static/site.css from source
        run: ./scripts/build-css.sh
      - name: Check for uncommitted CSS changes (drift)
        run: git diff --exit-code -- static/site.css
```

**Exact insertion point**: After the `clippy` job's closing line and before the end of file. The `clippy` job ends with:

```yaml
      - run: cargo clippy --all-targets --all-features --locked -- -D warnings
```

Append the new `css-drift` job block immediately after that line.

**No other files change.** `scripts/build-css.sh`, `scripts/test.sh`, `Dockerfile`, and all Rust source files are untouched.

### Verification

#### Automated
- [x] `cat .github/workflows/ci.yml` shows the new `css-drift` job with correct YAML syntax
- [x] `yamllint .github/workflows/ci.yml` passes (if available; otherwise visual YAML review)
- [x] `./scripts/test.sh` passes locally (local gate unchanged)

#### Manual
- [ ] Push a branch that modifies `css/site.css` without rebuilding `static/site.css` → open a PR → confirm the `css-drift` job **fails** with a non-empty diff in the step output
- [ ] In the same branch, run `./scripts/build-css.sh && git add static/site.css && git commit -m "rebuild" && git push` → confirm the `css-drift` job **passes**
- [ ] Push a branch with no CSS changes at all → confirm the `css-drift` job **passes**
- [ ] Confirm the `css-drift` job runs in the same parallel group as `test`, `todos`, `fmt`, `clippy` (all four jobs start concurrently; no `needs:` dependency)
- [ ] Confirm all other CI jobs (`test`, `todos`, `fmt`, `clippy`) continue to pass unchanged