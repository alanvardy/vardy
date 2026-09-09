# Research Findings

## Q1: How does the existing job-level permissions pattern work in ci.yml?

### Findings
- **Workflow-level permissions** (ci.yml:27-30) declare `contents: read`, `pull-requests: write`, `actions: write` as the default for all jobs without their own override.
- **Job-level override syntax** (ci.yml:131-132) uses `permissions:` at the job object level with explicit scope mappings: `contents: read` only.
- **Replacement semantics** — job-level permissions completely replace workflow-level permissions for that job; omitted scopes default to `none` (ci.yml:131-132).
- **Scope applies to GITHUB_TOKEN** used by the job's steps (ci.yml:134-146), not other jobs or the workflow-level block itself.
- The css-drift job's override is consistent with its actual needs: `actions/checkout` (ci.yml:134) and `git diff` (ci.yml:146) require only read access.

## Q2: What permissions are actually required by codecov-action upload steps?

### Findings
- **codecov-action** (codecov/codecov-action@v7 or actions/codecov@v3) requires `contents: read` for repository checkout and `pull-requests: write` for uploading test_results on PRs (ci.yml:71, :79).
- The current repository-level permissions (ci.yml:27-30) include `actions: write` which appears **unused** by codecov-action.
- Codecov uploads use `secrets.CODECOV_TOKEN` (ci.yml:73, :81) for authentication, not GITHUB_TOKEN for the upload API calls.
- The `actions: write` scope is not required for codecov functionality and may be unnecessary for the workflow.

## Q3: Why does fly-deploy.yml have no explicit permissions block?

### Findings
- **Template origin** — fly-deploy.yml matches the fly.io "Continuous Deployment with GitHub Actions" template verbatim (line 1 comment references fly.io docs) (fly-deploy.yml:1-20).
- **No GITHUB_TOKEN usage** — the workflow uses `secrets.FLY_API_TOKEN` (fly-deploy.yml:20) for deployment authentication, not GITHUB_TOKEN (verified via grep for GITHUB_TOKEN references).
- **Default token behavior** — without a permissions block, GITHUB_TOKEN inherits repository/org default (post-Feb 2023: read-only) (fly-deploy.yml:16-20).
- **Functional requirement** — no explicit permissions are needed; `actions/checkout@v7` (fly-deploy.yml:16) needs only `contents: read` which the default grants.
- **Residual risk** — the workflow's token posture depends on repo/org settings rather than being self-describing.

## Q4: What are the minimum permissions for dependabot_auto_merge.yml?

### Findings
- **Action behavior** — fastify/github-action-merge-dependabot@v3 with `use-github-auto-merge: true` (dependabot_auto_merge.yml:22) calls `approvePullRequest` and `enableAutoMergePullRequest` API endpoints.
- **Required scopes** — `pull-requests: write` is required for approval and auto-merge mutation (dependabot_auto_merge.yml:18-22).
- **Unnecessary scope** — `contents: write` is NOT required when `use-github-auto-merge: true` because the merge endpoint is unreachable (action.js:164-168 short-circuits) (dependabot_auto_merge.yml:9-11).
- **Recommended minimal form** — `pull-requests: write` + `contents: read` at job level (dependabot_auto_merge.yml:9-11).
- **Critical finding** — Dependabot-triggered runs get read-only GITHUB_TOKEN by default; this workflow needs `pull_request_target` or repo-level "Send write tokens" setting to function (dependabot_auto_merge.yml:7).

## Q5: How do ci-secure.yml's permission requirements differ between workflow-level and job-level?

### Findings
- **Workflow-level permissions** (ci-secure.yml:10-14) declare `contents: read`, `security-events: write`, `packages: read`, `actions: read` as the baseline.
- **Job-level override** (ci-secure.yml:57-60) for clippy-analyze declares `contents: read`, `security-events: write`, `actions: read` — drops `packages: read`.
- **Replacement semantics** — the clippy-analyze job-level block completely replaces the workflow-level block; `packages: read` is dropped (ci-secure.yml:57-60).
- **Unconsumed scope** — `packages: read` (ci-secure.yml:13) has no consumer in the workflow; `actions: read` (ci-secure.yml:14) is only used by clippy-analyze.
- **Valid minimization strategy** — the two-level pattern (workflow baseline + job narrowing) is valid but incomplete; could trim `actions: read` from both levels (ci-secure.yml:10-14, :57-60).

