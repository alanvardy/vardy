# Research Findings

## Q1: Module tree declaration and wiring

### Findings
- Binary-only crate: `name = "vardy"`, edition 2024 (`Cargo.toml:2-3`). No `lib.rs`, no top-level `tests/` directory — everything compiles under the binary root `src/main.rs`; all paths are `crate::<module>`.
- Root declares modules at `src/main.rs:3-6`: `mod app; mod domain; mod infra; mod interfaces;`. Shared test module is gated: `#[cfg(test)]` (src/main.rs:48) + `mod test;` (src/main.rs:49).
- `src/app/mod.rs:1-8`: `assets, db, env, error, log, picture, state, templates`.
- `src/domain/mod.rs` is empty (single newline) — a placeholder module with zero code, referenced by nothing.
- `src/infra/mod.rs:1-3`: `metrics, sentry, unsplash`. Note: an orphan empty file `src/infra/db.rs` exists but is **not declared** in `infra/mod.rs` and is not compiled.
- `src/interfaces/mod.rs:1-2`: `handlers, routes`. `src/interfaces/handlers/mod.rs:1-5`: `dump, home, metrics, singlethread, unsplash`. Each handler group's `mod.rs` declares one leaf: `pub mod web;` (dump/home/metrics/singlethread) or `pub mod json;` (unsplash).
- `src/test/mod.rs` (only under cfg(test)) exposes helpers reachable as `crate::test::*`: `start_app` (:11-13), `start_app_with` (:17-47), `start_app_with_metrics` (:50-84), `test_client` (:87-89), `UnsplashStub`/`start_unsplash_stub` (:92-133), plus its own test `page_hits_show_up_in_metrics` (:135-160).

## Q2: External crate imports per module (prod vs test)

### Findings
Declared deps (`Cargo.toml:6-20`): axum, minijinja, prometheus, serde, serde_json, sentry, sha2, sqlx (sqlite/tokio/chrono/migrate/json), tokio, tower-http, tracing, tracing-subscriber, reqwest, chrono. Dev-dep (`Cargo.toml:22-23`): `tower = "0.5"`.

Production-code imports of note:
- **app**: axum in `src/app/error.rs:1-4` (`IntoResponse for WebError` impl at :29); tower-http + axum extractors in `src/app/log.rs:3-9`; sqlx in `src/app/db.rs:1-4`, `src/app/picture.rs:2,6,14,23`, `src/app/state.rs:8`; sentry called from `src/app/error.rs:35,40`; sha2 in `src/app/assets.rs:1`; minijinja in `src/app/templates.rs:3-14`, `src/app/state.rs:7`.
- **infra**: reqwest in `src/infra/unsplash.rs:2`; prometheus in `src/infra/metrics.rs:1`; sentry fully qualified in `src/infra/sentry.rs:2-5`.
- **interfaces**: axum + tower_http in `src/interfaces/routes.rs:1-6`; prometheus in handlers/metrics/web.rs:2; **sqlx macros directly in handler code** — `sqlx::query_as!` at `src/interfaces/handlers/dump/web.rs:19`, `sqlx::query!` at :35; **reqwest constructed in handler** — `reqwest::Client::new()` at `src/interfaces/handlers/unsplash/json.rs:16`; chrono at json.rs:6.
- Files with **no** `#[cfg(test)]` section (all imports production): `src/app/log.rs`, `src/app/picture.rs`, `src/app/state.rs`, `src/infra/{unsplash,sentry}.rs`, `src/interfaces/handlers/metrics/web.rs`.

Test-gated imports:
- dev-dep `tower` used exactly once: `use tower::ServiceExt;` inside `#[cfg(test)]` in `src/interfaces/routes.rs:66`.
- Every handler test block imports `crate::test::{...}` helpers (e.g. `src/interfaces/routes.rs:40`, `home/web.rs:18`, `dump/web.rs:47`, `singlethread/web.rs:18`, `unsplash/json.rs:38`).
- All of `src/test/mod.rs` is test-gated via src/main.rs:48-49; its imports (axum, serde_json, sqlx, reqwest, tokio) sit at `src/test/mod.rs:1-6`.

