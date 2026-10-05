# Implementation Plan

## Overview

Replace the broken `fastify/github-action-merge-dependabot@v3` dependency in
`.github/workflows/dependabot_auto_merge.yml` with GitHub's prescribed native
auto-merge pattern — a `pull_request_target` job gated on
`github.actor == 'dependabot[bot]'` that runs `gh pr merge --auto --rebase`
(with a `mergeStateStatus` branch so already-clean/armed/merged PRs are handled
correctly) — and make the repo `allow_auto_merge` setting an enforced,
repo-tracked prerequisite in `scripts/branch-protection.sh`.

No Rust code, routes, migrations, or `./scripts/test.sh`-visible files change.
All Dependabot updates (cargo + GitHub Actions, major/minor/patch) merge.

**Verification model for every phase (read this once):** there is no automated
test harness for shell/YAML in this repo. Each phase's automated gate is
`./scripts/test.sh` (must stay green — untouched by `.github/`) plus
`bash -n` / `shellcheck` / `actionlint` / `./scripts/branch-protection.sh verify`.
The *live* end-to-end proof (a Dependabot PR actually merging) is impossible
from this worktree: `pull_request_target` only runs from the default branch, so
the workflow changes take effect only after this PR merges to `main`. Each
phase's live checkpoint is therefore a **dry observation** via `gh api` /
`workflow_dispatch`, with the real merge confirmed on the next daily Dependabot
PR after merge to `main`.

**Phase order is fixed by `structure.md`.** No file outside the three listed
(`scripts/branch-protection.sh`, `.github/workflows/dependabot_auto_merge.yml`,
`AGENTS.md`) is touched.

---

## Phase 1: Repo `allow_auto_merge=true` enforced in branch-protection.sh (horizontal prerequisite)

This is the genuinely horizontal prerequisite (migration-like): no workflow
slice can arm auto-merge until `allow_auto_merge` is `true`, and the setting is
currently `false` (design-phase live check). This phase ships and verifies on
its own.

**File**: `scripts/branch-protection.sh` (modify)

### Changes

#### 1. Header description comment
**File**: `scripts/branch-protection.sh` lines 2–9
**Action**: modify — mention the auto-merge toggle among enforced settings.

Current:
```bash
# Verify and apply GitHub branch protection and repo merge settings for main.
#   verify  (default) — read-only check that protection + merge settings match
#   apply   — PUT protection + PATCH merge settings, then re-verify
# Requires: gh CLI (authenticated with repo/admin scope), jq (apply path only).
# The REQUIRED_CONTEXTS array below is the canonical list of required CI check
# contexts. After any CI job name: change in .github/workflows/, update
# REQUIRED_CONTEXTS and re-run `apply`.
```

Replace the `# Verify and apply …` line and add one line after `# Requires:`:
```bash
# Verify and apply GitHub branch protection, repo merge settings, and the
# "Allow auto-merge" toggle for main.
#   verify  (default) — read-only check that protection + merge settings match
#   apply   — PUT protection + PATCH merge settings, then re-verify
# Requires: gh CLI (authenticated with repo/admin scope), jq (apply path only).
# allow_auto_merge=true is enforced here because Dependabot native auto-merge
# (see .github/workflows/dependabot_auto_merge.yml) cannot arm without it.
# The REQUIRED_CONTEXTS array below is the canonical list of required CI check
# contexts. After any CI job name: change in .github/workflows/, update
# REQUIRED_CONTEXTS and re-run `apply`.
```

#### 2. `usage()` text
**File**: `scripts/branch-protection.sh` lines 34–47
**Action**: modify — list the toggle in the command summaries.

Change the two description lines to read:
```text
  verify  (default) read-only check that main protection + repo merge
          settings + allow_auto_merge match expected values
  apply   PUT protection + PATCH merge settings (incl. allow_auto_merge),
          then re-verify
```

#### 3. `verify_repo_settings()` — assert `allow_auto_merge`
**File**: `scripts/branch-protection.sh` lines ~156–176
**Action**: modify — read the toggle and `FAIL` unless `true`.