## Q6: What security best practices exist for GitHub Actions workflows?

### Findings
- **Action pinning patterns** — SHA pinning exists (ci.yml:42 with `# v7.0.1` comment); tag pinning at minor.patch (ci-secure.yml:43, :50, :84 with v4.37.9); major-only tags (actions/checkout@v7, codecov/codecov-action@v7); floating channel tags (dtolnay/rust-toolchain@stable); branch pinning (superfly/flyctl-actions@master) (codebase-pattern-finder report).
- **Secret usage patterns** — all secrets use `${{ secrets.<NAME> }}` naming convention; passed as action input `token:` (CODECOV_TOKEN, MYTOKEN) or via `env:` (FLY_API_TOKEN) (codebase-pattern-finder report).
- **Documentation** — no documentation or comments about permission minimization exist (codebase-pattern-finder report).
- **Job-level permissions examples** — only two: css-drift (ci.yml:131-132) and clippy-analyze (ci-secure.yml:57-60) (codebase-pattern-finder report).
- **Concurrency hygiene** — `concurrency:` groups with `cancel-in-progress: true` exist in ci.yml:35-37, ci-secure.yml:6-8, rust-version-bump.yml:16-17, fly-deploy.yml:14 (codebase-pattern-finder report).

## Q7: How does rust-version-bump.yml's permission configuration compare?

### Findings
- **Permission configuration** — workflow-level `contents: write`, `pull-requests: write` (rust-version-bump.yml:12-14), no job-level override.
- **PAT authentication** — uses `secrets.MYTOKEN` (rust-version-bump.yml:49) for create-pull-request action, making workflow-level permissions redundant for the action.
- **Comparison** — only workflow granting `contents: write` to a scheduled job; only workflow using PAT instead of GITHUB_TOKEN for write operations (rust-version-bump.yml:12-14, :49).
- **Security posture** — the declared permissions block is belt-and-braces; actual capability comes from MYTOKEN's scopes (rust-version-bump.yml:12-14).
- **CI signal requirement** — MYTOKEN is needed because GITHUB_TOKEN commits cannot trigger downstream workflows (rust-version-bump.yml:1-3).

# Cross-Cutting Observations

- **Two-level permission pattern** — ci.yml and ci-secure.yml establish a pattern of workflow-level baseline + job-level narrowing (ci.yml:27-30, :131-132; ci-secure.yml:10-14, :57-60).
- **Permission scope redundancy** — `actions: write` in ci.yml:30 appears unused; `packages: read` in ci-secure.yml:13 is unconsumed; `contents: write` in dependabot_auto_merge.yml:11 is unreachable.
- **Token authentication diversity** — workflows use GITHUB_TOKEN (ci.yml, ci-secure.yml), secret tokens (CODECOV_TOKEN, FLY_API_TOKEN, MYTOKEN), with varying permission block dependency.
- **Pinning inconsistency** — SHA pinning only in ci.yml; tag pinning varies across workflows; branch pinning only in fly-deploy.yml.
- **Documentation gap** — no explicit permission minimization documentation exists in the codebase.

# Open Areas

- Whether codecov-action truly needs `pull-requests: write` for test_results uploads on PRs (ci.yml:79).
- Whether `actions: read` in ci-secure.yml:14, :60 is necessary for upload-sarif status polling in private repos.
- Whether `contents: write` in dependabot_auto_merge.yml:11 could be removed entirely given `use-github-auto-merge: true`.
- Whether the PAT-based rust-version-bump workflow should have its permissions block removed entirely since MYTOKEN provides the capability.
- Whether fly-deploy.yml should add `permissions: contents: read` for explicitness despite not being functionally required.