## Q3: Intra-crate dependency map

### Findings
Pairwise direction (→ = depends on), production edges unless noted:

| Source ↓ / Target → | app | domain | infra | interfaces | test |
|---|---|---|---|---|---|
| **app** | intra (state.rs:3, templates.rs:1) | ❌ | ✅ `state.rs:10` (`AppState.metrics: Arc<crate::infra::metrics::AppMetrics>`) | ❌ | ❌ |
| **domain** | ❌ | — | ❌ | ❌ | ❌ (empty module, zero traffic) |
| **infra** | ✅ `unsplash.rs:1` (`crate::app::{error::WebError, picture::Picture}`) | ❌ | — | ❌ | ❌ |
| **interfaces** | ✅ routes.rs:8; home/web.rs:4-5; dump/web.rs:1-2; singlethread/web.rs:4-5; unsplash/json.rs:1-3 | ❌ | ✅ routes.rs:32; metrics/web.rs:5; unsplash/json.rs:4 | intra (routes.rs:9 prod; dump/web.rs:88,112 test-only) | ✅ **test-only** (routes.rs:40; home:18; dump:47; singlethread:18; json:38) |
| **test** | ✅ test/mod.rs:8,24,29-30,57-58 | ❌ | ✅ test/mod.rs:31,61 | ✅ test/mod.rs:40,72-73 | — |

Key facts:
- A cycle exists between `app` ↔ `infra`: `src/app/state.rs:10` → `infra::metrics`; `src/infra/unsplash.rs:1` → `app::{error,picture}`.
- Nothing references `domain` anywhere (`src/domain/mod.rs` is empty; only mention is its declaration `src/main.rs:4`).
- No production references to `crate::test` exist — all interfaces→test edges live inside `#[cfg(test)]` blocks.
- `main` wires all layers unqualified: app (src/main.rs:14,15,21-26), infra (src/main.rs:17-20), interfaces (src/main.rs:35-46).
- No `pub use` re-exports exist anywhere in the crate.

## Q4: Testing conventions

### Findings
- Two coexisting styles: (A) inline `#[cfg(test)] mod tests` in each source file (11 files: metrics.rs:39, app/error.rs:52, env.rs:45, db.rs:31, assets.rs:46, templates.rs:22, routes.rs:39, home/web.rs:17, dump/web.rs:46, singlethread/web.rs:17, unsplash/json.rs:34); (B) shared helper module `src/test/mod.rs` registered via `#[cfg(test)] mod test;` (src/main.rs:48-49).
- Dev-dependencies: only `tower = "0.5"` (`Cargo.toml:22-23`). HTTP-level tests use plain `reqwest` (a regular dependency) through `test_client()` against real sockets on random ports; `#[tokio::test]` and `#[sqlx::test]` come from regular deps tokio/sqlx.
- New-file registration: inline modules need nothing; a new file under `src/test/` would need a `mod <name>;` line inside `src/test/mod.rs` to compile/run. There is no auto-discovery and no `tests/` dir.
- Helpers: `start_app_with` builds Env with `sqlite::memory:` (src/test/mod.rs:18-23), runs `sqlx::migrate!("./migrations")` (:25-27), assembles real `AppState` (:28-34), serves the real router on 127.0.0.1:0 (:35-43), returns `(SocketAddr, SqlitePool)` (:46). `start_unsplash_stub(status)` spawns a local axum stub of `GET /photos/random` with call counting (:92-133).
- `#[sqlx::test]` used 3×: `src/app/db.rs:35`, `src/interfaces/handlers/unsplash/json.rs:40,51`.
- Env-var tests serialized by mutex: `src/app/env.rs:52-63`.
- `scripts/test.sh` steps: source `.env` → `cargo fmt --all` → `cargo sqlx prepare -- --tests` → `cargo check --all-targets` → `cargo clippy --all-targets --all-features --locked -- -D warnings` → `cargo nextest run` → ripgrep scan of `src` for `FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO\s|todo\s` (exit 1 on hits) (`scripts/test.sh:5-16`).

## Q5: Reference implementation `../api/src/test/arkitect.rs`

