# Task

Auto-merge Dependabot pull requests in the `vardy` repo once CI passes,
replacing the `@dependabot merge` comment command that GitHub deprecated on
Jan 27, 2026. GitHub's migration guidance recommends native pull-request
auto-merge.

A `dependabot_auto_merge.yml` workflow already exists on `main`
(`pull_request_target` + `fastify/github-action-merge-dependabot@v3` with
`use-github-auto-merge: true`, target `minor`, rebase method). The work must
verify that this — or the recommended replacement — actually merges Dependabot
PRs after required checks (branch protection: strict status checks,
linear-history/rebase-only, 0 approvals) pass, and fix whatever prevents it.

This is a change to the repo's CI/CD and merge-flow configuration.