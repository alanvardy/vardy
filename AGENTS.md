# Project Instructions

## Responses
- Keep responses concise
- Ask clarifying questions when instructions are unclear

## Skills
- Pi skills live in `~/.pi/agent/skills/<name>/SKILL.md`

## Module Layout

| Layer | Location | Examples |
|-------|----------|----------|
| Handlers (axum) | `src/interfaces/handlers/<domain>/` | `home/web.rs`, `singlethread/web.rs`, `dump/web.rs` |
| Routes | `src/interfaces/routes.rs` | `fn routes() -> Router<AppState>` |
| App state, db, templates, errors | `src/app/` | `state.rs`, `db.rs`, `templates.rs`, `error.rs` |
| Shared test helpers | `src/test/` | `mod.rs` (`start_app`, `test_client`) |

- New handlers go in `src/interfaces/handlers/` (either a new module or a
  file under an existing domain). Register the module in
  `src/interfaces/handlers/mod.rs`.
- Routes are defined in `src/interfaces/routes.rs`. Keep route definitions
  there — don't inline them in `main.rs` or on model structs.

## Tests
- Unit tests live inline at the bottom of each source file in `#[cfg(test)] mod tests`, not in separate files
- Happy and sad path tests need to be written
- Integration-style tests boot the real router via `start_app()` from
  `src/test/mod.rs` (in-memory SQLite, random port) and assert with
  `test_client()`
- `#[sqlx::test]` provisions a temporary per-test database and applies
  `migrations/` automatically

## Shell
- Pi skills: see `fish-shell` in `~/.pi/agent/skills/fish-shell/SKILL.md`

## Commands
- Run `./scripts/test.sh` to format, refresh sqlx offline metadata, type-
  check, lint, run tests, and grep for forgotten TODOs. It loads
  `DATABASE_URL` from `.env`, which must exist.
- The local database is SQLite at `sqlite:test.db`, set in `.env` (created on
  first boot; gitignored)
- Compile-time-checked query macros (`query!` etc.) need either a reachable
  `DATABASE_URL` or committed offline metadata: set `SQLX_OFFLINE=true` and
  refresh metadata with `cargo sqlx prepare` after schema changes

## Commits and PRs
- Code comments should describe what and why but not how
- `main` is the base branch when reviewing code
- Never push directly to main — all changes go through pull requests and are
  merged into main

## Migrations
- Always use `sqlx migrate add <name>` to create new migration SQL files — never manually create files in the `migrations/` directory

## Routes
- Any changes to routes or parameters needs to be updated in `ROUTES.md`
- In `ROUTES.md`, each endpoint section (`###` through closing `---`) is a
  self-contained block — use `---` as the cut point when making batch edits

## QRSPI Workflow
Features follow the QRSPI pipeline: `/1_spec` → `/2_clarify` → `/3_design` →
`/4_research` → `/5_plan` → `/6_implement`. Do not implement features outside
this flow. If a user requests a new feature after `/5_plan` but before
`/6_implement`, redirect to `/1_spec` to expand scope rather than
implementing directly.

## Error Responses
- All handler-produced error responses must go through `WebError`'s
  `IntoResponse` impl (`src/app/error.rs`) — never return bare status-code
  tuples from handlers.
- Tests must validate both the HTTP status and the response body; never
  assert status alone.