### Findings
- Single `#[cfg(test)] mod tests` with one test `fn test_architectural_rules()` (`../api/src/test/arkitect.rs:31`); all custom rule types declared inside the fn body.
- APIs used (imports at :4-13): `Arkitect` (`dsl::arkitect`), `Project::from_current_crate()` (:69), `ArchitecturalRules::define().rules_for_module(...)` with `.it_must_not_depend_on(&[...])` / `.it_may_depend_on(&[...])` / `.it(Box::new(custom_builder))` (:72-85), then `Arkitect::ensure_that(project).complies_with(rules)` asserting `result.is_ok()` (:87-92).
- Rules defined per logical module prefix: `"api::app"` must not depend on `["api::interfaces"]`; `"api::domain"` may depend on allow-list; `"api::infra"` may depend on superset list; `"api::interfaces"` uses custom rule forbidding dep on external crate `"sqlx"` except inside `#[cfg(test)]` code.
- Custom rule implements `SubjectInjectableRuleBuilder::for_subject(&str) -> Box<dyn Rule>` (:101-108); the `Rule` impl (:124-152) has `is_applicable` (`file.logical_path.starts_with(subject)`) and `apply` returning Err listing violations.
- `#[cfg(test)]` exclusion mechanics: `has_cfg_test(attrs)` checks `attr.path().is_ident("cfg")` + token text (:155-164); `deps_outside_test_modules` walks top-level `ast.items`, skipping cfg(test)-gated items via manual `item_attrs` accessor (:217-242), collecting use-trees (`collect_use_tree`, :244-315, handles Path/Group/Name/Glob/Rename incl. `super`/`crate`) into an alias map, recursing into non-test mods (:317-356); a `syn::visit::Visit` walker `PathCollector` (:358-420) tracks `inside_test` for `visit_item_mod`/`visit_item_fn` and collects `visit_expr_path`/`visit_type_path`; `resolve_path` (:422-464) resolves `crate`/`super`/`self` and aliases (rename via `UseTree::Rename`), ignoring single unaliased identifiers.
- Uses let-chains ⇒ requires edition 2024 / Rust ≥ 1.88 (`../api/Cargo.toml:4`; toolchain pinned 1.97.1).
- Version: `rust_arkitect = "0.3.7"` (`../api/Cargo.toml:48`; locked in `../api/Cargo.lock`). Current stable on crates.io is **0.3.7** (max & newest, published 2025-01-25). Agent verified every API used exists in published 0.3.7 source (`Arkitect::init_logger/ensure_that/complies_with`, `Project::from_current_crate`, `ArchitecturalRules::define/rules_for_module/it_must_not_depend_on/it_may_depend_on/it`, trait `Rule: Display { apply, is_applicable }`, public `RustFile { path, logical_path, ast }` fields). The reference file matches 0.3.7 exactly; no newer version or extra features needed.

## Q6: Existing enforcement mechanisms / quality gates

### Findings
- **No `[lints]` table** in `Cargo.toml`; no crate-level `#![deny/warn]` attributes anywhere in `src/`. No `.clippy.toml`, `rustfmt.toml`, `deny.toml`, `.mutants.toml`, or `.cargo/config.toml`.
- Present gates:
  - `rust-toolchain.toml:3-5` pins 1.97.1 with clippy + rustfmt components.
  - `.config/nextest.toml:1-2` (CI junit profile).
  - `codecov.yml` — 70% project / 90% patch coverage thresholds, `src/main.rs` ignored.
  - `scripts/lint_string.sh:4-10` — generic string-ban gate (grep pattern, exit 1).
  - `scripts/test.sh` (see Q4).
- GitHub Actions (all under `.github/workflows/`):
  - `ci.yml` on push-main + PRs: nextest (`ci.yml:71-73`), llvm-cov+Codecov on main (:75-88), TODO/FIXME job running `./scripts/lint_string.sh` 5× (:101-109), `cargo fmt --check` (:111-117), clippy `-D warnings` (:119-133).
  - `ci-secure.yml` weekly: CodeQL (rust + actions, `security-and-quality` queries via `codeql/codeql.yml:1-4`) and a SARIF clippy report that is `continue-on-error` (non-blocking).
  - `fly-deploy.yml:8-19` deploys to Fly on main push; `rust-version-bump.yml` daily toolchain-bump PR; `dependabot.yml` daily cargo/actions updates with `dependabot_auto_merge.yml` auto-merging minor PRs.
