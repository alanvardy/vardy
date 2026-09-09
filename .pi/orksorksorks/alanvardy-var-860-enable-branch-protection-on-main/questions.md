# Research Questions

## Context

This repo is a Rust (axum) web application with a GitHub Actions CI
footprint (`.github/workflows/`), a shell-script tooling layer
(`scripts/`), and developer conventions documented in `AGENTS.md` and
`README.md`. Focus on the CI workflow definitions, the merge/PR automation
workflows, the documented merge conventions, the scripting patterns, and
the mapping between local verification commands and CI jobs. [What is
being built is not relevant to these questions.]

## Questions

1. Enumerate every GitHub Actions check that can surface as a check run on
   a pull request to `main` or on a push to `main`: for each, give the
   exact context string that would identify it (workflow and job display
   names), the `file:line` defining it, and the triggering events with
   branch filters. Also list the check-producing jobs that run only on
   `schedule` or `workflow_run` and therefore never appear on PRs.

2. Trace how the existing merge/PR automation workflows interact with the
   `main` branch: what `dependabot_auto_merge.yml`,
   `fly-deploy.yml`, `rust-version-bump.yml`, and `.github/dependabot.yml`
   each do, what they wait on (check success, workflow conclusions,
   notifications, approval), what merge method they use, and any explicit
   or implicit assumptions about branch protection or required checks.

3. What is the documented merge/commit convention of this repo? Search
   `AGENTS.md` (both the repo copy and the home `~/.pi/agent/AGENTS.md`
   conventions file), `README.md`, `research.md`, and any other markdown
   files for statements about how code reaches the `main` branch: rebase
   vs merge commits vs squash, linear history, merge-with-`--rebase`
   instructions, review requirements, or references to branch protection.

4. What scripting and automation patterns do the repo's maintenance
   scripts and workflows follow? For each file under `scripts/`, describe
   its shell style (bash/fish, `set -e`, error handling), how it loads
   config or `.env`, and what it does. Search the whole repo (including
   `.github/workflows/`) for any use of the `gh` CLI or GitHub REST API,
   and for how secrets/tokens are referenced in workflows. Note any
   precedent for scripts that perform repository-level administration.

5. How do the repo's local verification commands map to the CI jobs?
   Describe what `scripts/test.sh` runs step by step, which ci.yml job
   each command corresponds to (formatter, clippy, tests, CSS-drift,
   TODO/FIXME lint), and what `.config/nextest.toml`, `codecov.yml`, and
   the JUnit/coverage uploads contribute. Note any environment
   requirements or gotchas (e.g. `.env`, DATABASE_URL, SQLX_OFFLINE,
   regenerated `static/site.css`).

## How to Report

For your assigned question, describe what exists with `file:line`
references. Do not suggest improvements or propose solutions. Answer only
your assigned question. Keep your condensed report to ~100 lines.