# Design Discussion

## Current State

- **No branch protection on `main`**: `gh api repos/alanvardy/vardy/branches/main/protection` returns 404 (confirmed live).
- **All merge methods enabled at repo level**: `allow_merge_commit`, `allow_squash_merge`, `allow_rebase_merge` all `true` (confirmed live).
- **CI runs on every PR**: 5 CI jobs surface as checks on every PR to `main` (`ci.yml:38-39, 88-89, 99-100, 109-110, 128-129`), plus `Dependabot Auto Merge / auto-merge` (`dependabot_auto_merge.yml:5-15`). All 6 run on every `pull_request` with no branch filter.
- **Dependabot auto-merge** uses GitHub native auto-merge (`use-github-auto-merge: true`, `dependabot_auto_merge.yml:22`) with `merge-method: rebase` (`:21`). The header comment says it merges "once all checks and branch protection rules pass" (`:1-3`).
- **Fly deploy** keys off whole-CI-workflow conclusion on `main` (`fly-deploy.yml:4-13`), not individual checks or protection.
- **Rebase-only convention** is documented in the home conventions file (`~/.pi/agent/AGENTS.md:86-88`) and implied by `merge-method: rebase` in `dependabot_auto_merge.yml:21`, but no committed repo doc states it.
- **No `gh` CLI or GitHub REST API** usage exists anywhere in scripts or workflows (research §Q4). No precedent for repository-settings administration.
- **Scripting pattern**: bash scripts use `set -euo pipefail` (`scripts/build-css.sh:4`, `scripts/reset_db.sh:4`); `scripts/test.sh` uses `&&`-chaining without `set -e` (`scripts/test.sh:1`).

## Desired End State

1. `main` is protected via GitHub REST API with:
   - **Required status checks** (`strict: true`): all 6 contexts —
     `CI / Cargo CI Tests`, `CI / TODO and FIXME`, `CI / Rust-fmt (Cargo Format)`,
     `CI / Clippy (Cargo Clippy Lint Check)`, `CI / CSS Drift Check`,
     `Dependabot Auto Merge / auto-merge`
   - **Required linear history** (`required_linear_history: true`) — no merge commits, no squash merges into `main`
   - **Pull request reviews** (`required_pull_request_reviews` with `dismiss_stale_reviews: true`, `required_approving_review_count: 0`) — review dismissal works if reviews are given, but no approval count blocks the solo maintainer
   - **`enforce_admins: false`** — admin can bypass checks in emergencies
2. Repo-level merge method settings: `allow_merge_commit: false`, `allow_squash_merge: false`, `allow_rebase_merge: true` — matches the rebase-only convention, disables the merge-commit and squash buttons in the UI
3. Existing automation keeps working:
   - Dependabot auto-merge: `Dependabot Auto Merge / auto-merge` is in the required-check list, and native auto-merge waits on protection + checks
   - Fly deploy: `workflow_run` on "CI" completed for `main` continues to fire; the CI workflow conclusion gate is unchanged
4. Change is reproducible via `scripts/branch-protection.sh` — an idempotent script using `gh api` that applies and verifies the protection config
5. Verification: a `GET` on the protection endpoint returns the expected config; `gh pr merge` with rebase still works after the change

## Patterns to Follow

- **Shell style**: `set -euo pipefail` (`scripts/build-css.sh:4`, `scripts/reset_db.sh:4`). The new script follows this pattern.
- **`gh api` with `--silent`**: authenticated via the existing `gh` token (no new secrets needed; `gh auth status` confirms `repo` scope).
- **Idempotent**: PUT the protection config; re-running the script produces the same result (no double-apply side effects).
- **Verify after apply**: `GET` the protection endpoint and assert key fields match expectations, exit non-zero on mismatch.
- **No workflow integration**: the script is run manually by the maintainer, not triggered by CI. No `GITHUB_TOKEN` or `secrets` reference needed.
- **Commit the script** alongside a brief note in `AGENTS.md` "Commits and PRs" section documenting the rebase-only convention that was previously only in the home conventions file.