Extend the `local` declaration:
```bash
  local merge_commit squash_merge rebase_merge auto_merge
```
Add the read alongside the others:
```bash
  auto_merge=$(gh api "repos/$OWNER/$REPO_NAME" --jq '.allow_auto_merge')
```
Add the check after the `rebase_merge` block (mirroring its shape):
```bash
  if [ "$auto_merge" != "true" ]; then
    echo "FAIL: allow_auto_merge = $auto_merge (expected true — required for Dependabot native auto-merge)"
    mismatches=$((mismatches + 1))
  fi
```
Update the success line so the operator sees the toggle is covered:
```bash
  echo "OK: repo merge settings match expected config (rebase-only, auto-merge enabled)"
```
(Keep the `die "repo merge settings verification FAILED — …"` line unchanged.)

#### 4. `apply_repo_settings()` — set `allow_auto_merge`
**File**: `scripts/branch-protection.sh` lines ~193–202
**Action**: modify — add the field to the PATCH body.

Note: **do not** touch `can_approve_pull_request_reviews` (branch protection
requires 0 approvals; the toggle is irrelevant here).
```bash
apply_repo_settings() {
  gh api --silent -X PATCH "repos/$OWNER/$REPO_NAME" --input - <<EOF
{
  "allow_merge_commit": false,
  "allow_squash_merge": false,
  "allow_rebase_merge": true,
  "allow_auto_merge": true
}
EOF
  echo "OK: repo merge settings applied"
}
```

#### 5. `main()` apply summaries (both copies)
**File**: `scripts/branch-protection.sh` — the `apply` case and the `--yes|-y`
(non-interactive) case
**Action**: modify — add one bullet to each "This will:" block, after the
"Allow rebase merges only" line:
```bash
      echo "  - Enable auto-merge (required for Dependabot native auto-merge)"
```
Both blocks are byte-identical today, so apply the edit to both.

### Verification

#### Automated
- [x] `bash -n scripts/branch-protection.sh` → no syntax errors
- [x] `shellcheck scripts/branch-protection.sh` → no new warnings
- [x] `./scripts/branch-protection.sh verify` → exits 0 and its output includes `allow_auto_merge`
- [x] `./scripts/test.sh` → green (proves no repo-gate drift; does not touch `.github/`)
- [x] `gh api repos/alanvardy/vardy --jq '.allow_auto_merge'` → `true`

#### Manual
- [ ] Apply the setting if it is still off: `./scripts/branch-protection.sh apply --yes`
      (requires admin `gh` auth + `jq`), then re-run `verify` — it must pass.
      If the token lacks admin scope, enable **Settings → General → Pull Requests
      → Allow auto-merge** out-of-band; `verify` must then report `allow_auto_merge`.
- [ ] **Sad path**: temporarily flip the setting off
      (`gh api -X PATCH repos/alanvardy/vardy -f allow_auto_merge=false`), run
      `./scripts/branch-protection.sh verify` → exits non-zero with
      `FAIL: allow_auto_merge = false`; then restore with `apply --yes`.
      (If restoring via the script is not possible, PATCH it back directly.)

**Phase contract:** `allow_auto_merge` is `true` on `repos/alanvardy/vardy` and
asserted by `verify`; every later phase may assume it holds.

---

## Phase 2: Walking skeleton — Dependabot PR arms native auto-merge via `gh`

The thinnest real path with the riskiest integration front-loaded: a
Dependabot-authored PR event fires the base-branch workflow, `gh` queries the
PR, skips if already terminal, and arms native auto-merge with rebase. No
`actions/checkout`. A future Dependabot PR then merges with no human command.

**File**: `.github/workflows/dependabot_auto_merge.yml` (rewrite in place —
same path/name; delete the `fastify` action entirely)

### Changes

#### 1. Full file content (Phase 2 end state)
**File**: `.github/workflows/dependabot_auto_merge.yml`
**Action**: replace entire contents with:

