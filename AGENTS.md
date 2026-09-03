# Project Instructions

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
- **Rendered HTML is minijinja-autoescaped** — assert against escaped forms
  (`&#x27;` for `'`, `&#x2f;` for `/`) in HTML assertions, not raw strings
- Integration-style tests boot the real router via `start_app()` from
  `src/test/mod.rs` (in-memory SQLite, random port) and assert with
  `test_client()`
- `#[sqlx::test]` provisions a temporary per-test database and applies
  `migrations/` automatically
- Assert rendered HTML on short unique substrings (e.g. `bg-black/50`),
  never a full `class="…"` string or the head of a class list — those can
  never match exactly.

## Commands
- Run `./scripts/test.sh` to format, refresh sqlx offline metadata, type-
  check, lint, run tests, and grep for forgotten TODOs. It loads
  `DATABASE_URL` from `.env`, which must exist.
- The local database is SQLite at `sqlite:test.db`, set in `.env` (created on
  first boot; gitignored). To reset: `./scripts/reset_db.sh`.
- Compile-time-checked query macros (`query!` etc.) need either a reachable
  `DATABASE_URL` or committed offline metadata: set `SQLX_OFFLINE=true` and
  refresh metadata with `cargo sqlx prepare` after schema changes.
- `scripts/test.sh` includes a CSS-drift check (`git diff --exit-code --
  static/site.css`) — regenerate and commit `static/site.css` in the SAME
  change as any Tailwind class edit; if the check fails, run the Tailwind
  build step, commit the regenerated file, then re-run the gate.

## Commits and PRs
- `main` is the base branch when reviewing code
- If a session resumes onto a branch with uncommitted changes, treat them
  as suspect (orphans from an interrupted session) — compare against
  `plan.md` and the Linear ticket before keeping or reverting

## Routes
- Any changes to routes or parameters needs to be updated in `ROUTES.md`
- In `ROUTES.md`, each endpoint section (`###` through closing `---`) is a
  self-contained block — use `---` as the cut point when making batch edits

## QRSPI Workflow
Follow the QRSPI pipeline (`/1_spec` → `/6_implement`) — see the `qrspi`
skill; don't implement outside it; scope changes after `/5_plan` go back to
`/1_spec`.

## Error Responses
- All handler errors go through `WebError`'s `IntoResponse` impl
  (`src/app/error.rs`). Error handling chokepoint policy: see `~/.pi/agent/AGENTS.md` (`## Errors`).
