# Structure Outline

## Approach

Replace the broken `fastify/...@v3` dependency in `dependabot_auto_merge.yml` with
GitHub's prescribed native pattern — a `pull_request_target` job gated on
`github.actor == 'dependabot[bot]'` running `gh pr merge --auto --rebase`, with a
`mergeStateStatus` branch so already-clean/armed/merged PRs are handled — and make
the `allow_auto_merge` repo setting an enforced, repo-tracked prerequisite in
`scripts/branch-protection.sh`. All updates (cargo + actions, major/minor/patch)
merge. No Rust code or tests change.

**Vertical-slice caveat:** this task is CI/CD config, so its "stack" is
workflow YAML → `gh`/API → repo setting. The repo-level `allow_auto_merge` toggle
is genuinely horizontal (like a migration): no workflow slice can land green
end-to-end without it, so it is Phase 1, shippable and verified on its own.
There is **no automated test harness for shell/YAML** in this repo; each slice's
"tests" are `branch-protection.sh verify` (read-only) plus live `gh` assertions
against a Dependabot PR. `./scripts/test.sh` must stay green (it does not touch
`.github/`).

**E2E proof limit (applies to every slice):** `pull_request_target` only runs from
the default branch, so no slice's live merge can be observed from this worktree.
Each slice's live checkpoint is a dry observation via `gh api` / a
`workflow_dispatch` dry run, with the real end-to-end merge confirmed on the next
daily Dependabot PR **after** merge to `main`.

---

## Phase 1: Repo `allow_auto_merge=true` enforced in branch-protection.sh (horizontal prerequisite)

`scripts/branch-protection.sh verify` now asserts and `apply` now sets the
repository "Allow auto-merge" toggle, so the setting can never silently drift
back off (the primary current failure).

**Files**: `scripts/branch-protection.sh`
**Key changes**:
- `verify_repo_settings()` — additionally read `allow_auto_merge` via
  `gh api "repos/$OWNER/$REPO_NAME" --jq '.allow_auto_merge'`; `FAIL` unless `true`
  (mirror the existing rebase-merge checks at :156-176)
- `apply_repo_settings()` — add `"allow_auto_merge": true` to the `gh api -X PATCH`
  body (leave `can_approve_pull_request_reviews` untouched)
- header usage comment — mention the toggle among enforced settings
**Contract**: `allow_auto_merge` is `true` on `repos/alanvardy/vardy` and checked
by `verify`; every later slice may assume it holds.

**Tests**: `./scripts/branch-protection.sh verify` passes and its output reports
`allow_auto_merge`; `apply --yes` re-verifies cleanly. Sad path: run `verify`
against a repo with the toggle off (or temporarily flip it) → exits non-zero.
**Verify**: `./scripts/branch-protection.sh apply --yes` then `verify` green;
`gh api repos/alanvardy/vardy --jq .allow_auto_merge` → `true`.
(Requires admin `gh` auth + `jq`; if scope is missing, enable out-of-band and keep
`verify` asserting it.)

---

## Phase 2: Walking skeleton — Dependabot PR arms native auto-merge via `gh`

The thinnest real end-to-end path: a Dependabot-authored PR event fires the base-
branch workflow, `gh` queries the PR, skips if already terminal, and arms native
auto-merge with rebase. A future Dependabot PR merges with no human command.
Front-loads the riskiest integration (`gh` API under `pull_request_target`).

**Files**: `.github/workflows/dependabot_auto_merge.yml` (rewritten in place)
**Key changes**:
- `on: pull_request_target:` (keep default types) — no `actions/checkout`
- job `if: ${{ github.actor == 'dependabot[bot]' }}`; `permissions: {contents: write, pull-requests: write}`
- single `run` step (env `GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}`, `PR: ${{ github.event.pull_request.number }}`):
  - `gh pr view "$PR" --json state,mergeStateStatus,autoMergeRequest`
  - `state == MERGED` or `autoMergeRequest != null` → log + `exit 0`
  - else `gh pr merge "$PR" --auto --rebase`
- keep/refresh the security header comment (`pull_request_target`, never checkout PR head)
**Contract**: workflow filename + `auto-merge` job name unchanged; PR number is
sourced from `github.event.pull_request.number`; terminal-state guard semantics
(`MERGED`/`autoMergeRequest` → no-op exit 0) are the interface Phase 3 and 4 reuse.

