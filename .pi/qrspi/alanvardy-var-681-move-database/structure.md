# Structure Outline

## Approach
Pure config/docs relocation of the SQLite file from `data/vardy.db` to
root-level `test.db` (`DATABASE_URL=sqlite:test.db`). No Rust changes —
runtime is already path-agnostic. Slices follow the consumer chain:
local dev first (`.env` → `scripts/test.sh`), then build artifacts
(Dockerfile, `.gitignore`), then documentation.

Note on slicing: this ticket is small enough that each "layer" is a single
line edit; slices are grouped by *independent verification*, not by layer.
There is no database/service/API/UI stack to cross — the design explicitly
scopes out all `src/` changes.

---

## Phase 1: Local dev environment switch

Delivers the working change end-to-end: the database now lives at repo root
and the only script that touches the real file (`./scripts/test.sh`, via its
sourced `.env`) runs green against the new location.

**Files**: `.env` (untracked, update in-place), `.env_template`
**Key changes**:
- `DATABASE_URL=sqlite:data/vardy.db` → `DATABASE_URL=sqlite:test.db` — value edit, both files

**Verify**: `grep -n "data/vardy.db" .env .env_template` returns nothing;
`./scripts/test.sh` passes and refreshes `.sqlx/` against `sqlite:test.db`
(confirm with `ls test.db` appearing at repo root after the run).
Manual check: bare `cargo run` still boots and creates `test.db`.

---

## Phase 2: Build artifact + ignore rules

Makes Docker builds and git state consistent with the new path. Independently
valuable: image builds even if Phase 1's local `.env` were reverted, since
the Dockerfile carries its own default.

**Files**: `Dockerfile`, `.gitignore`
**Key changes**:
- `Dockerfile`: `ENV DATABASE_URL=sqlite:test.db`; remove `mkdir -p data` line
- `.gitignore`: `/data` → `test.db` (exact match per Decision 1)

**Verify**: `docker build .` succeeds (offline, `SQLX_OFFLINE=true`);
`git status --porcelain` shows no untracked `data/`; note whether
`test.db-wal`/`test.db-shm` appear as untracked during any local run
(known accepted risk).

---

## Phase 3: Documentation alignment

Updates every remaining reference and fixes the inaccurate "defaults to"
wording — the var is mandatory and panics when unset (`src/app/env.rs:29-34`);
it comes from `.env` / Docker `ENV`, never from code.

**Files**: `README.md`, `AGENTS.md`
**Key changes**:
- Replace `sqlite:data/vardy.db` references and "created on first boot /
  gitignored `data/` directory" phrasing with `test.db` at repo root,
  "set in `.env`" (not "defaults to")
- AGENTS.md command note about `DATABASE_URL` loaded from `.env` stays, new value

**Verify**: `grep -rn "data/vardy.db\|/data" --exclude-dir=.git --exclude-dir=.pi --exclude-dir=target .`
returns nothing live; read both docs for accuracy of the new wording.
Manual check: instructions in README work on a fresh clone
(`cp .env_template .env && ./scripts/test.sh`).

---

## Testing Checkpoints
- After Phase 1: `test.db` exists at repo root; `./scripts/test.sh` fully
  green; `.sqlx/` refreshed. This is the resume point if context resets —
  the change works locally from here on.
- After Phase 2: `docker build` green; gitignore matches; no `mkdir -p data`.
- After Phase 3: repo-wide grep clean; docs accurate; fresh-clone flow works.
- Final gate: full `./scripts/test.sh` re-run before commit (per project rules);
  commit via PR branch — never direct to main.
