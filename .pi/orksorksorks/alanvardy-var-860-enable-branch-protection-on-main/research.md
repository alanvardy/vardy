# Research Findings

Repo: Rust (axum) web app, CI via GitHub Actions, deploy to Fly.io.
Branch protection is currently NOT configured (no protection config exists in
repo or workflows; the only literal "branch protection" mention is a comment
at `.github/workflows/dependabot_auto_merge.yml:3`).

## Q1: GitHub Actions checks that can surface on PRs to / pushes of `main`

### Findings
- Workflow `CI` (`.github/workflows/ci.yml:7`) triggers on `push` (branches:
  `main`, `ci.yml:14-15`), `pull_request` (no branch filter, `ci.yml:16`),
  `workflow_dispatch` (`ci.yml:17`). All five jobs fire for both PR and push
  events; no per-job event filters (`ci.yml` job defs at 38, 88, 99, 109, 128).
- The five check runs on any PR to `main` / push to `main` (context string =
  workflow name + job `name:`):
  1. `CI / Cargo CI Tests` — job `test`, `name: Cargo CI Tests` (`ci.yml:38-39`).
     PR path: step "Cargo Test (PR — fast, no coverage)" `if: pull_request`
     (`ci.yml:60-61`); main path: "Cargo Test (main — with coverage)" `if:
     push && ref == refs/heads/main` (`ci.yml:65-66`).
  2. `CI / TODO and FIXME` — job `todos` (`ci.yml:88-89`); runs
     `./scripts/lint_string.sh` five times (`ci.yml:93-97`).
  3. `CI / Rust-fmt (Cargo Format)` — job `fmt` (`ci.yml:99-100`);
     `cargo fmt --all -- --check` (`ci.yml:107`).
  4. `CI / Clippy (Cargo Clippy Lint Check)` — job `clippy` (`ci.yml:109-110`);
     `cargo clippy --all-targets --all-features --locked -- -D warnings`
     (`ci.yml:126`).
  5. `CI / CSS Drift Check` — job `css-drift` (`ci.yml:128-129`); rebuild CSS +
     `git diff --exit-code -- static/site.css` (`ci.yml:145`).
- `Dependabot Auto Merge / auto-merge` also surfaces as a check on every PR
  to `main` — workflow `name: Dependabot Auto Merge`
  (`.github/workflows/dependabot_auto_merge.yml:5-6`), job `auto-merge` with
  no `name:` (job key becomes display name, `dependabot_auto_merge.yml:14`),
  trigger `on: [pull_request]` no branch filter (`dependabot_auto_merge.yml:7`).
- Upload steps (codecov coverage, JUnit test results) do NOT create separate
  check runs — they're steps inside `Cargo CI Tests` (`ci.yml:69-86`).
- Checks that NEVER appear on PRs (schedule/workflow_run only):
  - `CodeQL Analysis (Weekly) / CodeQL (actions)` and `/ CodeQL (rust)` —
    matrix job `analyze`, `name: CodeQL (${{ matrix.language }})`
    (`ci-secure.yml:17-19`, matrix `ci-secure.yml:22-26`).
  - `CodeQL Analysis (Weekly) / Run rust-clippy analyzing` — `clippy-analyze`
    (`ci-secure.yml:54-55`).
  - `Update Rust toolchain / Bump rust-toolchain.toml` — job `bump`
    (`rust-version-bump.yml:21-22`), schedule + dispatch only.
  - `Fly Deploy / Deploy app` — job `deploy` (`fly-deploy.yml:10-11`),
    `workflow_run` on "CI" completed for `main` only (`fly-deploy.yml:4-8`).
- `.github/dependabot.yml` (Dependabot config) and
  `.github/workflows/codeql/codeql.yml` (CodeQL config file) produce no checks.

## Q2: Existing merge/PR automation and its interaction with `main`

### Findings
- `dependabot_auto_merge.yml`: trigger any `pull_request` (`:7`); perms
  `pull-requests: write`, `contents: write` (`:10-11`); job `auto-merge`
  (`:14`) uses `fastify/github-action-merge-dependabot@v3` (`:18`) with
  `target: 'minor'` (`:19`), `merge-method: rebase` (`:21`),
  `use-github-auto-merge: true` (`:22`). Header comment (`:1-3`): auto-approves
  and merges dependabot PRs for minor/patch "once all checks and branch
  protection rules pass" — the merge is performed by GitHub native auto-merge,
  which waits on required status checks + branch protection. Non-dependabot
  PRs no-op but the job still surfaces as a check (see Q1).
- `fly-deploy.yml`: `workflow_run` on workflow "CI" completed, branches
  `[main]` (`:4-8`); job `deploy` gated `if: conclusion == 'success'` (`:13`),
  `concurrency: deploy-group` (`:14`); deploys whatever is on `main` via
  `flyctl deploy --remote-only` + `secrets.FLY_API_TOKEN` (`:18-23`). Gate is
  the whole-CI-workflow conclusion, not individual checks or protection rules.
