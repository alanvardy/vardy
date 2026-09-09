# Structure Outline

## Approach

YAML-only change to three GitHub Actions workflows (no Rust code, no
migrations, no handlers). The repo gate (`./scripts/test.sh`) is Rust-only and
cannot validate these files, so the horizontal foundation is a **verification
harness** (Layer 0), proven first. Each workflow file is then an independent,
fully-verified layer ordered bottom-up by risk — additive change first,
security-critical trigger switch last — so a failure at any layer leaves all
earlier layers mergeable on their own.

Two correctness details the design's shorthand glosses over are called out at
Layer 2 and Layer 3 (both flow mechanically from decisions already made; no
design re-run needed).

---

## Layer 0: Verification harness (foundation)

Proves we can detect invalid YAML/schema and permission drift before touching
the real files. Nothing shipped here — it defines the red/green instrumentation
every later layer's checkpoint leans on.

**Files**: none edited (establishes commands), `.github/workflows/*.yml` as read-only inputs.

**Key changes**: canonical verify incantation, fixed once here and reused:
- `actionlint .github/workflows/<file>.yml` — YAML syntax + workflow schema +
  permission-scope validity + expression lint (installed at
  `/opt/homebrew/bin/actionlint`).
- Permission invariants (grep assertions):
  - `grep -n "actions: write" .github/workflows/*.yml` → currently
    `ci.yml:30`; must return nothing after Layer 2.
  - `grep -n "contents: write" .github/workflows/dependabot_auto_merge.yml`
    → currently `:11`; must return nothing after Layer 3.