```yaml
# Dependabot auto-merge.
#
# Arms GitHub native auto-merge for Dependabot-authored PRs. Once armed, GitHub
# merges the PR itself as soon as the branch-protection required status checks
# pass and the branch is current — no human command, no `@dependabot merge`.
#
# Prerequisite: the repository "Allow auto-merge" setting must be enabled.
# scripts/branch-protection.sh verifies/enforces allow_auto_merge=true.
#
# pull_request_target runs in the base branch's context with a write-capable
# GITHUB_TOKEN — required to enable auto-merge on Dependabot PRs (pull_request
# events from Dependabot receive a read-only token).
# SECURITY: never check out the PR head and never run PR code in this workflow
# (RCE risk under pull_request_target). This job uses the GitHub API only.

name: Dependabot Auto Merge

on:
  pull_request_target:

permissions:
  pull-requests: write  # enable auto-merge (API)
  contents: write       # required for the enablePullRequestAutoMerge GraphQL mutation

jobs:
  auto-merge:
    if: ${{ github.actor == 'dependabot[bot]' }}
    runs-on: ubuntu-latest
    env:
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
      PR: ${{ github.event.pull_request.number }}
    steps:
      - name: Arm native auto-merge
        run: |
          set -euo pipefail

          resp=$(gh pr view "$PR" --json state,mergeStateStatus,autoMergeRequest)
          state=$(jq -r '.state' <<<"$resp")
          armed=$(jq -r '.autoMergeRequest != null' <<<"$resp")

          if [ "$state" = "MERGED" ] || [ "$armed" = "true" ]; then
            echo "PR #$PR is $state (already armed: $armed) — nothing to do."
            exit 0
          fi

          gh pr merge "$PR" --auto --rebase
```

Notes on the code:
- `on: pull_request_target:` with no `types:` keeps the defaults
  (`opened`, `synchronize`, `reopened`) — `synchronize` re-arms after a
  Dependabot rebase.
- `mergeStateStatus` is fetched but not yet used in this phase; Phase 3 adds
  the branch. It is included now so the single `gh pr view` call shape is the
  interface later phases reuse.
- `${{ secrets.GITHUB_TOKEN }}` is the write-capable token for the base-branch
  context. `jq` is preinstalled on `ubuntu-latest` GitHub runners.
- No `actions/checkout` anywhere — the security posture is structural.

### Verification

#### Automated
- [x] `actionlint .github/workflows/dependabot_auto_merge.yml` → clean
- [x] `shellcheck` over the embedded `run:` block via actionlint (already
      covered by the actionlint run) → clean
- [x] `python3 -c 'import yaml,sys;yaml.safe_load(open(".github/workflows/dependabot_auto_merge.yml"))'` → no error
- [x] `./scripts/test.sh` → green (unaffected by `.github/` changes)
- [x] `gh workflow view dependabot_auto_merge.yml` → available, job path resolves
      (may report the workflow as missing until this change is on `main`; that
      is expected and is not a failure of this phase)

#### Manual
- [ ] Diff review: the `fastify/github-action-merge-dependabot@v3` dependency,
      its `target: 'minor'` semver gate, and its mandatory-approval path are gone.
- [ ] Confirm the file name is unchanged (`dependabot_auto_merge.yml`) and the
      job name is unchanged (`auto-merge`).
- [ ] Reason through the sad path: a non-Dependabot PR never reaches the job
      (`if:` gate); a non-owner/repo token gets a loud failure, not a masked
      `|| true`.
- [ ] **Live checkpoint (post-merge only):** on a real Dependabot PR,
      `gh pr view <n> --json autoMergeRequest` shows the request set
      (`autoMergeRequest` non-null).

**Phase contract:** workflow filename + `auto-merge` job name unchanged; PR
number sourced from `github.event.pull_request.number`; terminal-state guard
semantics (`MERGED` / `autoMergeRequest != null` → no-op `exit 0`) are the
interface Phases 3–4 reuse.

---

## Phase 3: Merge-state discrimination + concurrency (no spurious red, no duplicate runs)

Every reachable `mergeStateStatus` maps to exactly one action, and concurrent
events on one PR collapse to a single run.

