# Conventions — alanvardy/vardy

Shared factual appendix for Design/Structure/Plan. Focus on the CI/CD and
merge-tooling areas relevant to the auto-merge task, plus canonical local
commands.

## Canonical commands

- **Gate (local + CI semantics)**: `./scripts/test.sh` (scripts/test.sh). Loads `DATABASE_URL` from `.env` (`set -a; source .env`), then runs in order:
  1. `cargo fmt --all` (scripts/test.sh:12)
  2. `cargo sqlx prepare -- --tests` (scripts/test.sh:15) — refreshes `.sqlx` offline metadata; needs a reachable DB
  3. `cargo check --all-targets` (scripts/test.sh:17)
  4. `./scripts/build-css.sh` then `git diff --exit-code -- static/site.css` (scripts/test.sh:19-20) — CSS-drift check
  5. `cargo clippy --all-targets --all-features --locked -- -D warnings` (scripts/test.sh:22)
  6. `cargo nextest run` (scripts/test.sh:24)
  7. `! rg -i -s -g '*.rs' 'FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO\s|todo\s' src` (scripts/test.sh:26) — forgotten-TODO grep (requires ripgrep)
- CI equivalent is `ci.yml`: job names map to branch-protection REQUIRED_CONTEXTS (see below). `ci.yml` triggers on `push: [main]`, `pull_request`, `workflow_dispatch` (ci.yml:12-17).

### Build/verify gotchas
- `cargo sqlx prepare` needs a reachable `DATABASE_URL`; a blank/offline DB can delete committed `.sqlx/` metadata — restore with `git checkout -- .sqlx`, never commit the damage. `SQLX_OFFLINE=true` relies on committed metadata. (per repo AGENTS.md)
- `.env` and `test.db` are gitignored and absent in fresh worktrees — copy `.env` from the canonical clone, then `cargo sqlx database create && cargo sqlx migrate run` before `cargo sqlx prepare`.
- `DATABASE_URL` is `sqlite:test.db` in `.env`. Reset: `./scripts/reset_db.sh`.
- CSS: any Tailwind class edit must regenerate and commit `static/site.css` in the same change (drift check is a required CI context).
- CI uses mold linker via `RUSTFLAGS="-C link-arg=-fuse-ld=mold"` (ci.yml:21-23) and nextest (`--locked --profile ci`) (ci.yml:52-53).

## GitHub Actions layout (`.github/`)

- `workflows/ci.yml` — main CI; jobs most relevant to mergeability:
  - `test` `name: Cargo CI Tests` (ci.yml:36-37) — required context; PR path fast no-coverage via nextest (ci.yml:52-53), main path llvm-cov (ci.yml:55-57)
  - `todos` `name: TODO and FIXME` (ci.yml:89-90)
  - `fmt` `name: Rust-fmt (Cargo Format)` (ci.yml:100-101)
  - `clippy` `name: Clippy (Cargo Clippy Lint Check)` (ci.yml:110-111)
  - `css-drift` `name: CSS Drift Check` (ci.yml:129-130)
- `workflows/ci-secure.yml` — weekly `schedule:` only (ci-secure.yml:4); CodeQL + sarif jobs. **Not** PR gatekeepers (not in REQUIRED_CONTEXTS).
- `workflows/dependabot_auto_merge.yml` — `pull_request_target` + fastify v3 (subject of this task).
- `workflows/fly-deploy.yml` — `workflow_run: [CI completed, main]` (fly-deploy.yml:5-8); uses `secrets.FLY_API_TOKEN`.
- `workflows/rust-version-bump.yml` — `schedule`/`workflow_dispatch`; uses `secrets.MYTOKEN` only for peter-evans/create-pull-request (rust-version-bump.yml:49).
- `dependabot.yml` — cargo + github-actions, both daily, `assignees: [alanvardy]`, grouped `cargo-all`/`gha-all` all-types.
- `workflows/codeql/` — CodeQL config referenced by ci-secure.yml:44.

## Branch protection / merge settings (`scripts/branch-protection.sh`)

- Operations on `main`: default is `verify` (read-only); `apply [--yes]` PUTs and re-verifies (branch-protection.sh:207-236). Requires `gh` (repo/admin) and `jq` for apply.
- Enforces: strict status checks with `REQUIRED_CONTEXTS` (:17-24), linear history, 0 approving reviews, stale-review dismissal, rebase-only repo merges, no force push/deletion, `enforce_admins: false`.
- `REQUIRED_CONTEXTS` is the canonical source of truth — after any CI job `name:` change, update it and re-run `apply`. The `CI / ` check-name prefix is a UI artifact and NOT part of the check-run name (branch-protection.sh:14-16).
- **Does not verify or set the repository-level "Allow auto-merge" setting** — that toggle is GitHub Settings → General → Pull Requests, not represented in `branch-protection.sh` or anywhere in-repo.

## Test suite inventory

- Rust project; unit tests live inline at the bottom of each source file in `#[cfg(test)] mod tests`.
- Integration-style tests boot the router via `start_app()` from `src/test/mod.rs` (in-memory SQLite, random port) and assert with `test_client()`.
- `#[sqlx::test]` provisions a temporary per-test database and auto-applies `migrations/`.
- Rendered HTML is minijinja-autoescaped — tests assert escaped forms (`&#x27;`, `&#x2f;`) and short unique substrings, never raw strings or full `class="…"` values.
- Run via `cargo nextest run` (the gate) with `--profile ci` under CI.