# Research Questions

## Context
This is a Rust web application using axum and minijinja. Relevant areas include:
`static/site.css` and the other files under `static/`, the three HTML templates in
`templates/`, the asset fingerprinting/serving code under `src/app/assets.rs`,
`src/app/templates.rs`, and `src/interfaces/routes.rs`, plus the build/deploy
pipeline (`Dockerfile`, `scripts/`) and project convention docs (`AGENTS.md`,
prior design docs under `.pi/qrspi/`).

## Questions
1. How does static asset serving and cache invalidation currently work? Trace the
   full flow through `src/app/assets.rs` (`asset_url`, hash computation) and
   `src/interfaces/routes.rs` (`ServeDir` setup), including every test that
   asserts on `/static/site.css` behavior, headers, or fingerprints.
2. What exactly does `static/site.css` contain today? Inventory its `:root`
   custom properties/theme palette, every class definition grouped by which
   template uses it, hard-coded colors repeated outside `:root`, and all media
   queries/breakpoints.
3. How are templates loaded and rendered? Trace `src/app/templates.rs` (`init`,
   minijinja environment setup, custom functions like `asset_url`), how
   `layout.html` includes CSS, which semantic classes each of
   `home.html`/`singlethread.html` applies, and any inconsistencies in how
   images are referenced between templates.
4. How does the build and deploy pipeline work end to end? Describe each stage
   of the `Dockerfile`, when and how `static/`, `templates/`, and binaries move
   between stages, what `scripts/*.sh` do (especially `test.sh` gates), and
   whether any asset/CSS build tooling exists anywhere in the repo.
5. What project conventions and prior decisions constrain changes to static
   assets and styling? Summarize relevant rules in root `AGENTS.md`, anything in
   `ROUTES.md` touching static routes, and prior decisions recorded in
   `.pi/qrspi/*/**/design.md` (e.g. the var-664 homepage design doc) about
   build steps, asset handling, or visual design constraints.