**File**: `.github/workflows/dependabot_auto_merge.yml` (modify)

### Changes

#### 1. Add `concurrency` block
**File**: `.github/workflows/dependabot_auto_merge.yml`
**Action**: modify — insert after the `permissions:` block.

```yaml
concurrency:
  group: dependabot-auto-merge-${{ github.event.pull_request.number }}
  cancel-in-progress: true
```

#### 2. Branch on `mergeStateStatus`
**File**: `.github/workflows/dependabot_auto_merge.yml` — the `Arm native auto-merge` step
**Action**: modify — read `mergeStateStatus`, dispatch on it, and **remove the
blind `gh pr merge --auto --rebase` tail** (never wrap any of this in `|| true`).

Replace the step body with:

```yaml
      - name: Arm native auto-merge
        run: |
          set -euo pipefail

          resp=$(gh pr view "$PR" --json state,mergeStateStatus,autoMergeRequest)
          state=$(jq -r '.state' <<<"$resp")
          merge_state=$(jq -r '.mergeStateStatus' <<<"$resp")
          armed=$(jq -r '.autoMergeRequest != null' <<<"$resp")

          if [ "$state" = "MERGED" ] || [ "$armed" = "true" ]; then
            echo "PR #$PR is $state (already armed: $armed) — nothing to do."
            exit 0
          fi

          case "$merge_state" in
            CLEAN)
              # GitHub rejects enablePullRequestAutoMerge on an already-clean PR
              # ("Pull request is in clean status"), so merge directly.
              echo "PR #$PR is CLEAN — merging directly (rebase)."
              gh pr merge "$PR" --rebase
              ;;
            BLOCKED|UNSTABLE|BEHIND|DIRTY)
              echo "PR #$PR is $merge_state — arming auto-merge (rebase)."
              gh pr merge "$PR" --auto --rebase
              ;;
            *)
              # DRAFT / HAS_HOOKS / UNKNOWN / anything new: arm and let GitHub
              # decide. Surface the unexpected value for observability.
              echo "PR #$PR has mergeStateStatus '$merge_state' — arming auto-merge (rebase)."
              gh pr merge "$PR" --auto --rebase
              ;;
          esac
```

Rationale (design decision 6): a blind `--auto` errors spuriously on a
`CLEAN` PR; wrapping it in `|| true` would mask real failures (setting off,
missing permission). The `case` makes the mapping explicit and keeps real
failures loud.

### Verification

#### Automated
- [x] `actionlint .github/workflows/dependabot_auto_merge.yml` → clean
- [x] `./scripts/test.sh` → green
- [x] Grep the workflow for `|| true` → **no match** (`rg -n '\|\| true' .github/workflows/dependabot_auto_merge.yml`)

#### Manual
- [ ] Confirm every `case` arm from `structure.md` is present: `CLEAN` →
      direct `--rebase`; `BLOCKED`/`UNSTABLE`/`BEHIND`/`DIRTY` → `--auto
      --rebase`; plus terminal-state + already-armed guards.
- [ ] Confirm the concurrency group is exactly
      `dependabot-auto-merge-${{ github.event.pull_request.number }}` with
      `cancel-in-progress: true`.
- [ ] **Sad path (post-merge):** simulate a missing-permission run (e.g.
      dispatch/temporarily remove `pull-requests: write`) and confirm the run
      **fails**, not silently exits 0.
- [ ] **Live checkpoint (post-merge):** re-run the workflow twice on one PR
      (`gh run list --workflow dependabot_auto_merge.yml`) and confirm the
      second run cancels/supersedes the first via the concurrency group.

**Phase contract:** `mergeStateStatus` values map deterministically to one
action; concurrency-group name is `dependabot-auto-merge-<pr-number>`.

---

## Phase 4: `workflow_dispatch` manual re-arm/observe hook

An operator can re-arm and inspect any Dependabot PR on demand — the repair and
end-to-end verification tool, independent of Dependabot's daily schedule.

**File**: `.github/workflows/dependabot_auto_merge.yml` (modify)

