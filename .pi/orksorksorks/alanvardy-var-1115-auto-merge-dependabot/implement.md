# Implementation Summary

All 5 phases of the Dependabot native auto-merge change implemented, verified,
and committed — one commit per phase, each scoped to exactly its phase's files.

## Commits

| Phase | Commit | Description |
|-------|--------|-------------|
| 1     | `3d67b9c` | enforce allow_auto_merge in branch-protection |
| 2     | `0503f3b` | Dependabot PR arms native auto-merge via gh |
| 3     | `f5d2601` | merge-state discrimination + concurrency |
| 4     | `6de75f4` | workflow_dispatch manual re-arm/observe hook |
| 5     | `47d9505` | hardening and documentation |

Branch: `alanvardy-var-1115-auto-merge-dependabot` (pushed to origin, fast-forward).

Files touched across all phases (the complete plan list):
- `scripts/branch-protection.sh` (Phase 1 only)
- `.github/workflows/dependabot_auto_merge.yml` (Phases 2–5)
- `AGENTS.md` (Phase 5 only)

Nothing else. The existing unrelated working-tree `DELETEME` deletion (ticket
scaffold placeholder) was left untouched and uncommitted.

## Automated Checks

- [x] `bash -n scripts/branch-protection.sh` — clean (Phase 1)
- [x] `shellcheck scripts/branch-protection.sh` — clean, 0 warnings (Phase 1)
- [x] `./scripts/branch-protection.sh verify` — exit 0, reports `allow_auto_merge`
      (`OK: repo merge settings match expected config (rebase-only, auto-merge enabled)`);
      run again green in Phase 5 (Phases 1 & 5)
- [x] `./scripts/test.sh` — green (127/127 tests, exit 0) in every phase that listed it
      (Phase 1, 2, 3, 4, 5)
- [x] `gh api repos/alanvardy/vardy --jq '.allow_auto_merge'` → `true` (Phase 1; the plan's
      intended end-state, achieved via the authorized `./scripts/branch-protection.sh apply --yes`)
- [x] `actionlint .github/workflows/dependabot_auto_merge.yml` — clean (Phases 2, 3, 4, 5)
- [x] `python3 -c 'import yaml,...'` on the workflow — YAML valid (Phases 2 & 4)
- [x] `rg -n '\|\| true' .github/workflows/dependabot_auto_merge.yml` — no match
      (Phases 3 & 4; no blanket error-masking)
- [x] `gh workflow view dependabot_auto_merge.yml` — available, job path resolves (Phase 2)

## Manual Verification Items (from the plan)

Gathered for you — none auto-completed:

- [ ] Phase 1 — Apply the setting if it is still off (`./scripts/branch-protection.sh apply --yes`).
      *Note:* the authorized `apply --yes` was run during implementation (Option A), so the
      setting is now `true` and `verify` reports it; left as a manual item for reconfirmation.
- [ ] Phase 1 — Sad path: temporarily flip `allow_auto_merge` off, confirm `verify` exits
      non-zero with `FAIL: allow_auto_merge = false`, then restore.
- [ ] Phase 2 — Diff review: the `fastify/github-action-merge-dependabot@v3` dependency, its
      `target: 'minor'` semver gate, and its mandatory-approval path are gone.
- [ ] Phase 2 — Confirm file name unchanged (`dependabot_auto_merge.yml`) and job name unchanged (`auto-merge`).
- [ ] Phase 2 — Reason through the sad path: non-Dependabot PRs never reach the job (`if:` gate);
      a non-owner/repo token fails loudly, not a masked `|| true`.
- [ ] Phase 2 — Live checkpoint (post-merge): on a real Dependabot PR, `gh pr view <n> --json autoMergeRequest`
      shows the request set.
- [ ] Phase 3 — Confirm every `case` arm: `CLEAN` → direct `--rebase`; `BLOCKED/UNSTABLE/BEHIND/DIRTY`
      → `--auto --rebase`; terminal-state + already-armed guards.
- [ ] Phase 3 — Confirm concurrency group exactly `dependabot-auto-merge-${{ github.event.pull_request.number }}`
      with `cancel-in-progress: true`.
- [ ] Phase 3 — Sad path (post-merge): simulate a missing-permission run and confirm it FAILS,
      not silently exit 0.
- [ ] Phase 3 — Live checkpoint (post-merge): re-run the workflow twice on one PR; second run
      cancels/supersedes the first via the concurrency group.
- [ ] Phase 4 — Confirm dispatch input name exactly `pr-number` (`required: true`, `type: string`)
      and resolve expression `github.event.pull_request.number || inputs.pr-number`.
- [ ] Phase 4 — Confirm the widened `if:` still blocks non-Dependabot PRs on the event path.
- [ ] Phase 4 — Live checkpoint (post-merge): `gh workflow run dependabot_auto_merge.yml -f pr-number=<n>`;
      assert `autoMergeRequest,state` shows the request set (or state unchanged for merged).
- [ ] Phase 4 — Sad path (post-merge): dispatch with a non-Dependabot PR number → run fails loudly.
- [ ] Phase 5 — Proofread `.pi/orksorksorks/alanvardy-var-1115-auto-merge-dependabot/*.md` and `AGENTS.md`
      for accuracy against the shipped workflow.
- [ ] Phase 5 — Confirm the workflow header states: native mechanism (`gh`/API only), the
      `pull_request_target` + no-checkout security posture, and the `BEHIND` follow-up.
- [ ] Phase 5 — Confirm `AGENTS.md` describes the shipped behaviour and names the prerequisite +
      the enforcing command.

## Notes / Observations

- **Plan/state divergence resolved (Phase 1):** the plan Overview documents `allow_auto_merge`
  as currently `false`, while the Phase 1 automated checks (`verify` exit 0, `gh api … → true`)
  require it to be `true`. The plan's own manual step (`apply --yes`) is what flips it. With the
  owner's authorization (Option A), `./scripts/branch-protection.sh apply --yes` was run, which
  re-applied branch protection + repo merge settings and set `allow_auto_merge=true`, making the
  automated checks pass as designed. This live mutation was NOT run silently.
- The `apply --yes` operation also re-PUTs branch protection (broader than just the toggle), as
  the script's `apply` path always does — the run re-verified green.
- No `actions/checkout` anywhere in the workflow; security posture is structural (API-only under
  `pull_request_target`).
- The real end-to-end proof (a Dependabot PR merging) is only observable after this PR lands on
  `main` — the post-merge live checkpoints above cover that.
- `.sqlx/` metadata and `static/site.css` were regenerated by `./scripts/test.sh` during phases but
  produced no tracked diff drift.

**Next step:** review this branch (`!1`), then merge to `main` with `--rebase`
(`gh pr merge <n> --rebase --delete-branch`) once approved.