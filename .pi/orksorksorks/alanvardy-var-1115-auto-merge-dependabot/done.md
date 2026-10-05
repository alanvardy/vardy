# Done

- **Branch / head SHA**: `alanvardy-var-1115-auto-merge-dependabot`
  - `628b356` — chore: start (scaffold `DELETEME`)
  - `3d67b9c`…`47d9505` — Phases 1–5 (branch-protection, workflow, AGENTS.md)
  - `aa532c5` — chore: remove scaffold placeholder (pre-merge cleanup; also adds
    this branch's `.pi/orksorksorks/...` artifacts)
  - `f740b06` — fix: clarify CLEAN direct-merge in workflow header, add trailing
    newline (review follow-up)
  - A final commit adds this `done.md` marker.
- **Mechanical checks** (all green):
  - `bash -n scripts/branch-protection.sh` — clean
  - `shellcheck scripts/branch-protection.sh` — clean, 0 warnings
  - `actionlint .github/workflows/dependabot_auto_merge.yml` — clean (re-run after
    the review follow-up edit)
  - YAML parse (`python3 -c 'import yaml; ...'`) — valid
  - `./scripts/branch-protection.sh verify` — exit 0
    (`OK: repo merge settings match expected config (rebase-only, auto-merge enabled)`)
  - `./scripts/test.sh` — green, 127/127 tests, exit 0 (the change touches only
    `.github/**` and docs, so the gate is unaffected)
- **Review outcome**: one bounded fresh-context reviewer; **no blockers**, merge
  verdict *OK with notes*. Four P2 report-only notes plus one nit.
  - **Applied now**: trailing newline on `dependabot_auto_merge.yml` (nit), and
    the header now states the already-CLEAN PR is merged directly because GitHub
    rejects enabling auto-merge in that state (doc-accuracy P2).
  - **Deferred (not applied, optional)**: (a) `workflow_dispatch` un-gates the
    `dependabot[bot]` actor check — reviewer confirmed it is *not* a privilege
    escalation (any `write` user can already `gh pr merge` a mergeable PR) and it
    is the intended manual re-arm hook; a comment or actor guard is optional.
    (b) `cancel-in-progress: true` could cancel an in-flight direct merge —
    low risk; noted, not changed.
  - **Rejected as a false positive**: reviewer suggested dropping
    `contents: write` as likely unnecessary. GitHub's GraphQL
    `enablePullRequestAutoMerge` requires **both** `contents: write` and
    `pull-requests: write`; the existing comment is correct, so the permission
    and comment stay
    ([community discussion #24686](https://github.com/orgs/community/discussions/24686)).
- **Remaining manual items** (from `implement.md`; live checkpoints are only
  observable after merge to `main`, since `pull_request_target` runs from the
  default branch):
  - Post-merge: on a real Dependabot PR, `gh pr view <n> --json autoMergeRequest`
    shows the request set (or the PR merges with no human command).
  - Phase 1 sad path: temporarily flip `allow_auto_merge=false`, confirm
    `./scripts/branch-protection.sh verify` exits non-zero, then restore.
  - Phase 2/3 sad path: confirm a missing-permission run fails loudly rather than
    exiting 0.
  - Phase 4 live checkpoint: `gh workflow run dependabot_auto_merge.yml -f pr-number=<n>`
    arms/observes; dispatch with a non-Dependabot PR number fails loudly.
  - Proofread `.pi/orksorksorks/alanvardy-var-1115-auto-merge-dependabot/*.md`
    against the shipped workflow before merge.