- Where an additional automated check currently fits: the repo's own precedent is a `#[cfg(test)]`-gated test run by nextest (like `../api/src/test/arkitect.rs`) plus script-based greps (`scripts/lint_string.sh` invoked from both `test.sh`-style chains and `ci.yml`). No `[lints]`/clippy-config mechanism for architecture rules exists today.

## Q7: Production I/O touchpoints (DB and outbound HTTP)

### Findings
Production sqlx usage (not test-gated):
- `src/app/db.rs:7-29` — only pool-construction site (`SqlitePoolOptions`, WAL, max_connections(5)).
- `src/app/state.rs:8` — pool held in `AppState.db`; wired in `src/main.rs:23`.
- `src/app/picture.rs:13-20` (`latest`), :22-32 (`create`) — runtime `query_as` against `unsplash_pictures`.
- `src/app/error.rs:12,23-26` — `Database(sqlx::Error)` variant + `From<sqlx::Error>`.
- `src/interfaces/handlers/dump/web.rs:19-25,35-39` — **compile-time-checked `sqlx::query_as!`/`query!` executed directly in the handler**, bypassing the app layer.
- Handler calls through app layer: `src/interfaces/handlers/unsplash/json.rs:11,23`.

Production outbound HTTP (reqwest):
- `src/infra/unsplash.rs:22-45` — sole explicit client wrapper: `fetch_random(client, base_url, api_key)` doing GET `{base_url}/photos/random` with Client-ID auth; non-2xx → `WebError::External`.
- `src/interfaces/handlers/unsplash/json.rs:16` — handler constructs `reqwest::Client::new()` per request and passes it in (:17-21).
- Implicit network egress: sentry transport — init at `src/infra/sentry.rs:1-10` (enabled via `src/main.rs:17-19`), capture at `src/app/error.rs:35,40`.

Test-gated I/O code (excluded from production): `src/app/db.rs:33-56`, `src/interfaces/handlers/unsplash/json.rs:35+`, and all of `src/test/mod.rs` (migrations at :25, stub server :92-133, `test_client()` :87-89).

Layer ownership (factual): DB pool creation + entity queries live in `app`; the HTTP client wrapper lives in `infra`; `interfaces` reaches into both — including two direct-I/O exceptions (`dump/web.rs` raw SQL, `unsplash/json.rs` client construction). `domain` has no I/O (empty module).

## Cross-Cutting Observations
- Everything is one binary crate rooted at `src/main.rs`; `crate::` paths reach any module freely, so architectural rules would operate on logical paths prefixed `vardy::app`, `vardy::infra`, etc. (the reference uses `api::*` prefixes for the same purpose).
- Layering today is not strict: `app↔infra` mutual references (`src/app/state.rs:10`, `src/infra/unsplash.rs:1`), sqlx macros and reqwest construction inside handlers, and axum types in the app layer are existing facts any rule set would have to encode as allowed (or the suite would fail immediately).
- The `#[cfg(test)] mod test;` pattern at the binary root (`src/main.rs:48-49`) is exactly where a sibling `arkitect.rs` file would be registered (one `mod arkitect;` line in `src/test/mod.rs`).
- Toolchain is edition 2024 / Rust 1.97.1 (`Cargo.toml:3`, `rust-toolchain.toml:3`), satisfying the let-chain requirement seen in the reference implementation.
- CI runs nextest on every PR (`ci.yml:71-73`), so a cfg(test)-based arkitect test would execute automatically without new workflow changes.

## Open Areas
- GitHub branch-protection settings (which CI jobs are required) are not observable from this filesystem.
- Whether `rust_arkitect` handles macro-generated `crate::` paths or re-export chains was not verified beyond the reference implementation's documented limitations (string-prefix matching, single-segment identifiers ignored).
- The orphan empty file `src/infra/db.rs` is not compiled; its intent is unknown.
