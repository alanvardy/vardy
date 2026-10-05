# Design Discussion

## Current State

Dependabot auto-merge is intended to run via `.github/workflows/dependabot_auto_merge.yml`:
`pull_request_target`, gated `if: github.actor == 'dependabot[bot]'`, calling
`fastify/github-action-merge-dependabot@v3` with `target: minor`,
`merge-method: rebase`, `use-github-auto-merge: true` (research Q1). The file has
explicit scoped `permissions:` (`pull-requests: write`, `contents: write`) and a
security comment forbidding PR-head checkout (dependabot_auto_merge.yml:7-15).

Branch protection / repo merge settings (`scripts/branch-protection.sh`) enforce:
strict status checks against `REQUIRED_CONTEXTS`, linear history, 0 approving
reviews, rebase-only merges, no force-push/deletion (research Q2,
branch-protection.sh:17-24, :80-148). `main` is thus mergeable when: branch is
current with `main`, all 5 CI contexts green, rebase method, no approval needed.

**Design-phase live verification (this phase, via `gh api`):**

```
repos/alanvardy/vardy                      → allow_auto_merge: false
repos/alanvardy/vardy/actions/permissions/workflow
                                           → default_workflow_permissions: read
                                             can_approve_pull_request_reviews: false
```

**Design-phase source verification (vendored `fastify/...@v3.1.0`):** the current
workflow cannot merge anything today; there are three independent blockers:

1. `allow_auto_merge: false` → `enablePullRequestAutoMerge` is rejected outright.
2. `can_approve_pull_request_reviews: false` → the action calls
   `client.approvePullRequest()` **unconditionally** before arming auto-merge
   (`dist/index.js:35039-35040`); it throws, `setFailed`s, and never arms.
3. `target: minor` + `pull_request_target` → the action's bundled
   `fetch-metadata` step is gated `if: github.event_name == 'pull_request'`
   (`action.yml`), so `update-type` is empty; the `target !== any` guard then
   rejects as `skipped:invalid_semver` (`dist/index.js:34981-34985`).

## Desired End State

Dependabot's daily `cargo-all`/`gha-all` PRs auto-merge after the required CI
contexts pass and the branch is current, with no human command and no
`@dependabot merge`. End state:

- A `pull_request_target` workflow (same filename) that arms GitHub native
  auto-merge for any PR whose actor is `dependabot[bot]`, using `gh` only —
  no third-party action.
- Repository-level **Allow auto-merge** is `true`, verified/enforced by
  `scripts/branch-protection.sh` alongside the existing merge settings.
- A manual `workflow_dispatch` hook (`pr-number`) to re-arm/observe a PR.
- Correct behaviour on already-`CLEAN`, already-armed, and already-merged PRs
  (no spurious red runs, no silent no-ops).

**Verification:** after this change merges to `main`, observe the next daily
Dependabot PR: check run reports, then `gh pr view <n> --json
state,autoMergeRequest,mergeStateStatus` shows `autoMergeRequest` set and,
once checks pass, `state: MERGED`. A manual `workflow_dispatch` on the same PR
re-produces the outcome. `./scripts/branch-protection.sh verify` passes with
`allow_auto_merge=true`.

## Patterns to Follow

- **Scope top-level `permissions:` explicitly** rather than `write-all` or
  relying on defaults — copy dependabot_auto_merge.yml:13-15
  (`contents: write`, `pull-requests: write`).
- **Keep the security comment and never check out the PR head** under
  `pull_request_target` — dependabot_auto_merge.yml:7-9; the action-less `gh`
  form makes this structural (no `actions/checkout` at all).
- **Add a `concurrency` block** per workflow, as ci.yml:27-33,
  rust-version-bump.yml:12-18, ci-secure.yml:6-15 and fly-deploy.yml:9-16 each
  do.
- **Treat `scripts/branch-protection.sh` as the single source of truth for
  repo/branch settings** — `verify` (read-only) vs `apply` (`--yes`)
  (branch-protection.sh:207-236); `REQUIRED_CONTEXTS` (:17-24) is the canonical
  job-name list and any CI `name:` change must update it.
- **Shell steps under `pull_request_target` operate on metadata/API only**,
  never on untrusted PR content.

**Anti-patterns found (do NOT follow):**

- The `fastify/github-action-merge-dependabot@v3` dependency: it is not
  SHA-pinned (`@v3` float), it mandates an approval the repo does not need, and
  its semver gate is broken on `pull_request_target` (above). Replace it.
- Do not encode merge prerequisites only in a workflow comment — the
  `allow_auto_merge` toggle is invisible in-repo and was the primary failure.

## Design Decisions

1. **Mechanism — replace fastify with GitHub's prescribed native pattern**:
   a `pull_request_target` job with `if: github.actor == 'dependabot[bot]'` and
   `gh pr merge --auto --rebase` (research Q4/Q5). Removes a non-pinned
   third-party action, eliminates the mandatory-approval requirement (blocker 2)
   and the `update-type` semver gate (blocker 3) by construction, and drops the
   need for `can_approve_pull_request_reviews`.
