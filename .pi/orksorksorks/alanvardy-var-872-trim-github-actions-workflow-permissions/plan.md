# Implementation Plan — Trim GitHub Actions Workflow Permissions

## Overview

Minimize the declared permissions on three GitHub Actions workflows (`ci.yml`,
`fly-deploy.yml`, `dependabot_auto_merge.yml`) so each grants only what its
jobs actually use, while making fly-deploy's posture explicit and switching
dependabot auto-merge to `pull_request_target` so it can actually approve
dependabot PRs. Pure YAML change — no Rust code, migrations, or handlers.

---

## Canonical verification incantation (fixed once, reused everywhere)

`actionlint` is installed at `/opt/homebrew/bin/actionlint` (v1.7.12).

- **Lint one file**: `/opt/homebrew/bin/actionlint .github/workflows/<file>.yml`
- **Exit-0 gate** (hides the 2 pre-existing shellcheck findings, see Layer 0):
  `/opt/homebrew/bin/actionlint -ignore 'SC2086' .github/workflows/<file>.yml; echo $status` → `0`
- **Rust gate**: `./scripts/test.sh` (does **not** validate YAML/actions files —
  it is the Rust project gate only).
- **Repo root**: `/Users/vardy/dev/alanvardy-var-872-trim-github-actions-workflow-permissions`

---

## Layer 0: Verification harness (foundation)

No files edited. Establishes the red/green baseline every later layer's
checkpoint leans on.

### Changes

None — these commands are the instrumentation. Nothing shipped here.

### Verification

#### Automated
- [x] `/opt/homebrew/bin/actionlint .github/workflows/ci.yml .github/workflows/fly-deploy.yml .github/workflows/dependabot_auto_merge.yml` reports **0 error-level findings** on changed lines. Expected output is *exactly* the 2 pre-existing info-level shellcheck findings, both `SC2086` ("Double quote to prevent globbing and word splitting"):
  - `ci.yml:45:9` (mold-install `run:` block)
  - `ci.yml:116:9` (mold-install `run:` block in the clippy job)
  These are in unrelated `run:` blocks we never touch — the green bar is "no error findings, and no findings on changed lines", **not** "empty output".
- [x] `/opt/homebrew/bin/actionlint -ignore 'SC2086' .github/workflows/ci.yml .github/workflows/fly-deploy.yml .github/workflows/dependabot_auto_merge.yml; echo $status` prints `0` (exit-0 form of the same check).
- [x] `grep -n "actions: write" .github/workflows/*.yml` currently matches exactly `ci.yml:30` — confirms the detector fires and will catch Layer 2's removal.
- [x] `grep -n "contents: write" .github/workflows/dependabot_auto_merge.yml` currently matches `:11` — confirms the detector fires and will catch Layer 3's removal.

#### Manual
- [ ] Confirm the two grep assertions show the *known* hits (ci.yml:30, dependabot_auto_merge.yml:11) — the harness is proven able to catch the drift it will later require to vanish.

**Checkpoint**: actionlint baseline documented; greps fire on current state.

---

## Layer 1: fly-deploy.yml — explicit read-only posture

**File**: `.github/workflows/fly-deploy.yml` (modify)

Add a workflow-level `permissions: contents: read` so the fly.io template is
self-describing and independent of repo/org defaults. No functional change —
`actions/checkout@v7` only needs read, and deploy auth is `secrets.FLY_API_TOKEN`.

### Changes

#### 1. Insert workflow-level `permissions:` between the `on:` block and `jobs:`

**File**: `.github/workflows/fly-deploy.yml`
**Action**: modify

```yaml
name: Fly Deploy
on:
  workflow_run:
    workflows: [CI]
    types: [completed]
    branches: [main]
permissions:
  contents: read    # checkout needs read only; deploy auth is FLY_API_TOKEN
jobs:
  deploy:
```

The single `deploy` job (no job-level `permissions:`) inherits this baseline.

### Verification

#### Automated
- [x] `/opt/homebrew/bin/actionlint -ignore 'SC2086' .github/workflows/fly-deploy.yml; echo $status` → `0` (0 error findings).
- [x] `grep -n "contents: read" .github/workflows/fly-deploy.yml` matches the new line.
- [x] `grep -n "GITHUB_TOKEN" .github/workflows/fly-deploy.yml` → empty (no token reference introduced).

#### Manual
- [ ] Read the diff: `permissions: contents: read` sits at workflow level (between `on:` and `jobs:`); no other lines touched.
- [ ] `./scripts/test.sh` still green (Rust gate untouched by a YAML-only change).

