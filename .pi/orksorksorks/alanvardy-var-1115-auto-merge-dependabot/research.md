# Research Findings

## Q1: How does `.github/workflows/dependabot_auto_merge.yml` trigger and behave?

### Findings
- Only event: `on: pull_request_target:` with no `types:`/`branches:`/`paths:` filter — fires for every PR activity on every PR (dependabot_auto_merge.yml:10-11). Runs in the *base branch* context with a write-capable token.
- In-file comments document intent: base-branch write context is required to approve/auto-merge Dependabot PRs; warns never to check out the PR head here (RCE risk) — and no head checkout exists (dependabot_auto_merge.yml:7-9).
- `permissions:` granted (top-level, explicit scopes not `write-all`): `pull-requests: write` and `contents: write` (dependabot_auto_merge.yml:13-15).
- Job `auto-merge`, `runs-on: ubuntu-latest`, gated `if: github.actor == 'dependabot[bot]'` (dependabot_auto_merge.yml:18-20).
- Action: `fastify/github-action-merge-dependabot@v3` with inputs `target: minor`, `merge-method: rebase`, `use-github-auto-merge: true` (dependabot_auto_merge.yml:22-26).
- Only `target` value present is `minor`; no comment on major/patch semantics in this file. No `concurrency:` block on this workflow.

## Q2: What merge/review/branch rules does `scripts/branch-protection.sh` enforce, and how do CI job names map to REQUIRED_CONTEXTS?

### Findings
- Operates on `main` (default branch detected from origin HEAD) (branch-protection.sh:42-71, :96).
- Branch protection (`verify_protection`, :71-152):
  - `required_status_checks.strict = true` — branch must be up-to-date with base (:80-87).
  - contexts must exactly equal REQUIRED_CONTEXTS (sorted compare) (:90-100).
  - `required_linear_history.enabled = true` — no merge commits, rebase required (:103-108).
  - `dismiss_stale_reviews = true` (:112-118); `required_approving_review_count = 0` — no human approval gate (:120-126).
  - `enforce_admins.enabled = false` — admins NOT exempt (:128-134).
  - `allow_force_pushes = false` (:136-141); `allow_deletions = false` (:143-148); `restrictions: null` (:186).
- Repo merge settings (`verify_repo_settings`, :156-176): `allow_merge_commit = false`, `allow_squash_merge = false`, `allow_rebase_merge = true` — **rebase-only**; enforced at repo level too.
- `REQUIRED_CONTEXTS` array (:17-24): `Cargo CI Tests`, `TODO and FIXME`, `Rust-fmt (Cargo Format)`, `Clippy (Cargo Clippy Lint Check)`, `CSS Drift Check`.
- All 5 contexts come from `ci.yml` jobs (the `CI / ` prefix is a UI grouping artifact, not part of the check-run name):
  - `Cargo CI Tests` → ci.yml:36-37 (`test`) · `TODO and FIXME` → ci.yml:89-90 (`todos`) · `Rust-fmt (Cargo Format)` → ci.yml:100-101 (`fmt`) · `Clippy (Cargo Clippy Lint Check)` → ci.yml:110-111 (`clippy`) · `CSS Drift Check` → ci.yml:129-130 (`css-drift`).
- **Not** in REQUIRED_CONTEXTS: ci-secure.yml jobs (`CodeQL (…)`, `Run rust-clippy analyzing`) are not gatekeepers — ci-secure.yml triggers only on `schedule:` (ci-secure.yml:4). rust-version-bump.yml, fly-deploy.yml, dependabot_auto_merge.yml jobs are not required checks either.
- Mergeable condition: branch current with main (strict), all 5 contexts green, rebase method, no approvals required; merge/squash commits disabled everywhere.

## Q3: What else in `.github/` relates to Dependabot / auto-merge-adjacent behavior?

