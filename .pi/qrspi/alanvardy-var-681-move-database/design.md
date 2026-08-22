# Design — VAR-681: Move database

## Current State
The SQLite database file lives at `data/vardy.db`, referenced in five places —
all config/docs, none in Rust code (research Q2):

| Location | Contents |
|---|---|
| `.gitignore:2` | `/data` (ignores the whole directory); `.gitignore:3` ignores `.env` |
| `.env:1` (untracked) | `DATABASE_URL=sqlite:data/vardy.db` |
| `.env_template:7` | `DATABASE_URL=sqlite:data/vardy.db` |
| `Dockerfile:31-34` | `ENV DATABASE_URL=sqlite:data/vardy.db`, `mkdir -p data`, `sqlx database create`, `sqlx migrate run` |
| `README.md:7-8`, `AGENTS.md:41-42` | Docs describing the path and the `data/` directory |

Runtime is path-agnostic: `DATABASE_URL` is read mandatorily via
`Env::init()` (`src/app/env.rs:16`, panics if unset at `src/app/env.rs:29-34`)
and passed to `db::init()` (`src/main.rs:25`), which parses the URL, enables
WAL (`src/app/db.rs:12`), and creates the file's parent directory generically
(`src/app/db.rs:14-21`). No hardcoded path anywhere in `src/`.

Only one consumer touches the actual file: `scripts/test.sh:2-3` sources
`.env` and `scripts/test.sh:8` runs `cargo sqlx prepare -- --tests`, which
connects to the configured DB to refresh `.sqlx/` offline metadata. Tests are
independent (`src/test/mod.rs:17-24` hardcodes `sqlite::memory:`;
`#[sqlx::test]` sites provision temp DBs).

Target convention from the sibling `../api` repo: DB file directly at repo
root named `test.db`, `DATABASE_URL=sqlite:test.db` (research Q3). That repo
has since moved to Postgres, so only the placement convention applies.

## Desired End State
- Database file is `test.db` at the repository root, reached via
  `DATABASE_URL=sqlite:test.db`.
- Every reference to `data/vardy.db` / `data/` is updated or removed:
  `.env`, `.env_template`, `.gitignore`, `Dockerfile`, `README.md`,
  `AGENTS.md`.
- `./scripts/test.sh` works unchanged against the new location.
- Docker image builds and runs with the new URL; no `mkdir -p data` needed.

Verification:
- `grep -r "data/vardy.db\|/data"` across repo returns no live references.
- `./scripts/test.sh` passes (it sources `.env` and refreshes `.sqlx/`).
- `cargo sqlx prepare` succeeds against `sqlite:test.db`.
- `docker build` succeeds (offline build; `SQLX_OFFLINE=true` at
  `Dockerfile:15`).

## Patterns to Follow
- **Sibling-repo placement**: root-level `test.db`, `sqlite:test.db` URL —
  matches `../api` convention the ticket asks for (research Q3).
- **Env plumbing stays as-is**: keep mandatory `DATABASE_URL` via
  `Env::init()` (`src/app/env.rs:16`) and panic-on-missing semantics; do not
  add a dotenv crate or code-level default — that's a separate concern.
- **Generic parent-dir creation stays**: `src/app/db.rs:14-21` is
  path-agnostic and useful for other deployments; don't special-case the new
  path or delete the logic.
- **`.env` sourced only by `scripts/test.sh`** (`scripts/test.sh:2-3`) —
  keep that mechanism; just update the value.

Patterns NOT to follow:
- `../api`'s `test.db*` wildcard gitignore (research Q3) — we chose an exact
  match (Decision 1).
- `../api`'s stale SQLite docs after its Postgres migration — a reminder to
  update all doc references in the same change, not leave stragglers.

## Design Decisions
1. **Gitignore pattern**: `test.db` exact match (replacing `/data`) — per
   user choice. Simpler; the WAL-sidecar risk is accepted (see Open Risks).
2. **Dockerfile**: change `DATABASE_URL` to `sqlite:test.db` and drop
   `mkdir -p data` — the file now lands at root, so the directory creation
   is unnecessary; `sqlx database create` / `sqlx migrate run` stay.
3. **Docs**: update `README.md` and `AGENTS.md` to the new path AND fix the
   inaccurate "defaults to" wording — the value comes from `.env` /
   `.env_template` / Docker `ENV`, not from code (`src/app/env.rs:29-34`
   panics when unset). Say "set in `.env`" instead of "defaults to".
4. **Local `.env`**: update the untracked `.env` in the same change so the
   next `./scripts/test.sh` run works without manual steps.
5. **Scope of edits**: config/docs only. No changes to `src/`, `fly.toml`,
   `ROUTES.md`, scripts, CI, or `migrations/` — research confirmed they have
   no references (research Q2/Q4).

## What We're NOT Doing
- No code-level default for `DATABASE_URL` and no dotenv loader (pre-existing
  gap noted in research "Open Areas"; separate ticket if wanted).
- No Fly volume/mount for persistence — `fly.toml` stays untouched; the
  container DB remains ephemeral across deploys (status quo preserved).
- No migration of existing `data/vardy.db` data — the file doesn't exist
  locally and there's no production data at stake.
- No cleanup of the sibling `../api` repo's stale artifacts.
- No renaming of `.sqlx/` offline metadata or test harness — both are
  path-independent.

## Open Risks
- **WAL sidecars**: with an exact `test.db` gitignore entry, `test.db-wal`
  and `test.db-shm` (created because WAL is enabled, `src/app/db.rs:12`)
  would show as untracked. If that annoys, widen the entry to `test.db*`
  later — one-line change.
- **Stale `data/` on other machines**: any checkout that already created
  `data/vardy.db` will keep the old file; devs must move or delete it
  manually. Nothing in the repo can enforce this.
- **Docker `sqlx database create` semantics**: creating `sqlite:test.db` at
  build time bakes an empty migrated DB into the image layer; unchanged
  behavior from today, but worth confirming the image still builds.
