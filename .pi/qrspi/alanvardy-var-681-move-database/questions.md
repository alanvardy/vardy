# Research Questions

## Context
Focus on how the SQLite database file location is configured and consumed in
this Rust/axum web service, where that path appears across config, Docker,
scripts, and documentation, and what convention the sibling `../api`
repository follows for its database file placement.

## Questions
1. How does the database connection get established at runtime — trace the
   flow of `DATABASE_URL` from environment loading (`src/app/env.rs`) through
   `app::db::init` (`src/app/db.rs`) into application state, including any
   logic that creates parent directories or handles special filenames?
2. Where across the repository does the path `data/vardy.db` or the `data/`
   directory appear (`.env`, `.env_template`, `.gitignore`, `Dockerfile`,
   `fly.toml`, `README.md`, `AGENTS.md`, scripts), and what does each
   occurrence do with it?
3. In the sibling `../api` repository, where is the SQLite database file
   located relative to the repo root, how is it named, and how do that repo's
   `.gitignore`, env files, and Dockerfile treat it?
4. Which tests and developer scripts depend on the database file's location
   or on `DATABASE_URL` being loaded from `.env` (e.g. `scripts/test.sh`,
   sqlx offline metadata / `cargo sqlx prepare` usage, `#[sqlx::test]`
   provisioning)?
