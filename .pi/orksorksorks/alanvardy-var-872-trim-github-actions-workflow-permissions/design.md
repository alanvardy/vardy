# Design Discussion

## Current State

Three GitHub Actions workflows carry permission configurations that grant more
than their jobs actually use, and one relies on invisible repo/org defaults.

**ci.yml** — workflow-level baseline (`ci.yml:27-30`):
`contents: read`, `pull-requests: write`, `actions: write`.
Five jobs. Only `css-drift` overrides at job level with `contents: read`
(`ci.yml:131-132`). The `test` job runs two `codecov/codecov-action@v7` steps
(`ci.yml:71`, `:79`) that need `pull-requests: write` for `test_results`
upload on PRs (`ci.yml:79`). `actions: write` has **no consumer** anywhere in
the workflow (research Q2).

**fly-deploy.yml** — has **no `permissions:` block**; it is the fly.io template
verbatim (`fly-deploy.yml:1-20`). Authenticates via `secrets.FLY_API_TOKEN`
(`fly-deploy.yml:20`); `actions/checkout` (`fly-deploy.yml:16`) needs only
`contents: read`. Token posture silently depends on repo/org defaults.

**dependabot_auto_merge.yml** — workflow-level `pull-requests: write` +
`contents: write` (`:9-11`), trigger `on: [pull_request]` (`:7`). The action
uses `use-github-auto-merge: true` (`:22`), which makes `contents: write`
**unreachable** (the action short-circuits the manual-merge branch). Critically,
`pull_request` gives dependabot PRs a read-only GITHUB_TOKEN, so the declared
`pull-requests: write` cannot actually approve/auto-merge — and there is no
`github.actor == 'dependabot[bot]'` guard despite the header comment claiming it
only runs for dependabot (research Q4).

## Desired End State

**ci.yml** — workflow baseline `contents: read` only; job-level
`pull-requests: write` on the `test` job; `actions: write` removed.

**fly-deploy.yml** — explicit `permissions: contents: read`, self-describing,
independent of repo/org defaults.

**dependabot_auto_merge.yml** — `permissions: pull-requests: write` +
`contents: read`; trigger switched to `pull_request_target`; explicit
`if: github.actor == 'dependabot[bot]'` guard added.

### Verification

- YAML parses and GitHub accepts each workflow (no invalid-key rejection).
- `grep -R "actions: write" .github/workflows` returns nothing after the change.
- Workflow-level blocks contain only `contents: read` across the three files
  (except dependabot's `pull-requests: write`, justified by Q4).
- A CI run on this PR (push + pull_request paths) stays green; css-drift and
  codecov uploads still succeed.
- Next dependabot PR is auto-approved/merged (confirms `pull_request_target`
  actually grants the write token).

## Patterns to Follow

**Good — keep matching these:**

- **Two-level permission pattern** (workflow baseline + job-level narrowing),
  already used at `ci.yml:27-30` + `:131-132` and `ci-secure.yml:10-14` +
  `:57-60`. Our change *adds* a scope at job level rather than narrowing, but
  the baseline-first shape is the same.
- **Job-level replacement semantics** — an omitted scope defaults to `none`
  (`ci.yml:131-132`). Jobs with no `permissions:` block inherit the baseline.
- **Inline rationale comments** — precedent at `ci-secure.yml:60`
  (`# for upload-sarif in private repos`). We'll add one-line reasons at each
  permission block we change.
- **SHA pinning with version comment** — `ci.yml:42`
  (`@3d3c42e... # v7.0.1`). Not expanded in this task, but we must not
  *loosen* any existing pin.
- **Concurrency hygiene** — `concurrency:` + `cancel-in-progress: true`
  (`ci.yml:35-37`, `fly-deploy.yml:14`). Untouched, preserved.

**Bad — do NOT replicate:**

- **Unconsumed scopes** — `actions: write` (`ci.yml:30`) and `packages: read`
  (`ci-secure.yml:13`) are the anti-pattern this task fixes. Do not add new
  grant-anything-extra scopes.
- **Unreachable `contents: write`** under `use-github-auto-merge: true`
  (`dependabot_auto_merge.yml:22` → action.js short-circuit, research Q4).
- **Missing actor guard on a merge-capable workflow** — the current
  dependabot workflow claims dependabot-only but has no `github.actor` check.
- **Branch pinning** (`superfly/flyctl-actions@master` in fly-deploy.yml) —
  out of scope to change here, but flagged so nothing new is branch-pinned.

## Design Decisions

1. **Scope = the three named workflows only** (`ci.yml`, `fly-deploy.yml`,
   `dependabot_auto_merge.yml`) — literal task scope, keeps the PR tight and
   reviewable. `ci-secure.yml` (`packages: read` unconsumed) and
   `rust-version-bump.yml` (redundant block under MYTOKEN PAT) are
   follow-up candidates, not part of this change (Q1-A).

2. **ci.yml: baseline `contents: read`, escalate `pull-requests: write` at the
   `test` job** — the only job with codecov upload steps (`ci.yml:79`). Drop
   `actions: write`. Matches the existing baseline-first pattern; keeps a
   read-only safety net for future jobs (Q2-A).

3. **fly-deploy.yml: add `permissions: contents: read`** — explicit posture,
   removes silent dependence on repo/org defaults; no functional change since
   checkout only needs read (Q3-A).

4. **dependabot_auto_merge.yml: trim + fix the trigger** — `pull-requests:
   write` + `contents: read`, switch `on: [pull_request]` → `on:
   pull_request_target`, and add `if: github.actor == 'dependabot[bot]'`.
   `pull_request_target` runs in base-branch context with a write-capable
   token, which is what allows approve/auto-merge for dependabot PRs; the step
   does no checkout, so the untrusted-code surface is nil (Q4-B).

5. **Document via inline rationale comments** — one line at each changed /
   added `permissions:` block explaining the minimum set, matching the
   `ci-secure.yml:60` precedent. No separate ADR/SECURITY doc (Q5-A).

## What We're NOT Doing

- **Not** touching `ci-secure.yml` or `rust-version-bump.yml` (follow-up scope).
- **Not** changing action-pinning strategy — no SHA↔tag↔branch conversions;
  preserve all existing pins exactly.
- **Not** altering triggers, `concurrency:` blocks, `env:`, or secret usage
  anywhere except the dependabot trigger switch (Decision 4).
- **Not** adding checkout/run steps or changing job names in `ci.yml`.
- **Not** adding a standalone CI/SECURITY documentation page — inline comments
  only (Decision 5).
- **Not** introducing a new action or removing the
  `fastify/github-action-merge-dependabot@v3` action.

## Open Risks

- **`pull_request_target` surface** — mitigated (no checkout, actor guard), but
  the workflow now runs in base context; any future step that checks out the
  PR head here would reintroduce a RCE risk. Comment in the file to forbid it.
- **Does auto-merge actually fire post-change?** — the write token in
  `pull_request_target` context is the documented fix, but the first dependabot
  PR after merge is the real confirmation (see Verification).
- **codecov `test_results` upload on PRs** — the `test` job's
  `pull-requests: write` must remain job-scoped; the existing `if`
  (`ci.yml:79`) already excludes fork PRs, so no fork-token exposure.
- **fly-deploy checkout pinned to major tag `@v7`** (`fly-deploy.yml:16`) and
  branch pin `superfly/flyctl-actions@master` remain as-is — accepted residual
  risk, out of scope.
- **YAML-only change** — the repo gate (`./scripts/test.sh`) is Rust-only and
  will not validate these files; verification is grep + a real GitHub Actions
  run, not an automated local gate.