# Task

Auto-merge Dependabot pull requests in the `vardy` repo once CI passes,
replacing the `@dependabot merge` comment command that GitHub removed (see the
linked changelog / migration guidance). GitHub's guidance is to enable native
pull-request auto-merge instead of comment commands, which is a change to the
repo's CI/CD and merge-flow configuration.

A `dependabot_auto_merge.yml` workflow already exists on `main`
(`pull_request_target` + `fastify/github-action-merge-dependabot@v3` with
`use-github-auto-merge: true`, target `minor`, rebase method). The work must
verify that this — or the correct replacement per GitHub's migration guidance —
actually merges Dependabot PRs after required checks (branch protection: strict
status checks, linear-history/rebase-only, 0 approvals) pass, fixing whatever
prevents it today.

## Why LARGE

UNKNOWNS + CONVENTION_RISK (+ likely DESIGN_SIGN-OFF): at least three open
questions need research and GitHub behavior verification — the exact migration
path GitHub now expects, whether the existing fastify/`use-github-auto-merge`
approach actually works (or is already broken) for Dependabot PRs, and what
token/secret scope is required for the auto-merge API mutation on a
dependabot-triggered workflow. It also touches shared CI/CD and the repo's
branch-protection/merge settings, where a misstep changes how every PR merges.