### Findings
- `.github/dependabot.yml` — two ecosystems, both daily, both `assignees: [alanvardy]`, `commit-message: {prefix: chore, include: scope}` (dependabot.yml:7-13, 17-23):
  - cargo → group `cargo-all` (`patterns: ["*"]`, `update-types: ["major","minor","patch"]`) (:14-15).
  - github-actions → group `gha-all`, identical (:24-25).
  - No `labels`, no `open-pull-requests-limit`, no `rebase-strategy` for Dependabot PRs.
- Event types across workflows: only `pull_request_target` is dependabot_auto_merge.yml:11; `pull_request` (read context) only in ci.yml:16; others are `schedule`, `push`, `workflow_run`, `workflow_dispatch`.
- Tokens/secrets: `secrets.MYTOKEN` only in rust-version-bump.yml:49 (peter-evans/create-pull-request); `secrets.CODECOV_TOKEN` ci.yml:74,:82; `secrets.FLY_API_TOKEN` fly-deploy.yml:22. No others.
- `permissions:`/`concurrency:` patterns: dependabot_auto_merge.yml has no `concurrency`; rust-version-bump.yml:12-18 and ci.yml:27-33 and ci-secure.yml:6-15 and fly-deploy.yml:9-16 each have their own blocks.
- **Native auto-merge enablement is only referenced transitively** via `use-github-auto-merge: true` in dependabot_auto_merge.yml:22-26. No literal "Allow auto-merge", `enablePullRequestAutoMerge`, or `auto-approve` outside that file. The repo "Allow auto-merge" setting is not confirmed anywhere in the repo.

## Q4 (web): What does GitHub's migration guidance prescribe for auto-merging Dependabot PRs?

