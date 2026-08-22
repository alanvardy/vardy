# Implementation Plan — VAR-681: Move database

## Overview
Relocate the SQLite database file from `data/vardy.db` to a root-level
`test.db`, reached via `DATABASE_URL=sqlite:test.db`, updating every config
and doc reference. No Rust changes — runtime is already path-agnostic.

---

## Phase 1: Local dev environment switch

### Changes

#### 1. Local env file (untracked, update in place)
**File**: `.env`
**Action**: modify

```dotenv
DATABASE_URL=sqlite:test.db
```

(Line 1 only; leave `UNSPLASH_API_KEY` untouched.)

#### 2. Env template
**File**: `.env_template`
**Action**: modify

```dotenv
DATABASE_URL=sqlite:test.db
```

(Line 7 only; header comment block and other vars stay as-is.)

No script changes needed: `scripts/test.sh:2-3` sources `.env` unchanged and
line 8 (`cargo sqlx prepare -- --tests`) will connect to the new file.

### Verification
#### Automated
- [x] `grep -n "data/vardy.db" .env .env_template` returns nothing
- [x] `./scripts/test.sh` passes end-to-end (format, sqlx prepare, check,
      clippy, nextest, TODO grep) and refreshes `.sqlx/`
- [x] `ls test.db` shows the file at repo root after the run; no `data/`
      directory created (`test data/vardy.db` fails)

#### Manual
- [ ] `cargo run` boots successfully and creates/uses `test.db` at repo root

---

## Phase 2: Build artifact + ignore rules

### Changes

#### 1. Docker image default
**File**: `Dockerfile`
**Action**: modify

Replace line 31 and remove line 32:

```dockerfile
ENV DATABASE_URL=sqlite:test.db
RUN sqlx database create
RUN sqlx migrate run
```

(`mkdir -p data` is deleted — the file now lands at `/app/test.db` in the
runtime stage, whose parent exists by definition. `SQLX_OFFLINE=true` at
line 15 stays.)

#### 2. Ignore rules
**File**: `.gitignore`
**Action**: modify

```
/target
test.db
.env
```

(Exact match per design Decision 1 — replaces `/data`. Known accepted risk:
`test.db-wal` / `test.db-shm` sidecars from WAL mode may appear untracked;
widen to `test.db*` later if it annoys.)

### Verification
#### Automated
- [x] `docker build .` succeeds (offline build via `SQLX_OFFLINE=true`)
- [x] `git status --porcelain` shows no untracked `data/` entries
- [x] `grep -n "data" Dockerfile .gitignore` returns nothing
      (adapted to `grep -nE "data/vardy\.db|/data|mkdir -p data"` — the
      literal check false-positives on `RUN sqlx database create`)

#### Manual
- [ ] Note during any local run whether `test.db-wal` / `test.db-shm` show up
      as untracked in `git status` — expected, accepted risk, no action

---

## Phase 3: Documentation alignment

Fixes both the path references and the inaccurate "defaults to" wording —
the var is mandatory (`src/app/env.rs:29-34` panics when unset); its value
comes from `.env` / Docker `ENV`, never from code.

### Changes

#### 1. README development section
**File**: `README.md`
**Action**: modify

Replace lines 7–8:

```markdown
- `DATABASE_URL` is set in `.env` (the database file `test.db` lives at the
  repository root and is gitignored).
```

(Retains the surrounding bullets — migrations, tests, sqlx prepare — which
have no path references.)

#### 2. Project instructions
**File**: `AGENTS.md`
**Action**: modify

Replace lines 41–42 under **Commands**:

```markdown
- The local database is SQLite at `sqlite:test.db`, set in `.env` (created on
  first boot; gitignored)
```

(The adjacent note that `./scripts/test.sh` loads `DATABASE_URL` from `.env`,
which must exist, stays accurate and untouched.)

### Verification
#### Automated
- [ ] `grep -rn "data/vardy.db\|/data" --exclude-dir=.git --exclude-dir=.pi --exclude-dir=target .` returns nothing live
- [ ] Final gate before commit: full `./scripts/test.sh` re-run passes

#### Manual
- [ ] Read README.md and AGENTS.md — new wording says "set in `.env`", not
      "defaults to"; no remaining mention of a `data/` directory
- [ ] Fresh-clone flow works: `cp .env_template .env && ./scripts/test.sh`

---

## Testing Checkpoints
- After Phase 1: `test.db` exists at repo root; `./scripts/test.sh` fully
  green; `.sqlx/` refreshed. Resume point if context resets.
- After Phase 2: `docker build` green; gitignore matches; no `mkdir -p data`.
- After Phase 3: repo-wide grep clean; docs accurate; fresh-clone flow works.
- Commit via PR branch — never direct to main.

## Notes for Implementer
- No schema migrations, codegen, or `src/` changes are involved; nothing to
  add to `ROUTES.md`.
- If `docker build` fails locally due to network/toolchain constraints, fall
  back to verifying the Dockerfile edit by inspection plus the grep checks;
  flag it in the PR description.