### Changes

#### 1. Add the `workflow_dispatch` trigger
**File**: `.github/workflows/dependabot_auto_merge.yml` — the `on:` block
**Action**: modify.

```yaml
on:
  pull_request_target:
  workflow_dispatch:
    inputs:
      pr-number:
        description: 'Pull request number to re-arm/observe Dependabot auto-merge'
        required: true
        type: string
```

#### 2. Widen the job gate
**File**: `.github/workflows/dependabot_auto_merge.yml` — the `auto-merge` job
**Action**: modify.

```yaml
jobs:
  auto-merge:
    if: ${{ github.event_name == 'workflow_dispatch' || github.actor == 'dependabot[bot]' }}
```

#### 3. Resolve the PR number for both triggers
**File**: `.github/workflows/dependabot_auto_merge.yml` — the job `env:` block
**Action**: modify.

```yaml
    env:
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
      PR: ${{ github.event.pull_request.number || inputs.pr-number }}
```
Under `workflow_dispatch`, `github.event.pull_request` is absent, so
`github.event.pull_request.number` is null and the expression falls through to
`inputs.pr-number`. No `gh pr view` re-resolution is needed. The step body from
Phase 3 runs unchanged for both triggers.

#### 4. Extend the concurrency group to cover the dispatch path
**File**: `.github/workflows/dependabot_auto_merge.yml` — `concurrency:` block
**Action**: modify — **deviation from `structure.md`**, which fixed the group as
`dependabot-auto-merge-${{ github.event.pull_request.number }}`. With
`workflow_dispatch` added, that group collapses to the literal
`dependabot-auto-merge-` for every dispatch run, so a manual re-arm and an event
run on the same PR would not share a group. Extending the expression keeps the
invariant "one group per PR" for both triggers:

```yaml
concurrency:
  group: dependabot-auto-merge-${{ github.event.pull_request.number || inputs.pr-number }}
  cancel-in-progress: true
```

### Verification

#### Automated
- [x] `actionlint .github/workflows/dependabot_auto_merge.yml` → clean
- [x] `python3 -c 'import yaml,sys;yaml.safe_load(open(".github/workflows/dependabot_auto_merge.yml"))'` → no error
- [x] `./scripts/test.sh` → green

#### Manual
- [ ] Confirm the dispatch input name is exactly `pr-number` (`required: true`,
      `type: string`) and the resolve expression is
      `github.event.pull_request.number || inputs.pr-number`.
- [ ] Confirm the widened `if:` still blocks non-Dependabot PRs on the event
      path (only `workflow_dispatch` bypasses the actor gate).
- [ ] **Live checkpoint (post-merge):**
      `gh workflow run dependabot_auto_merge.yml -f pr-number=<n>` then
      `gh run watch`; assert `gh pr view <n> --json autoMergeRequest,state`
      shows the request set (or state unchanged for an already-merged PR).
- [ ] **Sad path (post-merge):** dispatch with a non-Dependabot PR number →
      the run fails to arm (GitHub rejects), surfaced loudly (non-zero exit).

**Phase contract:** dispatch input name `pr-number`; the Phase-3
`mergeStateStatus` logic runs unchanged for both triggers.

---

## Phase 5: Hardening and documentation

The change is discoverable and its prerequisite is documented, so it does not
silently regress.

**Files**: `AGENTS.md`, `.github/workflows/dependabot_auto_merge.yml`

> **Deviation from `structure.md`:** `structure.md` lists
> `scripts/branch-protection.sh` in Phase 5's files ("header comment updated to
> list `allow_auto_merge`"). That header line is already written in Phase 1
> (§1), so Phase 5 does **not** re-edit `branch-protection.sh` — re-touching it
> here would be a no-op duplicate. Phase 5 covers `AGENTS.md` and the workflow
> header only.

### Changes

#### 1. `AGENTS.md` — "Commits and PRs" note
**File**: `AGENTS.md` (repo root)
**Action**: modify — add a bullet after the existing "Merge PRs only with
`--rebase` …" bullet and before the branch-protection bullet.