### Findings
- Deprecation timeline: announced 2025-10-06/07, removal effective 2026-01-27 (cloud + GHES 3.20). Removed commands: `@dependabot merge`, `squash and merge`, `cancel merge`, `close`, `reopen`. Remaining: `rebase`, `recreate`, `ignore`/`unignore`/`show` family.
  - [Changelog 2025-10-07](https://github.blog/changelog/2025-10-07-upcoming-changes-to-github-dependabot-pull-request-comment-commands/) · [Changelog 2026-01-27](https://github.blog/changelog/2026-01-27-changes-to-github-dependabot-pull-request-comment-commands/)
- Prescribed replacement: GitHub **native auto-merge** via an Actions workflow on `pull_request_target` gated `if: github.actor == 'dependabot[bot]'` calling `gh pr merge --auto`, often with `dependabot/fetch-metadata@v2` to classify update type (commonly exclude `version-update:semver-major`).
  - [Automating Dependabot with GitHub Actions](https://docs.github.com/en/code-security/dependabot/working-with-dependabot/automating-dependabot-with-github-actions)
- Prerequisite: repo setting **Settings → General → Pull Requests → "Allow auto-merge"** must be enabled; auto-merge completes the merge only once checks/approvals required by branch protection are met. "Allow GitHub Actions to create and approve pull requests" is needed if the workflow approves the PR.
- Token scopes: official example declares `permissions: pull-requests: write` + `contents: write`. Note: `pull_request`/`pull_request_review`/`pull_request_review_comment`/`push` workflows triggered by Dependabot get a default read-only token; `pull_request_target` (write token + secrets from Actions) is the canonical pattern.
  - [Dependabot on GitHub Actions](https://docs.github.com/en/code-security/reference/supply-chain-security/dependabot-on-actions)
- Behavior change: native auto-merge merges as soon as required checks pass and waits **only** for required (not conditional/non-required) checks — a known divergence from the old bot-merge. [Discussion #176097](https://github.com/orgs/community/discussions/176097)

## Q5 (web): Does `fastify/github-action-merge-dependabot@v3` with `use-github-auto-merge: true` still function after the deprecation?

### Findings
- Yes — the action never used the `@dependabot merge` comment commands; it drives the GitHub API directly. `use-github-auto-merge: true` arms GitHub's native auto-merge, the exact mechanism GitHub prescribes. No upstream fastify issue documents breakage from the 2026 deprecation.
- `use-github-auto-merge` (input, default `false`): instead of merging, marks the PR for auto-merge; GitHub performs the merge once required checks pass. Reports `merge_status: auto_merge` (other values: `approved`, `merged`, `merge_failed`, `skipped:*`).
- Prerequisites (documented): (a) repo "Allow auto-merge" setting on, (b) base branch has branch protection with required status checks, (c) status checks are **not yet satisfied** — GitHub rejects arming auto-merge on an already-mergeable PR.
- GraphQL flow: query PR node id (`repository.pullRequest(id)`), then `enablePullRequestAutoMerge` with `pullRequestId` and `mergeMethod` mapped from `merge-method` input (`SQUASH` default, `MERGE`, `REBASE`). Failure modes: missing permissions → "Resource not accessible by integration"; repo setting off → "Pull request Auto merge is not allowed for this repository"; token actor cannot push to protected base → "User is not authorized for this protected branch".
- Token: defaults to `github.token` (automatic GITHUB_TOKEN); v3 removed the GitHub App and `api-url` inputs. Needs `pull-requests: write` (approve) + `contents: write` (merge; not needed with `approve-only: true`). Auto-merge GraphQL path needs both.
- Inputs: `target` ∈ major/minor/patch/any (default `any`, so patch AND major both merge unless overridden); `merge-method` (squash/merge/rebase); `approve-only`, `comment`, `skip-commit-verification` (GHES), `event-name` (supports `pull_request` and `pull_request_target`), `skip-verification`. `pull_request_target` is a supported trigger.
- v2→v3 was the last breaking change (App + `api-url` removed, `permissions:` block now required); no deprecation tied to the 2026 command removal.
  - [fastify action](https://github.com/fastify/github-action-merge-dependabot) · [action.yml](https://github.com/fastify/github-action-merge-dependabot/blob/d5b96419a7ef6b606cb08d1c194e1c561cbab975/action.yml) · [PR #130 migration](https://github.com/fastify/github-action-merge-dependabot/pull/130) · [Issue #67](https://github.com/fastify/github-action-merge-dependabot/issues/67) · [Disc. #24686](https://github.com/orgs/community/discussions/24686)

## Cross-Cutting Observations
- The existing `dependabot_auto_merge.yml` (pull_request_target + fastify v3 + `use-github-auto-merge: true`, `merge-method: rebase`, explicit scopes) already matches GitHub's prescribed native-auto-merge pattern end-to-end.
- Repo settings are consistent with native auto-merge: rebase-only merges, strict status checks, linear history, 0 approvals — auto-merge can therefore arm (only required checks gate) and rebase is the allowed method that the action requests.
- `target: minor` (not `any`) means major Dependabot updates are NOT auto-merged. **Decision (user, research checkpoint): majors should be auto-merged too** → `target` must change to `any`. Caveat: a single fastify `target` applies to all Dependabot PRs this workflow sees, so `target: any` also enables auto-merging major GitHub-Actions updates (`gha-all` group includes major, dependabot.yml:24-25) — consider whether that's desired or needs an ecosystem-specific filter (e.g. dependabot/fetch-metadata) in design.
- Critically, neither the repo nor `branch-protection.sh` confirms the **repo "Allow auto-merge" setting** is enabled — this is the most likely thing blocking merges today and is not verified anywhere in the repo.

## Open Areas
- Whether the "Allow auto-merge" repository setting is actually enabled cannot be determined from the codebase (it is a GitHub UI/Settings toggling, not represented in-repo). This must be checked against the live repo (Settings → General → Pull Requests) or via the REST `allow_auto_merge` field.
- Whether auto-merge is currently observed to fail (status of open/merged Dependabot PRs) was not in scope; a live check would confirm the actual failure mode.
- Web-research claims for Q4/Q5 were sourced from search-result summaries and docs without a full live workflow run; the fastify "still works" conclusion is inferred from design + absence of breakage reports.
- Exact deprecation announcement date conflates 2025-10-06 vs 10-07 across sources; treat "October 2025" as the announcement window.