## Design Decisions

1. **All 6 check contexts required (including Dependabot Auto Merge)**: chosen over requiring only the 5 CI checks. The Dependabot Auto Merge job no-ops on non-dependabot PRs (it always passes), so it never blocks a human PR. Including it in the required list ensures it's present as a signal and doesn't surprise dependabot auto-merge (which already expects it). Chosen over a subset because the CI jobs are already the gate for `./scripts/test.sh` — they represent the full local verification pipeline.

2. **`required_approving_review_count: 0` + `dismiss_stale_reviews: true` + `enforce_admins: false`**: chosen over a positive review count. `alanvardy/vardy` is a personal repo with a single maintainer who cannot self-approve. A review count of 1 would block every merge. Setting count to 0 with `dismiss_stale_reviews` enables review infrastructure proactively: if a collaborator reviews and the PR is updated, the review is dismissed. `enforce_admins: false` preserves the escape hatch for the admin to bypass required checks in emergencies.

3. **`required_linear_history: true` + repo-level `allow_merge_commit: false`, `allow_squash_merge: false`**: chosen over branch-protection-only or repo-level-only. `required_linear_history` on branch protection enforces the rule at the merge level; the repo-level settings disable the UI buttons for all branches. Together they match the documented convention and prevent accidental merge-commit creation. The `allow_rebase_merge: true` repo-level setting is left as-is (it's the intended method).

4. **`scripts/branch-protection.sh` (idempotent bash script)**: chosen over a doc-only approach. A script is reproducible, self-documenting, and eliminates copy-paste errors. The script uses `gh api` (already authenticated with `repo` scope), follows the `set -euo pipefail` pattern, and includes a verify step. A one-line reference in `AGENTS.md` points to it.

## What We're NOT Doing

- **Not adding a GitHub Actions workflow** to manage branch protection (no `terraform`, no CI-driven admin). The script is run once by the maintainer.
- **Not changing `fly-deploy.yml`**: the `workflow_run` gate on CI workflow conclusion is unchanged. Required status checks are on PRs, not on pushes to `main` — fly-deploy watches pushes, so it's unaffected.
- **Not changing `dependabot_auto_merge.yml`**: the `use-github-auto-merge: true` and `merge-method: rebase` settings are already correct. Native auto-merge waits on required checks (which now include the Dependabot Auto Merge job itself).
- **Not adding `restrictions` (code owner / push restrictions)**: the repo has no team structure.
- **Not adding `required_conversation_resolution`**: PRs are small and solo-maintained; the overhead is not justified.
- **Not adding `lock_branch`**: no need to prevent deletion of the protected branch beyond the default GitHub behavior.
- **Not changing the `rust-version-bump.yml` PR creation workflow**: it opens PRs against `main`, which will go through the new protection rules. That's correct.

## Open Risks

- **Context name drift**: if a CI job `name:` changes in `ci.yml`, the required check context string changes, and the protection config must be updated to match. The script documents the context strings explicitly, making it the canonical place to update.
- **Dependabot auto-merge with `required_linear_history`**: GitHub native auto-merge rebases the PR branch before merging. With `required_linear_history`, the branch must be up-to-date with the base — auto-merge handles this automatically, but stale PRs may need manual intervention. This is the existing behavior (the protection just formalizes it).
- **`required_linear_history` + `enforce_admins: false`**: the admin can bypass the linear history requirement. If the admin accidentally uses `gh pr merge --merge` or `--squash`, it will bypass the protection. The repo-level merge method settings mitigate this by disabling those buttons in the UI, but the CLI will still allow it.
- **`Dependabot Auto Merge / auto-merge` as a required check**: if the `dependabot_auto_merge.yml` workflow is ever removed or renamed, the context string changes, and merges will be blocked until the protection config is updated.