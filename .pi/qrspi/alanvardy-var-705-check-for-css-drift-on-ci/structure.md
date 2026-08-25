# Structure Outline

## Approach

Add a standalone `css-drift` job to `.github/workflows/ci.yml` that rebuilds
`static/site.css` from source via the existing `scripts/build-css.sh` script,
then diffs against the committed artifact. A non-zero diff fails the job,
blocking PRs with stale CSS. No other files change — the script, test gate,
and Dockerfile are untouched.

---

## Phase 1: Add `css-drift` job to CI workflow

Delivers the complete drift check as a new sibling job in `ci.yml`. On every
PR and main push, the job rebuilds CSS and fails if the committed artifact is
out of date. It runs in parallel with `test`, `todos`, `fmt`, and `clippy`
(no `needs:`).

**Files**: `.github/workflows/ci.yml`

**Key changes**:
- New job entry `css-drift` in `jobs:` map, sibling to `test` / `todos` / `fmt` / `clippy`
- No new types or signatures — pure YAML CI configuration

**Steps**:
1. `actions/checkout@v7` — clone the repo
2. `Run ./scripts/build-css.sh` — downloads pinned Tailwind v4.3.3 CLI, verifies checksum, compiles `css/site.css` → `static/site.css`
3. `Run git diff --exit-code -- static/site.css` — fails job if the freshly-built artifact differs from the committed version

**Job shape** (pseudocode):
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

**Verification**:

| Scenario | Expected result |
|---|---|
| PR changes `css/site.css` but not `static/site.css` | `css-drift` job fails (non-zero diff) |
| PR changes neither CSS file | `css-drift` job passes (no diff) |
| PR changes both `css/site.css` and the rebuilt `static/site.css` | `css-drift` job passes (committed artifact matches build) |
| PR changes only `static/site.css` without source change | `css-drift` job fails (build overwrites the hand-edit) |

Manual verification checklist:
1. Push a branch that edits `css/site.css` without rebuilding `static/site.css` → open a PR → confirm the `css-drift` job fails with a non-empty diff in the step output
2. In the same branch, run `./scripts/build-css.sh && git add static/site.css && git commit -m "rebuild" && git push` → confirm the `css-drift` job passes
3. Run `./scripts/test.sh` locally — confirm the local gate still works unchanged

**Risk mitigations** (from design, verified by this phase):
- Platform determinism: `ubuntu-latest` is `Linux/x86_64`, which `build-css.sh` explicitly supports. If Tailwind output differs across platforms (macOS vs Linux), this phase surfaces it immediately.
- Network dependency: GitHub release download is the same pattern already used by the Dockerfile and local `build-css.sh` — no new dependency.
- Checksum integrity: `build-css.sh` verifies the downloaded binary's SHA-256 before running; CI inherits this guarantee.

---

## Testing Checkpoints

After Phase 1:
- [ ] A PR that modifies `css/site.css` without rebuilding `static/site.css` must have a **red** `css-drift` job
- [ ] A PR with consistent CSS files (or no CSS changes) must have a **green** `css-drift` job
- [ ] All other CI jobs (`test`, `todos`, `fmt`, `clippy`) continue to pass unchanged
- [ ] `./scripts/test.sh` still passes locally (the local gate is untouched)
- [ ] The `css-drift` job runs in parallel with other jobs (no `needs:` blocking), so total CI latency does not increase