**Checkpoint**: `actionlint fly-deploy.yml` clean; `contents: read` present.

---

## Layer 2: ci.yml — narrow baseline, escalate only the `test` job

**File**: `.github/workflows/ci.yml` (modify)

Baseline drops to `contents: read`; `actions: write` (no consumer) removed;
`pull-requests: write` moves to the `test` job — the only job with codecov
upload steps (`ci.yml:79`, the `report_type: test_results` upload).

### Changes

#### 1. Workflow-level baseline (currently `ci.yml:27-30`)

**File**: `.github/workflows/ci.yml`
**Action**: modify

Replace:
```yaml
permissions:
  contents: read
  pull-requests: write
  actions: write
```
With:
```yaml
permissions:
  contents: read    # checkout everywhere; no job needs more by default
```

#### 2. `test` job gains a job-level block (replacement semantics!)

**File**: `.github/workflows/ci.yml`
**Action**: modify

Replace the `test` job header:
```yaml
  test:
    name: Cargo CI Tests
    runs-on: ubuntu-latest
    steps:
```
With:
```yaml
  test:
    name: Cargo CI Tests
    runs-on: ubuntu-latest
    permissions:
      contents: read        # checkout
      pull-requests: write  # codecov test_results upload on PRs (:79)
    steps:
```

> **Why `contents: read` must be re-declared here**: a job-level `permissions:`
> block *replaces* the workflow baseline entirely — omitting `contents: read`
> would silently revoke checkout for the `test` job (`contents` → `none`).
> This is the correction flagged in `structure.md` Layer 2.

#### No other job changes

`todos`, `fmt`, `clippy` inherit the `contents: read` baseline (they only
checkout + run Rust tooling). `css-drift` keeps its existing job-level
`contents: read` override (`ci.yml:131-132`) unchanged.

### Verification

#### Automated
- [x] `/opt/homebrew/bin/actionlint -ignore 'SC2086' .github/workflows/ci.yml; echo $status` → `0`.
- [x] `/opt/homebrew/bin/actionlint .github/workflows/ci.yml` → only the 2 known `SC2086` info findings at `ci.yml:46` and `ci.yml:117` (shifted +1 from `:45`/`:116` by the new `test` job block); **no** finding on any changed `permissions:`/`test:` line.
- [x] `grep -n "actions: write" .github/workflows/ci.yml` → empty.
- [x] `grep -n "pull-requests: write" .github/workflows/ci.yml` → matches only inside the `test` job block, not the workflow baseline.
- [x] `grep -n "contents: read" .github/workflows/ci.yml` → matches the baseline + `test` job + `css-drift` job (3 places).

#### Manual
- [ ] Read the diff: baseline is exactly the single `contents: read` line; `test` job block lists `contents: read` then `pull-requests: write`; `css-drift` untouched.
- [ ] `./scripts/test.sh` green.

**Checkpoint**: `actionlint ci.yml` clean; `grep "actions: write"` empty on ci.yml.

---

## Layer 3: dependabot_auto_merge.yml — trim, switch trigger, add guard

**File**: `.github/workflows/dependabot_auto_merge.yml` (modify)

Highest-risk layer. Changes the token posture so dependabot PRs are actually
approvable/auto-mergeable, and hardens against untrusted head refs.

### Changes

#### 1. Trigger switch + RCE-risk inline comment

**File**: `.github/workflows/dependabot_auto_merge.yml`
**Action**: modify

Replace:
```yaml
on: [pull_request]
```
With:
```yaml
# pull_request_target runs in the base branch's context with a write-capable
# GITHUB_TOKEN — required to approve/auto-merge dependabot PRs.
# SECURITY: never add a checkout of the PR head in this workflow (RCE risk).
on:
  pull_request_target:
```

#### 2. Permissions trim (currently `:9-11`) — drop unreachable `contents: write`

**File**: `.github/workflows/dependabot_auto_merge.yml`
**Action**: modify

Replace:
```yaml
permissions:
  pull-requests: write
  contents: write
```
With:
```yaml
permissions:
  pull-requests: write  # approve + enable auto-merge (API); a contents write token is unreachable here
  contents: read        # no checkout step — read-only if one is ever added
```

#### 3. Replace the job-level `if` guard

**File**: `.github/workflows/dependabot_auto_merge.yml`
**Action**: modify

Replace:
```yaml
    if: ${{ github.event_name == 'pull_request'}}
```
With:
```yaml
    if: ${{ github.actor == 'dependabot[bot]' }}
```