- `rust-version-bump.yml`: daily cron `"0 6 * * *"` + `workflow_dispatch`
  (`:8-10`); bumps `rust-toolchain.toml` channel via sed and OPENS a PR
  (`peter-evans/create-pull-request@v8`, branch `chore/rust-toolchain-bump`,
  labels `dependencies`, token `secrets.MYTOKEN`; `:45-58`). Never writes
  `main` directly (comment `:1-3`): "Opening a PR rather than committing to
  main lets the full CI suite validate the new compiler first."
- `.github/dependabot.yml`: cargo (`:4-8`) and github-actions (`:22-24`)
  ecosystems, daily schedule, commit prefix `chore: <scope>`, all updates
  grouped into `cargo-all` / `gha-all` PRs including `major` update-types
  (`:13-17`, `:29-33`). Targets default branch (`main`). Auto-merge `target:
  minor` means major grouped PRs stay open for human merge.
- No workflow or script merges PRs itself (no `gh` CLI, no REST API calls);
  merges happen via human `gh pr merge` (convention, see Q3) or GitHub
  auto-merge for dependabot.

## Q3: Documented merge/commit convention

### Findings
- Committed repo docs: only `AGENTS.md`, `README.md`, `ROUTES.md`,
  `linear-project.md`, `research.md` exist (no CONTRIBUTING.md, no docs/).
- Repo `AGENTS.md` "Commits and PRs" (`AGENTS.md:46-49`) states only:
  "`main` is the base branch when reviewing code" (`AGENTS.md:47`) and the
  resumed-session orphan rule. No statement about rebase/squash/merge
  commits, linear history, reviews, or branch protection.
- Home conventions `~/.pi/agent/AGENTS.md` (global, NOT committed):
  - "Never push directly to main — all changes go through pull requests and
    are merged into main" (lines 85-86).
  - "Merge PRs only with `--rebase` (merge commits and squashes are disabled
    on the repos) — `gh pr merge <n> --rebase --delete-branch`" (lines 86-88).
  - `hx` editor workaround for `git commit -m` (lines 90-93).
- The only committed repo signal for the merge method: `merge-method: rebase`
  in `dependabot_auto_merge.yml:21`.
- `README.md` is dev-commands only (`README.md:4-16`); `research.md` mentions
  "never push to main" only as generic best-practice notes (`research.md:50,62`).
- No committed doc guarantees linear history, review approval, or branch
  protection for this repo. The rebase/merge convention exists but lives in
  the personal home conventions file.

## Q4: Scripting and automation patterns

### Findings
- `scripts/` (4 bash scripts, no Makefile/Justfile):
  - `scripts/test.sh` (10 lines): `#!/usr/bin/env bash`, no `set -e`
    (`:1`); `set -a; source .env; set +a` (`:2`); `&&`-chained pipeline
    (`:3-9`) — see Q5 for step order.
  - `scripts/build-css.sh` (28 lines): `set -euo pipefail` (`:4`);
    pinned Tailwind v4.3.3 with per-platform SHA256 (`:6-8`), platform
    dispatch on `uname` (`:10`), download+verify into gitignored
    `target/tailwindcss-cli/`, minify build `css/site.css` →
    `static/site.css` (`:19-28`).
  - `scripts/lint_string.sh` (9 lines): no `set -e`; `find . -name "*.rs" |
    xargs grep -E "$1" | wc -l`, exit 1 if >0 (`:4-8`).
  - `scripts/reset_db.sh` (12 lines): `set -euo pipefail` (`:4`); removes
    `test.db` (`:5-11`).
- CI does NOT call `test.sh`; it spells out equivalent commands per job
  (e.g. fmt `--check` in `ci.yml:107`, clippy in `ci.yml:126`, nextest with
  `--profile ci` in `ci.yml:62`).
- No `gh` CLI usage and no GitHub REST API usage anywhere in `scripts/` or
  workflows. No precedent for a script doing repository-level administration;
  closest are dependabot auto-merge and toolchain-bump PR creation.
- Secrets referenced: `secrets.CODECOV_TOKEN` (`ci.yml:73,81`),
  `secrets.FLY_API_TOKEN` (`fly-deploy.yml:23`), `secrets.MYTOKEN`
  (`rust-version-bump.yml:51` — a dedicated PAT, notable name). No
  `GITHUB_TOKEN` reference; auth via workflow `permissions:` blocks
  (`ci.yml:28-30`, `dependabot_auto_merge.yml:10-11`,
  `rust-version-bump.yml:12-13`).
- `.env` gitignored (`.gitignore:2`); `.env_template` documents 7 vars and 4
  sync locations (`:1-11`); `.envrc` = `dotenv` (direnv) (`:1`).

