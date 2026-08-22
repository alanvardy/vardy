# vardy

## Development

cp .env_template .env   # or export the vars manually

- `DATABASE_URL` is set in `.env` (the database file `test.db` lives at the
  repository root and is gitignored).
- Run migrations locally: `sqlx migrate run` (requires
  `cargo install sqlx-cli --no-default-features --features sqlite`).
- Tests: `cargo nextest run`. Tests use `#[sqlx::test]`, which provisions a
  temporary per-test database and applies `migrations/` automatically.
- Compile-time-checked query macros (`query!` etc.) need either a reachable
  `DATABASE_URL` or committed offline metadata: set `SQLX_OFFLINE=true` and
  refresh metadata with `cargo sqlx prepare` after schema changes. The
  `.sqlx/` directory is committed so Docker builds (`SQLX_OFFLINE=true`)
  compile without a live database.
