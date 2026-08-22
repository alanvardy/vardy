# Research Findings

Branch: `alanvardy-var-681-move-database`

## Q1: How does the database connection get established at runtime?

### Findings
- `Env` struct holds `pub database_url: String` — `src/app/env.rs:5-11`.
- `Env::init()` reads it via `get_string_env("DATABASE_URL")` — `src/app/env.rs:16`.
- `get_string_env` panics if the var is unset or empty (`"{key} must be set and non-empty"`) — `src/app/env.rs:29-34`.
- **No dotenv loader exists**: module doc says "Set them locally in `.env`" (`src/app/env.rs:3`), but no `dotenv`/`dotenvy` crate is in `Cargo.toml` or called anywhere in `src/`. Env vars must be exported by the shell, sourced by scripts, or set via `fly secrets`.
- Call site: `main()` calls `Env::init()` at `src/main.rs:17`, then builds state with `db: app::db::init(&env.database_url).await` at `src/main.rs:25`, passing to axum via `.with_state(state)` at `src/main.rs:39`.
- `AppState` holds `pub db: sqlx::SqlitePool` — `src/app/state.rs:9`.
- `pub async fn init(database_url: &str) -> SqlitePool` — `src/app/db.rs:7-28`:
  - Parses URL into `SqliteConnectOptions::from_str(...).expect("invalid DATABASE_URL")` with `create_if_missing(true)`, `foreign_keys(true)`, `journal_mode(Wal)` — `src/app/db.rs:8-12`.
  - **Parent-directory creation** at `src/app/db.rs:14-21`: comment explains `create_if_missing` creates the file but not its parent ("a fresh checkout would fail to boot without this"); skips creation when filename is literally `":memory:"`, otherwise `std::fs::create_dir_all(parent).expect("failed to create database directory")`.
  - Pool: `max_connections(5)`, `.connect_with(options).await.expect("failed to connect to database")` — `src/app/db.rs:23-27`.
- All failures are panic-based (`.expect` / `panic!`) rather than `Result` propagation.
- Tests confirming behavior:
  - `init_creates_database_file_and_parent_directory` builds `sqlite://<tmp>/sub/db.sqlite`, asserts the file exists after `init` — `src/app/db.rs:46-55`.
  - `migrations_applied` verifies migrations run on the pool — `src/app/db.rs:35-43`.

Flow summary: `DATABASE_URL` env → `get_string_env` (panic if unset) → `Env.database_url` → `db::init` → parse + WAL + create parents (unless `:memory:`) → `SqlitePool(5)` → `AppState.db`.

## Q2: Where does `data/vardy.db` / `data/` appear across the repository?

### Findings
| Location | What it does |
|---|---|
| `.gitignore:2` | `/data` — ignores the whole directory |
| `.gitignore:3` | `.env` — ignored (contains the path too) |
| `.env:1` | `DATABASE_URL=sqlite:data/vardy.db` (untracked local file) |
| `.env_template:7` | `DATABASE_URL=sqlite:data/vardy.db` — copied to `.env` per `README.md:5` |
| `Dockerfile:31` | `ENV DATABASE_URL=sqlite:data/vardy.db` baked into image |
| `Dockerfile:32-34` | `mkdir -p data`, `sqlx database create`, `sqlx migrate run` at image build |
| `README.md:7-8` | Docs: "`DATABASE_URL` defaults to `sqlite:data/vardy.db` (the `data/` directory is created on first boot and gitignored)" |
| `AGENTS.md:41-42` | Docs: "The local database is SQLite at `sqlite:data/vardy.db` (created on first boot; the `data/` directory is created automatically)" |

Files with **no** occurrences: `fly.toml` (no `[mounts]`, no volume), `ROUTES.md`, `Cargo.toml`, `scripts/*`, all `.github/workflows/*`, `.dockerignore`, and all of `src/` (the code has no hardcoded path — parent-dir creation in `src/app/db.rs:16-21` is generic).

On-disk/git state:
- `data/vardy.db` does not exist locally; `data/` does not exist either.
- `git check-ignore -v data/` matches `.gitignore:2:/data`; `git ls-files` has no `data/` or `.db` entries.
- Note: `README.md:7` says DATABASE_URL "defaults to" that value, but there is no code-level default — `src/app/env.rs:29-34` panics when unset; the effective default comes only from `.env`/`.env_template` or Docker's `ENV`.

## Q3: Sibling `../api` repo — SQLite file placement convention

