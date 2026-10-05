# Research Questions

## Context

Focus on the GitHub Actions workflow definitions under `.github/workflows/`
(especially `dependabot_auto_merge.yml`, `ci.yml`, `ci-secure.yml`,
`rust-version-bump.yml`), the Dependabot config `.github/dependabot.yml`, and
the repo merge/branch-protection tooling in `scripts/branch-protection.sh`.
Two questions target external GitHub behavior and are answered by web
research. Do not propose solutions or improvements; describe what exists.

## Questions

1. [codebase-analyzer] How does `.github/workflows/dependabot_auto_merge.yml`
   currently trigger and behave — exactly which event types (`on:` block),
   what GITHUB_TOKEN permissions it declares, the `pull_request_target`
   context (runs in base-branch context with write token), the
   `if: github.actor == 'dependabot[bot]'` filter, and each
   `fastify/github-action-merge-dependabot` input (`target`, `merge-method`,
   `use-github-auto-merge`) plus the documented prerequisites those inputs
   imply?

2. [codebase-pattern-finder] What merge/review/branch rules does
   `scripts/branch-protection.sh` enforce for `main` (strict status checks,
   linear history, approval count, merge methods, admin enforcement, force
   push/deletion), and how do the CI job `name:` values across
   `.github/workflows/*.yml` map to the `REQUIRED_CONTEXTS` array? Under these
   settings, what must be true before a PR is mergeable?

3. [codebase-locator] Across `.github/`, what other configuration relates to
   Dependabot-triggered or auto-merge-adjacent behavior: dependabot.yml
   update groups and their scopes, tokens/secrets referenced by workflows
   (e.g. MYTOKEN), `permissions:`/`concurrency:` patterns, any use of
   `pull_request_target` vs `pull_request`, and whether "Allow auto-merge"
   or any native auto-merge enablement is referenced anywhere in the repo?

4. [researcher - web] Per GitHub's migration guidance (the 2025-10-06 and
   2026-01-27 changelog entries "Changes to GitHub Dependabot pull request
   comment commands"), what does GitHub now prescribe for automatically
   merging Dependabot PRs after CI passes? Cover the recommended workflow
   event/actor detection, the "Allow auto-merge" repository setting
   prerequisite, required branch protection/status checks, and the exact
   GITHUB_TOKEN scopes (e.g. contents:write, pull-requests:write) required.

5. [researcher - web] Does the `fastify/github-action-merge-dependabot@v3`
   action with `use-github-auto-merge: true` still function for Dependabot
   PRs after the comment-command deprecation? Document its current behavior,
   the GraphQL `enablePullRequestAutoMerge` mutation it invokes, its default
   token and required permissions, the `target`/`merge-method` semantics it
   supports, and any documented limitations or known deprecations.