> **Why**: once the trigger is `pull_request_target`, `github.event_name` is
> permanently `pull_request_target`, so the old `== 'pull_request'` check is
> always false and the job would never run. The actor guard (which the design
> already mandates) is both the correct security check and the correct
> "only for dependabot" predicate — it also finally makes the file's header
> comment (`"This job will only run for pull requests created by Dependabot"`)
> actually true.

### Verification

#### Automated
- [x] `/opt/homebrew/bin/actionlint -ignore 'SC2086' .github/workflows/dependabot_auto_merge.yml; echo $status` → `0`.
- [x] `/opt/homebrew/bin/actionlint .github/workflows/dependabot_auto_merge.yml` → 0 findings on this file (trigger/guard/permission lines all valid).
- [x] `grep -n "contents: write" .github/workflows/dependabot_auto_merge.yml` → empty.
- [x] `grep -n "pull_request_target" .github/workflows/dependabot_auto_merge.yml` matches the new trigger.
- [x] `grep -n "dependabot\[bot\]" .github/workflows/dependabot_auto_merge.yml` matches the `if:` guard.

#### Manual
- [ ] Read the full file: header comment preserved; `on: pull_request_target:`; permissions are `pull-requests: write` + `contents: read`; `if: github.actor == 'dependabot[bot]'`; action + inputs unchanged.
- [ ] `./scripts/test.sh` green.

**Checkpoint**: `actionlint dependabot_auto_merge.yml` clean; `grep "contents: write"` empty; guard + `pull_request_target` present.

---

## Layer 4: Cross-file audit + CI confirmation

No files edited. Whole-surface sweep proving the three files collectively meet
the design's end state, then the real-world confirmation only a GitHub run can
give.

### Changes

None.

### Verification

#### Automated
- [x] `/opt/homebrew/bin/actionlint -ignore 'SC2086' .github/workflows/ci.yml .github/workflows/fly-deploy.yml .github/workflows/dependabot_auto_merge.yml; echo $status` → `0`.
- [x] `/opt/homebrew/bin/actionlint .github/workflows/ci.yml .github/workflows/fly-deploy.yml .github/workflows/dependabot_auto_merge.yml` → only the 2 known `SC2086` info findings (`ci.yml:46`, `ci.yml:117` — shifted +1 from `:45`/`:116` by the Layer 2 `test` job block), no error findings, nothing on changed lines.
- [x] `grep -R "actions: write" .github/workflows` → nothing (was `ci.yml:30`).
- [x] `grep -R "contents: write" .github/workflows` → only `rust-version-bump.yml:13` remains (documented out-of-scope follow-up).
- [x] Workflow-level `permissions:` blocks across the three files contain only `contents: read`, except dependabot's justified `pull-requests: write`.
- [x] `./scripts/test.sh` green (Rust gate).
- [x] `git diff --exit-code --stat -- .github/workflows/ci.yml .github/workflows/fly-deploy.yml .github/workflows/dependabot_auto_merge.yml` shows exactly 3 files changed (no accidental edits elsewhere).

#### Manual
- [ ] Open PR; confirm `push` (main) + `pull_request` CI jobs stay green, and `css-drift` + both codecov uploads succeed under the narrowed token.
- [ ] Dependabot auto-merge left as post-merge follow-up — first dependabot PR after merge is the real functional confirmation (not gateable locally). Note this in the PR description.

**Checkpoint**: full `./scripts/test.sh` green + PR CI green; cross-file grep sweep passes.

---

## Testing Checkpoints (resume if context resets)

- **After L0**: actionlint baseline documented; greps fire on current state.
- **After L1**: `actionlint fly-deploy.yml` clean; `contents: read` present.
- **After L2**: `actionlint ci.yml` clean; `grep "actions: write"` empty on ci.yml.
- **After L3**: `actionlint dependabot_auto_merge.yml` clean; `grep "contents: write"` empty; guard + `pull_request_target` present.
- **After L4**: full `./scripts/test.sh` green + PR CI green; cross-file grep sweep passes.

## Out of scope (do NOT touch)

- `ci-secure.yml` (`packages: read` unconsumed) and `rust-version-bump.yml`
  (redundant block under MYTOKEN PAT) — documented follow-ups.
- No action-pinning changes (preserve all SHA/tag/branch pins exactly, incl.
  `superfly/flyctl-actions@master`).
- No trigger/`concurrency:`/`env:`/secret changes except the dependabot trigger.
- No new actions, no removal of `fastify/github-action-merge-dependabot@v3`.