```markdown
- Dependabot PRs merge via GitHub native auto-merge: the
  `.github/workflows/dependabot_auto_merge.yml` workflow (`pull_request_target`)
  arms auto-merge for `dependabot[bot]` PRs, and `main` merges them once the
  required checks pass. This needs the repo "Allow auto-merge" setting, enforced
  by `scripts/branch-protection.sh verify` (`allow_auto_merge=true`).
```

#### 2. Workflow header — mechanism, security posture, `BEHIND`-stall follow-up
**File**: `.github/workflows/dependabot_auto_merge.yml` — header comment block
**Action**: modify — append the deferred-escalation note (the mechanism and
security posture lines were already written in Phase 2). Add after the
`SECURITY:` lines:

```yaml
#
# Known limitation: with strict status checks, an armed PR that falls BEHIND
# waits for Dependabot's next scheduled rebase. If stalls are observed, the
# escalation is a deferred `push: main` workflow running `gh pr update-branch`
# (or a merge queue). Not enabled here.
```

#### 3. `branch-protection.sh` header
**File**: `scripts/branch-protection.sh`
**Action**: none — already updated in Phase 1. Verify only.

### Verification

#### Automated
- [x] `./scripts/branch-protection.sh verify` → green and reports `allow_auto_merge`
- [x] `./scripts/test.sh` → green (docs-only change to non-Rust files)
- [x] `actionlint .github/workflows/dependabot_auto_merge.yml` → clean

#### Manual
- [ ] Proofread `.pi/orksorksorks/alanvardy-var-1115-auto-merge-dependabot/*.md`
      and `AGENTS.md` for accuracy against the shipped workflow.
- [ ] Confirm the workflow header states: native mechanism (`gh`/API only), the
      `pull_request_target` + no-checkout security posture, and the `BEHIND`
      follow-up.
- [ ] Confirm `AGENTS.md` describes the shipped behaviour and names the
      prerequisite + the enforcing command.

**Phase contract:** docs describe the shipped behaviour; no functional change.

---

## Testing Checkpoints

- **After Phase 1:** `bash -n` + `shellcheck` clean;
  `./scripts/branch-protection.sh verify` green and reports `allow_auto_merge`
  before touching the workflow.
- **After Phase 2:** `actionlint` + PyYAML valid; the `--auto` path is
  understood and the live `gh pr view … autoMergeRequest` assertion is defined.
- **After Phase 3:** state branches cover
  CLEAN/BLOCKED/UNSTABLE/BEHIND/DIRTY + terminal + armed; concurrency group
  present; no blanket `|| true`.
- **After Phase 4:** dispatch path shares the exact Phase-3 logic; `pr-number`
  input + widened `if:` present; concurrency group covers both triggers.
- **After Phase 5:** docs mention the prerequisite; `verify` +
  `./scripts/test.sh` green. Final real E2E remains the next daily Dependabot PR
  after merge to `main`.

## Files touched (complete list)

| File | Phase(s) | Action |
|------|----------|--------|
| `scripts/branch-protection.sh` | 1 | modify |
| `.github/workflows/dependabot_auto_merge.yml` | 2, 3, 4, 5 | rewrite in place, then modify |
| `AGENTS.md` | 5 | modify |

Nothing else. No Rust, routes, `ROUTES.md`, `dependabot.yml`, migrations, or
`static/site.css`.

## Out of scope (from `design.md`)

- Keeping/upgrading `fastify/github-action-merge-dependabot` — removed entirely.
- Excluding GitHub-Actions majors from auto-merge — all updates merge.
- GitHub merge queue on `main`.
- A `push: main` / scheduled `gh pr update-branch` workflow (deferred escalation).
- Any approval step / enabling `can_approve_pull_request_reviews`.
- Changing merge method, branch protection rules, `REQUIRED_CONTEXTS`, CI job
  names, `dependabot.yml` groups/schedules, or `delete_branch_on_merge`.
- Rust test-suite additions (this is CI/CD config).
- Child Linear tickets.