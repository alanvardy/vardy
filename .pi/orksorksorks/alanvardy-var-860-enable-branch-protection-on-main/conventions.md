# Conventions — shared factual appendix

## Canonical commands (from `scripts/` + CI)

| Action | Command | Where |
|--------|---------|-------|
| Full local gate (fmt, sqlx prepare, check, css-drift, clippy, nextest, TODO/FIXME grep) | `./scripts/test.sh` (loads `.env`, must exist) | `scripts/test.sh:1-22`, `AGENTS.md:33-35` |
| Reset local DB | `./scripts/reset_db.sh` (removes `test.db`) | `scripts/reset_db.sh:4-11`, `.gitignore:2` |
| Rebuild committed CSS | `./scripts/build-css.sh` (pinned Tailwind v4.3.3 + sha256) | `scripts/build-css.sh:6-28` |
| Refresh sqlx offline metadata after schema changes | `cargo sqlx prepare` (in test.sh:7 as `-- --tests`); set `SQLX_OFFLINE=true` to compile without DB | `README.md:14-16`, `AGENTS.md:38-40` |
| TODO/FIXME lint (CI variant) | `./scripts/lint_string.sh <pattern>` ×5 | `scripts/lint_string.sh:4-8`, `ci.yml:93-97` |
| CI-equivalent commands (CI never calls test.sh) | `cargo fmt --all -- --check` (`ci.yml:107`); `cargo clippy --all-targets --all-features --locked -- -D warnings` (`ci.yml:126`); `cargo nextest run --profile ci` (`ci.yml:62`) | `.github/workflows/ci.yml` |
| Merge to main | `gh pr merge <n> --rebase --delete-branch` (convention lives in HOME `~/.pi/agent/AGENTS.md:85-89`, not committed) | home AGENTS.md |
| CI triggers | `push`→`main`, `pull_request` (all), `workflow_dispatch`, cancel-in-progress concurrency | `ci.yml:12-17, 33-35` |

## CI check-run inventory (for branch protection contexts)

Required-check candidates (contexts as GitHub renders them, workflow name +
job `name:`):
- `CI / Cargo CI Tests` — `ci.yml:38-39` (PR fast path `:60-61`; main coverage path `:65-66`; codecov/JUnit uploads `:69-86`, not separate checks)
- `CI / TODO and FIXME` — `ci.yml:88-89`
- `CI / Rust-fmt (Cargo Format)` — `ci.yml:99-100`
- `CI / Clippy (Cargo Clippy Lint Check)` — `ci.yml:109-110`
- `CI / CSS Drift Check` — `ci.yml:128-129`
- `Dependabot Auto Merge / auto-merge` — ALSO surfaces on every PR (`dependabot_auto_merge.yml:5-15`)

Not PR checks (schedule/workflow_run only): `CodeQL Analysis (Weekly) / CodeQL (*)` + `/ Run rust-clippy analyzing` (`ci-secure.yml:17-19, 54-55`); `Update Rust toolchain / Bump rust-toolchain.toml` (`rust-version-bump.yml:21-22`); `Fly Deploy / Deploy app` (`fly-deploy.yml:10-11`).

## Test-suite inventory

- Unit tests: inline `#[cfg(test)] mod tests` at the bottom of each source
  file (project AGENTS.md "Tests"). Happy + sad paths expected.
- Integration tests: boot the real router via `start_app()` /
  `test_client()` from `src/test/mod.rs` (in-memory SQLite, random port);
  `#[sqlx::test]` provisions a temporary per-test DB and applies
  `migrations/` automatically.
- HTML assertions: rendered HTML is minijinja-autoescaped — assert against
  escaped forms (`&#x27;`, `&#x2f;`) and on short unique substrings, never a
  full class list (project AGENTS.md "Tests").
- JUnit: `.config/nextest.toml:1` emits `ci`-profile JUnit → codecov upload;
  coverage gates `codecov.yml:1-10` (project 70%, patch 90%, ignore
  `src/main.rs`).
- Run via `cargo nextest run` / `./scripts/test.sh:17`; not `cargo test`.

## Build/verify gotchas

- `static/site.css` is a COMMITTED artifact: any Tailwind class edit must
  regenerate and commit it in the SAME change, or the CSS-drift check fails
  (`scripts/test.sh:11-13`, `ci.yml:130-145`, `AGENTS.md:41-43`). CI caches
  the Tailwind binary keyed on `hashFiles('scripts/build-css.sh')`
  (`ci.yml:118-124`).
- `.env` must exist locally (sourced by `scripts/test.sh:2`); `.env_template`
  documents the 7 vars + 4 sync locations (`.env`, template, fly.io, 1Password)
  (`.env_template:1-11`).
- CI jobs install mold (`ci.yml:44-50, 115-122`); toolchain pinned by
  `rust-toolchain.toml` (channel + clippy/rustfmt components), bumped only via
  scheduled PR workflow.
- No `gh` CLI or GitHub REST API usage exists anywhere in the repo
  (scripts/workflows); repository-settings administration has no precedent.
- Secrets in use: `CODECOV_TOKEN`, `FLY_API_TOKEN`, `MYTOKEN` (dedicated PAT
  for PR-creation workflow). No `GITHUB_TOKEN` references.
- Git editor `hx` panics without a TTY: always `git commit -m "..."`;
  `git -c core.editor=true rebase --continue` (home AGENTS.md:90-93).
- Branch protection: currently none configured; rebase-only merge convention
  documented in home AGENTS.md (lines 85-89) + `merge-method: rebase` at
  `dependabot_auto_merge.yml:21`.