### Findings
Repo `/Users/vardy/dev/api` exists (Rust/axum/sqlx).
- Database file was **`test.db` at the repo root** (`/Users/vardy/dev/api/test.db`, 76 KB SQLite 3.x file).
- `.gitignore:2` ignores `test.db*` (wildcard covers `-shm`/`-wal` siblings); `.gitignore:3` ignores `.env`.
- The file is untracked today but historically committed (`git log -- test.db`: `44fa2df "add sqlite"`, `73a8f09`).
- That repo has since **migrated to Postgres**: `.env:7` and `.env_template:7` now hold `postgres://...` URLs; `Cargo.toml:22` sqlx features are postgres-only; `src/infra/db.rs:1-8` returns a `PgPool`; `Dockerfile:9` installs postgres-only sqlx-cli; `scripts/db_proxy.sh` proxies fly Postgres.
- So `test.db` is an orphaned artifact of its earlier SQLite setup, where `DATABASE_URL=sqlite:test.db` pointed at a root-level file (historical notes: `.pi/qrspi/VAR-539-switch-to-postgres/research.md:68-71`, `.research/local.md:55`).
- Stale SQLite wording survives there in `CONTRIBUTING.md:31`, `src/domain/ids.rs:5`, and `.sqruff:2` (`dialect = sqlite`).

Convention observed in `../api`: DB file lived directly at the repo root (`test.db`), not under a subdirectory.

## Q4: Tests and scripts depending on DB location / `.env`

### Findings
- `scripts/test.sh:2-3` — `set -a; source .env; set +a` is the only place `.env` enters the environment. Line 8 runs `cargo sqlx prepare -- --tests`, which needs a live `DATABASE_URL` (i.e., connects to `sqlite:data/vardy.db`) to refresh `.sqlx/` offline metadata. This is the sole consumer of the actual db file among scripts.
- `.sqlx/` contains two cached queries backing the compile-time-checked macros in `src/interfaces/handlers/dump/web.rs:19` (`query_as!`) and `:35` (`query!`). Builds without a live `DATABASE_URL` depend on this directory.
- `Dockerfile:15` sets `ENV SQLX_OFFLINE=true` before `cargo build` (`Dockerfile:16`) — image builds fully offline. CI workflows set neither `SQLX_OFFLINE` nor `DATABASE_URL`; macro compilation relies on committed `.sqlx/`.
- `#[sqlx::test]` sites (provision their own temp DB + migrations, independent of `DATABASE_URL`):
  - `src/app/db.rs:35` (`migrations_applied`)
  - `src/interfaces/handlers/unsplash/json.rs:40`, `:51`
- Integration-test harness never touches `data/vardy.db` or `.env`: `src/test/mod.rs:17-24` hardcodes `database_url: "sqlite::memory:"` (again at `src/test/mod.rs:50-58` for `start_app_with_metrics`), then calls `crate::app::db::init` + `sqlx::migrate!("./migrations").run(&db)` (`src/test/mod.rs:25-28`). All integration tests go through `start_app*` (`src/interfaces/routes.rs:45,97,114,126,143`; handler tests in `home/web.rs:23`, `dump/web.rs`, `unsplash/json.rs`).
- `scripts/lint_string.sh` has no DB dependency.

Dependency matrix:

| Consumer | Needs `.env` `DATABASE_URL` | Touches `data/vardy.db` |
|---|---|---|
| `scripts/test.sh:3,8` (`cargo sqlx prepare`) | Yes (sources `.env`) | Yes (connection target) |
| Compile-time macros + committed `.sqlx/` | Only when `.sqlx` stale/absent | No |
| `#[sqlx::test]` tests (3 sites) | No | No (temp DBs) |
| `start_app*` harness (`src/test/mod.rs`) | No (`sqlite::memory:` hardcoded) | No |

## Cross-Cutting Observations
- The literal path `data/vardy.db` exists only in config/docs (`.env*`, `Dockerfile:31`, `README.md:7`, `AGENTS.md:41`); Rust code is path-agnostic except the generic parent-directory creation in `src/app/db.rs:14-21`.
- Runtime defaulting happens at three layers, not in code: shell export, `.env` sourced by `scripts/test.sh`, and Docker `ENV` (`Dockerfile:31`); missing var panics at startup (`src/app/env.rs:33`).
- `fly.toml` has no volume/mount, so the container-local `data/` from `Dockerfile:32` is ephemeral across deploys as currently configured.
- The sibling `../api` convention was a root-level `test.db` (now orphaned after its Postgres migration); this repo instead uses a gitignored `data/` subdirectory.
- Both repos share near-identical env plumbing: mandatory `DATABASE_URL` read in `app::env`, pool init in `app::db` (or `infra::db`), panic-on-missing semantics.

## Open Areas
- No code-level default for `DATABASE_URL` exists despite README/AGENTS wording saying it "defaults" — behavior differs between a bare `cargo run` (panic) and Docker/.env-sourced runs.
- Whether Fly deployment intends persistent storage cannot be determined from `fly.toml` (no `[mounts]` section present).