2. **Update scope — everything Dependabot opens**: cargo and GitHub Actions, all
   major/minor/patch (research Q3, dependabot.yml:14-25). Matches the user's
   "majors should auto-merge too" checkpoint; the `gh` mechanism applies no
   semver filter, so this is the natural semantics.
3. **Strict-check staleness — accept, with a documented follow-up**: an armed PR
   that goes `BEHIND` waits for Dependabot's next scheduled rebase (official
   `rebase-strategy` docs: rebase triggers are conflict, schedule, reopen, or
   `target-branch` change — not merely `BEHIND`). At current Dependabot volume
   this is acceptable; a future `push: main` → `gh pr update-branch` workflow
   (or a merge queue) is the escalation if stalls are observed.
4. **Manual re-arm hook — `workflow_dispatch` with a required `pr-number`
   input**, resolved via `gh pr view`, reusing the same state-branch logic as
   the event path. Serves as the end-to-end verification/repair tool.
5. **Repo setting — extend `scripts/branch-protection.sh`** to verify and apply
   `allow_auto_merge=true` in `verify_repo_settings`/`apply` (research Q2,
   branch-protection.sh:156-176), leaving `can_approve_pull_request_reviews`
   untouched. The script is the existing settings chokepoint; drift is then
   caught by `verify` rather than discovered as a stuck PR.
6. **Merge-state handling — branch on `mergeStateStatus`** instead of a blind
   `--auto`: query `gh pr view --json state,mergeStateStatus,autoMergeRequest`;
   `MERGED` or `autoMergeRequest != null` → log and exit 0; `CLEAN` → direct
   `gh pr merge --rebase`; otherwise (`BLOCKED`/`UNSTABLE`/`BEHIND`/`DIRTY`) →
   `gh pr merge --auto --rebase`. GitHub rejects `enablePullRequestAutoMerge`
   on an already-clean PR ("Pull request is in clean status"), so the blind
   form both errors spuriously and, once wrapped in `|| true`, masks real
   failures (setting off, permissions).
7. **Concurrency and event types — keep default `pull_request_target` types
   (`opened`, `synchronize`, `reopened`) and add**
   `concurrency: {group: dependabot-auto-merge-${{ github.event.pull_request.number }}, cancel-in-progress: true}`.
   Keeping `synchronize` re-arms/resolves after a Dependabot rebase; the
   concurrency group prevents parallel duplicate runs on the same PR.
8. **Security posture — no `actions/checkout`, no PR-head code execution, no
   secrets**: the job runs only `gh` against the API from the base-branch
   context (preserving the existing intent at dependabot_auto_merge.yml:7-9).
9. **Filename — rewrite `dependabot_auto_merge.yml` in place** (same path/name)
   so history and any external references are preserved; no new workflow file.
10. **Documentation — short header comment in the rewritten workflow, a header
    line in `branch-protection.sh`, and a repo `AGENTS.md` note** under
    "Commits and PRs" describing how Dependabot PRs now merge and the
    `allow_auto_merge` prerequisite.

## What We're NOT Doing

- Keeping or upgrading `fastify/github-action-merge-dependabot` (any `target`
  value) — replaced entirely.
- Excluding `github-actions` major updates from auto-merge — all updates merge.
- Enabling GitHub **merge queue** on `main` — strict-check staleness handled by
  Dependabot's own rebase for now.
- Adding a `push: main` / scheduled `gh pr update-branch` workflow — deferred
  escalation only.
- Adding any approval step or enabling `can_approve_pull_request_reviews`
  (branch protection requires 0 approvals).
- Changing merge method, branch protection rules, `REQUIRED_CONTEXTS`, CI job
  names, or `dependabot.yml` update groups/schedules.
- Changing `delete_branch_on_merge` (currently `false`) or other unrelated repo
  settings.
- Creating child Linear tickets — all work lands on the main ticket.
- Adding tests to the Rust test suite — this is CI/CD config, verified by
  `branch-protection.sh verify` and a live Dependabot PR, not `cargo nextest`.

## Open Risks

- **`pull_request_target` requires the workflow to exist on the default
  branch**: the fix only takes effect after this PR merges to `main`; it cannot
  be fully exercised from the feature branch.
- **End-to-end proof depends on a real Dependabot PR**: Dependabot's daily run
  is the only natural trigger; the `workflow_dispatch` hook mitigates but still
  needs a Dependabot-authored PR to pass the actor gate (or a manual test that
  bypasses it locally).
- **`BEHIND` stalls**: if Dependabot's scheduled rebase lags (or the PR ages
  past the 30-day auto-rebase cutoff), auto-merge waits indefinitely; the
  deferred `push: main` update workflow is the documented next step.
- **GitHub-Actions major bumps merge unattended**: a breaking `actions/*` major
  could land and only surface on the next workflow run; accepted (decision 2)
  but worth watching.
- **`gh pr merge --auto` edge behaviours** may differ slightly across runner
  `gh` versions (clean-status race, merge-queue interactions); the
  `mergeStateStatus` branch is designed to absorb these, but the exact
  `gh` output/exit semantics should be confirmed on the first live run.
- **`branch-protection.sh apply` requires admin `gh` auth + `jq`**
  (research Q2, branch-protection.sh:207-236); enabling `allow_auto_merge`
  may need to be done out-of-band first if the automation lacks admin scope.