## Q5: Local verification ↔ CI jobs mapping

### Findings
- `scripts/test.sh` pipeline (each step → CI job):
  1. `cargo fmt --all` (`test.sh:5`) → `CI / Rust-fmt (Cargo Format)`; CI
     runs `-- --check` (`ci.yml:107`), local applies formatting.
  2. `cargo sqlx prepare -- --tests` (`test.sh:7`) → no CI job; regenerates
     committed `.sqlx/` offline metadata (`README.md:14-16`).
  3. `cargo check --all-targets` (`test.sh:9`) → type-check; no dedicated
     CI job (covered implicitly by clippy/test-compile).
  4. `./scripts/build-css.sh` + `git diff --exit-code -- static/site.css`
     (`test.sh:11-13`) → `CI / CSS Drift Check` (`ci.yml:128-145`); CI caches
     the Tailwind binary keyed on `hashFiles('scripts/build-css.sh')`
     (`ci.yml:118-124`).
  5. `cargo clippy --all-targets --all-features --locked -- -D warnings`
     (`test.sh:15`) → `CI / Clippy (Cargo Clippy Lint Check)`, identical
     command (`ci.yml:126`).
  6. `cargo nextest run` (`test.sh:17`) → `CI / Cargo CI Tests` (`ci.yml:38`);
     CI PR path `cargo nextest run --profile ci` (`ci.yml:62`), main path with
     llvm-cov coverage → `lcov.info` (`ci.yml:67`).
  7. `! rg -i -s -g '*.rs' 'FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO\s|todo\s' src`
     (`test.sh:19-20`) → `CI / TODO and FIXME` (`ci.yml:88-97`; CI variant uses
     `lint_string.sh` with five literal args, `ci.yml:93-97`).
- `.config/nextest.toml`: JUnit output for `ci` profile →
  `target/nextest/ci/junit.xml` (`.config/nextest.toml:1`); uploaded by
  codecov-action `report_type: test_results`, flag `nextest`
  (`ci.yml:77-86`), skip logic for forks (`ci.yml:78`).
- Coverage: main pushes only (`ci.yml:65-75`), codecov-action with
  `secrets.CODECOV_TOKEN`, `files: lcov.info` (`ci.yml:69-75`);
  `codecov.yml` ignores `src/main.rs`, project gate 70%, patch gate 90%
  (`codecov.yml:1-10`).
- Env/gotchas:
  - CI env preset `ci.yml:19-31`; `test` (`ci.yml:44-50`) and `clippy`
    (`ci.yml:115-122`) jobs install mold; test job installs rust toolchain
    with `llvm-tools-preview` + cargo-llvm-cov + nextest (`ci.yml:51-57`).
  - Local: `.env` MUST exist (sourced by `scripts/test.sh:2`,
    `AGENTS.md:33-35`); databases via `#[sqlx::test]` per-test; `static/site.css`
    is committed and drift fails CI (`AGENTS.md:41-43`); `SQLX_OFFLINE=true`
    in Docker builds (`README.md:16-17`).
  - `concurrency` cancels in-progress same-ref CI runs (`ci.yml:33-35`).

## Cross-Cutting Observations
- The exact required-check context strings for `main` protection, if all five
  CI jobs are required, are `CI / Cargo CI Tests`, `CI / TODO and FIXME`,
  `CI / Rust-fmt (Cargo Format)`, `CI / Clippy (Cargo Clippy Lint Check)`,
  `CI / CSS Drift Check` (`ci.yml:38-39, 88-89, 99-100, 109-110, 128-129`).
- The repo's merge convention (rebase-only, no direct pushes) is documented in
  the HOME `~/.pi/agent/AGENTS.md` (lines 85-88), plus
  `merge-method: rebase` in `dependabot_auto_merge.yml:21` — no committed repo
  doc states it.
- `Dependabot Auto Merge / auto-merge` surfaces as a check on all PRs
  (`dependabot_auto_merge.yml:5-15`); GitHub auto-merge (used for dependabot)
  waits on required status checks + branch protection.
- Fly deploy keys off the whole-CI-workflow conclusion on `main`
  (`fly-deploy.yml:4-13`), not on individual checks or a protection gate.
- No repo precedent exists for repository-settings administration via script
  or API — all current automation is workflow-internal.

## Open Areas
- Whether `allowed_merge_methods`/`allow_merge_commit=false` is appropriate is
  a design decision; research only records that the documented convention is
  rebase-only merges (`~/.pi/agent/AGENTS.md:86-88`,
  `dependabot_auto_merge.yml:21`).
- No committed record of the CURRENT protection state (the GitHub-side
  settings were not readable from the repo; the ticket reports the protection
  GET currently 404s).
- Exact GitHub-native behavior of auto-merge when `required_linear_history`
  and `strict` status checks are enabled is GitHub-side behavior, not
  observable from this repo.