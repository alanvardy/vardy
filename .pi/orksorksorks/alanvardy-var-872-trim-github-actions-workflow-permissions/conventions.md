# Conventions

## Canonical Commands

### Build and Test
- `cargo test` — run all tests (local development database at `sqlite:test.db`)
- `cargo sqlx prepare` — refresh SQLx offline metadata (requires `DATABASE_URL` or committed metadata)
- `cargo sqlx migrate run` — apply pending migrations
- `cargo sqlx migrate revert` — revert last migration

### Lint and Format
- `cargo fmt` — format code
- `cargo fmt --all -- --check` — format check (fails on drift)
- `cargo clippy --all-targets --all-features --locked -- -D warnings` — clippy check
- `./scripts/lint_string.sh` — FIXME/fixme/dbg! scan
- `./scripts/build-css.sh` — build CSS
- `./scripts/test.sh` — full gate: format, sqlx metadata, type-check, lint, tests, TODO grep

### Verify
- `./scripts/test.sh` — full gate (format, sqlx, type-check, lint, tests, TODO grep)
- `git diff --exit-code -- static/site.css` — CSS drift check (after Tailwind build)
- `git diff --exit-code` — general drift check

### CI Gate
- `./scripts/branch-protection.sh apply` — enforce branch protection config

## Test Suite Inventory

### Inline Unit Tests
- `src/interfaces/handlers/.../*.rs` — unit tests at bottom of each handler file in `#[cfg(test)] mod tests`
- Assert against escaped HTML (`&#x27;`, `&#x2f;`) for rendered HTML

### Integration Tests
- `src/test/mod.rs` — `start_app()` boots real router (in-memory SQLite, random port)
- `test_client()` — integration test client
- Assert HTML on short unique substrings (e.g. `bg-black/50`), not full class strings

### SQLx Tests
- `#[sqlx::test]` — provisions temporary per-test database, applies migrations automatically
- Migrations in `migrations/` directory

## Build Gotchas

### Database
- Local database is SQLite at `sqlite:test.db` (set in `.env`, gitignored)
- Reset with `./scripts/reset_db.sh`
- `DATABASE_URL` must be set in `.env` for `./scripts/test.sh`

### SQLx Offline Mode
- Query macros (`query!`) need either reachable `DATABASE_URL` OR committed offline metadata
- Refresh offline metadata with `cargo sqlx prepare` after schema changes
- Set `SQLX_OFFLINE=true` for offline mode

### CSS Drift Check
- Tailwind build regenerates `static/site.css`
- Commit regenerated `static/site.css` in same change as any Tailwind class edit
- Run `./scripts/test.sh` after any CSS class changes

### Test Isolation
- `#[sqlx::test]` provisions a new database per test
- Run one test at a time for best results (per `scripts/test.sh` design)

## Permission Patterns (from research)

### Two-Level Pattern
- Workflow-level `permissions:` as baseline (ci.yml:27-30, ci-secure.yml:10-14)
- Job-level `permissions:` overrides that replace workflow-level for that job (ci.yml:131-132, ci-secure.yml:57-60)
- Omitted scopes at job-level default to `none`

### Unconsumed Scopes
- `actions: write` in ci.yml:30 — unused by any step
- `packages: read` in ci-secure.yml:13 — no consumer
- `contents: write` in dependabot_auto_merge.yml:11 — unreachable when `use-github-auto-merge: true`

### Pinning Conventions
- SHA pinning with version comment: `actions/checkout@3d3c42e... # v7.0.1` (ci.yml:42)
- Tag pinning at minor.patch: `github/codeql-action@v4.37.9` (ci-secure.yml:43)
- Floating major tags: `actions/checkout@v7`, `actions/cache@v6`
- Branch pinning: `superfly/flyctl-actions@master` (only branch-pinned action)

### Secret Usage
- Naming: `SCREAMING_SNAKE_CASE` with `_TOKEN` suffix
- Passing: `token: ${{ secrets.NAME }}` (action input) or `env: NAME: ${{ secrets.NAME }}`
- No `GITHUB_TOKEN` explicit references in workflows

## Documentation Gaps

- No documentation about permission minimization exists
- No comments about why certain permissions are declared
- Only one permission-related comment: ci-secure.yml:60 `# for upload-sarif in private repos`