**Tests**: no automated harness — live checkpoint only. Sad path: a non-Dependabot
PR is skipped by the `if:`. Happy path: `gh pr view <n> --json autoMergeRequest`
on a real Dependabot PR shows the request set (post-merge observation).
**Verify**: `./scripts/test.sh` green (unaffected by `.github/` changes);
validate YAML with `gh workflow view dependabot_auto_merge.yml` (post-merge) or
`python -c 'import yaml,sys;yaml.safe_load(open(".github/workflows/dependabot_auto_merge.yml"))'`.

---

## Phase 3: Merge-state discrimination + concurrency (no spurious red, no duplicate runs)

Every reachable `mergeStateStatus` behaves correctly and concurrent events on one
PR collapse to one run.

**Files**: `.github/workflows/dependabot_auto_merge.yml`
**Key changes**:
- branch on `mergeStateStatus`: `CLEAN` → `gh pr merge "$PR" --rebase` (direct);
  `BLOCKED`/`UNSTABLE`/`BEHIND`/`DIRTY` → `gh pr merge "$PR" --auto --rebase`
  (never wrap in blanket `|| true` — real permission/setting failures must fail loud)
- add `concurrency: {group: dependabot-auto-merge-${{ github.event.pull_request.number }}, cancel-in-progress: true}`
**Contract**: `mergeStateStatus` values map deterministically to one action;
concurrency-group name is `dependabot-auto-merge-<pr-number>`.

**Tests**: live checkpoint on PRs in each state (CLEAN arm, BEHIND, already-armed).
Sad path: simulate a missing-permission run and confirm it **fails** (not masked).
**Verify**: `./scripts/test.sh` green; re-run the workflow twice on one PR and
confirm the second run cancels/supersedes the first (concurrency) via
`gh run list --workflow dependabot_auto_merge.yml`.

---

## Phase 4: `workflow_dispatch` manual re-arm/observe hook

An operator can re-arm and inspect any Dependabot PR on demand — the repair and
end-to-end verification tool, independent of Dependabot's daily schedule.

**Files**: `.github/workflows/dependabot_auto_merge.yml`
**Key changes**:
- `on.workflow_dispatch.inputs."pr-number"` (`required: true`, string)
- job gate widened: `if: github.event_name == 'workflow_dispatch' || github.actor == 'dependabot[bot]'`
- resolve PR: `PR: ${{ github.event.pull_request.number || inputs.pr-number }}` (no `gh pr view` re-resolution needed)
**Contract**: dispatch input name `pr-number`; the same `mergeStateStatus`
logic from Phase 3 runs unchanged for both triggers.

**Tests**: no automated harness — dispatch against a real Dependabot PR and assert
`autoMergeRequest` set / state unchanged for already-merged. Sad path: a
non-Dependabot PR number → the run fails to arm (GitHub rejects), surfaced loudly.
**Verify**: `gh workflow run dependabot_auto_merge.yml -f pr-number=<n>` then
`gh run watch`; `gh pr view <n> --json autoMergeRequest,state`.

---

## Phase 5: Hardening and documentation

The change is discoverable and its prerequisite is documented, so it does not
silently regress.

**Files**: `AGENTS.md`, `scripts/branch-protection.sh`, `.github/workflows/dependabot_auto_merge.yml`
**Key changes**:
- `AGENTS.md` → "Commits and PRs": note that Dependabot PRs merge via native
  auto-merge once required contexts pass, and that `allow_auto_merge=true` is a
  prerequisite enforced by `scripts/branch-protection.sh verify`
- `branch-protection.sh` header comment updated to list `allow_auto_merge`
- `.github/workflows/dependabot_auto_merge.yml` header comment states the mechanism, the strictly-API/security posture,
  and the `BEHIND`-stall follow-up (deferred `push: main` update-branch workflow)
**Contract**: docs describe the shipped behaviour; no functional change.

**Tests**: none beyond `verify`; this slice is review-only.
**Verify**: `./scripts/branch-protection.sh verify` green; `./scripts/test.sh`
green (docs-only change to non-Rust files); proofread artifacts.

---

## Testing Checkpoints

- **After Phase 1**: `branch-protection.sh verify` green and reports
  `allow_auto_merge=true` before touching the workflow.
- **After Phase 2**: YAML valid; the naive `--auto` path is understood and the
  live `gh pr view ... autoMergeRequest` assertion is defined.
- **After Phase 3**: state branches cover CLEAN/BLOCKED/UNSTABLE/BEHIND/DIRTY +
  terminal; concurrency group present; no blanket `|| true`.
- **After Phase 4**: dispatch path shares the exact Phase-3 logic; `pr-number`
  input + widened `if:` present.
- **After Phase 5**: docs mention the prerequisite; `verify` + `./scripts/test.sh`
  green. Final real E2E remains the next daily Dependabot PR after merge to `main`.