**Tests**: establish the baseline so the harness is proven able to catch the
drift it will later require to vanish:
- actionlint on the three *unchanged* files: 0 **error**-level findings.
  (Documented: 2 pre-existing `info`-level shellcheck findings at
  `ci.yml:45`, `ci.yml:116` — unrelated `run:` blocks, untouched by this
  change; the green bar is "no error findings, and no findings on changed
  lines".)
- Assert the greps currently *do* match (`actions: write` @ ci.yml:30,
  `contents: write` @ dependabot:11) — confirming the detector fires.

**Verify**: `actionlint .github/workflows/ci.yml .github/workflows/fly-deploy.yml .github/workflows/dependabot_auto_merge.yml` → only the 2 known info findings; `grep -n "actions: write\|contents: write" .github/workflows/*.yml` shows the two known hits.

---

## Layer 1: fly-deploy.yml — explicit read-only posture

Makes the fly.io template self-describing: add a workflow-level
`permissions: contents: read`. No functional change (checkout already only
needs read); removes silent dependence on repo/org defaults.

**Files**: `.github/workflows/fly-deploy.yml`

**Key changes** — insert after the `on:` block (workflow level, so the single
`deploy` job inherits it):
```yaml
permissions:
  contents: read    # checkout needs read only; deploy auth is FLY_API_TOKEN
```

**Tests**: actionlint clean for this file (0 error findings); presence
assertion `grep -n "contents: read" .github/workflows/fly-deploy.yml` matches;
no `GITHUB_TOKEN` reference introduced.

**Verify**: `actionlint .github/workflows/fly-deploy.yml` → 0 error findings;
`./scripts/test.sh` still green (Rust gate untouched by YAML-only change).

---

## Layer 2: ci.yml — narrow baseline, escalate only the `test` job

Baseline drops to `contents: read`; `actions: write` (no consumer) removed;
`pull-requests: write` moves to the `test` job, the only job with codecov
upload steps.

**Files**: `.github/workflows/ci.yml`

**Key changes** — workflow baseline (`:27-30`):
```yaml
permissions:
  contents: read    # checkout everywhere; no job needs more by default
```
`test` job gains a job-level block. **Correction to note**: job-level
`permissions` *replace* the baseline, so omitting `contents: read` here would
silently revoke checkout for the `test` job (`contents` → `none`). The
job-level block must re-declare it:
```yaml
  test:
    name: Cargo CI Tests
    runs-on: ubuntu-latest
    permissions:
      contents: read        # checkout
      pull-requests: write  # codecov test_results upload on PRs (:79)
    steps: ...
```
No other job changes: `todos`, `fmt`, `clippy` inherit the `contents: read`
baseline; `css-drift` keeps its existing `contents: read` override unchanged.

**Tests**: actionlint clean (0 error findings on this file, and none on the
changed `permissions:` lines); asserted `grep -n "actions: write" .github/workflows/ci.yml` returns nothing; baseline block contains exactly
`contents: read`.

**Verify**: `actionlint .github/workflows/ci.yml`; `grep -n "actions: write" .github/workflows/ci.yml` → empty; `./scripts/test.sh` green. Real CI confirmation deferred to Layer 4 (codecov + checkout only prove out on a PR run).

---

## Layer 3: dependabot_auto_merge.yml — trim, switch trigger, add guard

Highest-risk layer: corrects the token posture so dependabot PRs are actually
approvable/auto-mergeable, and hardens the workflow against running on
untrusted head refs.

**Files**: `.github/workflows/dependabot_auto_merge.yml`

**Key changes**:
1. Permissions trim (`:9-11`) — drop the unreachable `contents: write`:
   ```yaml
   permissions:
     pull-requests: write  # approve + enable auto-merge (API), contents: write unreachable
     contents: read        # no checkout: read-only stay safe if a step is added
   ```
2. Trigger switch (`:7`): `on: [pull_request]` → `on: pull_request_target`
   (base-branch context, write-capable token).
3. **Correction to note**: the existing job-level `if`
   (`if: ${{ github.event_name == 'pull_request'}}`) becomes permanently false
   once the trigger changes — `github.event_name` is then
   `pull_request_target`, so the job would never run. Replace it with the
   actor guard (which the design already mandates) and drop the now-redundant
   event check:
   ```yaml
     auto-merge:
       if: ${{ github.actor == 'dependabot[bot]' }}
       runs-on: ubuntu-latest
   ```
4. Inline comment on the `on:` block flagging the RCE-risk invariant: no step
   may ever check out the PR head in a `pull_request_target` workflow.

**Tests**: actionlint clean (0 error findings; trigger/guard/permission lines
valid); asserted `grep -n "contents: write" .github/workflows/dependabot_auto_merge.yml` returns nothing; `grep -n "pull_request_target\|dependabot\[bot\]" .github/workflows/dependabot_auto_merge.yml` matches both.

**Verify**: `actionlint .github/workflows/dependabot_auto_merge.yml`;
`grep -n "contents: write" .github/workflows/dependabot_auto_merge.yml` → empty; `./scripts/test.sh` green. True functional confirmation (auto-merge
actually fires) only happens on the *next* dependabot PR after merge — noted in
the PR description, not gateable locally.

---

## Layer 4: Cross-file audit + CI confirmation

Whole-surface sweep proving the three files collectively meet the design's end
state, then the real-world confirmation only a GitHub run can give.

**Files**: none edited (aggregate verification of Layers 1–3).

**Key changes**: none — asserts the combined invariant across `ci.yml`,
`fly-deploy.yml`, `dependabot_auto_merge.yml`.

**Tests / Verify**:
- `actionlint .github/workflows/ci.yml .github/workflows/fly-deploy.yml .github/workflows/dependabot_auto_merge.yml` → 0 error findings, only the 2 known info findings.
- `grep -R "actions: write" .github/workflows` → nothing.
- `grep -n "contents: write" .github/workflows/*.yml` → only the out-of-scope `rust-version-bump.yml:13` remains (documented follow-up).
- Workflow-level blocks now contain only `contents: read` across the three files (except dependabot's justified `pull-requests: write`).
- `./scripts/test.sh` green (Rust gate).
- Open PR → confirm `push` + `pull_request` CI stays green; css-drift and codecov uploads succeed under the narrowed token. Dependabot auto-merge left as post-merge follow-up.

---

## Testing Checkpoints (resume if context resets)

- **After L0**: actionlint baseline documented; greps fire on current state.
- **After L1**: `actionlint fly-deploy.yml` clean; `contents: read` present.
- **After L2**: `actionlint ci.yml` clean; `grep "actions: write"` empty on ci.yml.
- **After L3**: `actionlint dependabot_auto_merge.yml` clean; `grep "contents: write"` empty; guard + `pull_request_target` present.
- **After L4**: full `./scripts/test.sh` green + PR CI green; cross-file grep sweep passes.