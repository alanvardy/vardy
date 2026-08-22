# Design — VAR-677: Add arkitect (architectural boundary enforcement)

## Current State

Binary-only crate rooted at `src/main.rs`; every path is `crate::<module>`
(`Cargo.toml:2-3`, modules declared `src/main.rs:3-6`). Top-level modules:
`app`, `domain` (empty placeholder), `infra`, `interfaces`, plus
test-gated `test` (`src/main.rs:48-49`). No `[lints]` table, no clippy
config; quality gates are `scripts/test.sh` (fmt → sqlx prepare → check →
clippy `-D warnings` → nextest → TODO grep) and GitHub Actions
(`ci.yml:71-73` runs nextest on every PR).

Actual layering today is **not** strict:

| Edge | Evidence |
|---|---|
| app → infra | `src/app/state.rs:10` (`AppState.metrics: Arc<crate::infra::metrics::AppMetrics>`) |
| infra → app | `src/infra/unsplash.rs:1` (`use crate::app::{error::WebError, picture::Picture}`) |
| interfaces → sqlx | `src/interfaces/handlers/dump/web.rs:19,35` (`query_as!` / `query!` in handler) |
| interfaces → reqwest | `src/interfaces/handlers/unsplash/json.rs:16` (`reqwest::Client::new()` per request) |
| interfaces → infra | `src/interfaces/routes.rs:32`, `src/interfaces/handlers/metrics/web.rs:5` (name `infra::metrics::AppMetrics`) |

A proven reference exists at `../api/src/test/arkitect.rs`: single
`#[cfg(test)]` test using `rust_arkitect = "0.3.7"` (current crates.io
stable; every API used verified against published 0.3.7), with ~250 lines
of syn-based AST walking that excludes `#[cfg(test)]`-gated code from
dependency analysis (`arkitect.rs:155-464`). Toolchain here is edition
2024 / Rust 1.97.1, satisfying its let-chain requirement.

## Desired End State

1. `rust_arkitect` added as dev-dependency; a cfg(test)-gated
   architectural test at `src/test/arkitect.rs` (registered with one
   `mod arkitect;` line in `src/test/mod.rs`) fails the suite on any
   boundary violation.
2. The codebase actually complies with strict layering — violations above
   are fixed, not encoded as permanent exceptions.
3. Verification: `./scripts/test.sh` green; deliberately breaking a rule
   locally (e.g. adding `use crate::app::...` under `src/infra/`)
   makes the arkitect test fail.

Target dependency graph (→ = may depend on):

```
domain   → (nothing crate-internal)
infra    → domain
app      → domain, infra
interfaces → app, domain          (NOT infra)
main.rs  → wires everything       (root path "vardy"; matched by no rule)
test     → anything               (excluded via cfg(test) machinery)
```

## Patterns to Follow

- **Reference port**: structure of `../api/src/test/arkitect.rs` — all
  rule types inside the test fn, `Project::from_current_crate()`,
  `ArchitecturalRules::define().rules_for_module(...)`
  (`arkitect.rs:69-92`).
- **Test registration**: sibling-file pattern of `src/test/mod.rs`;
  add `mod arkitect;` next to existing helpers (`src/main.rs:48-49`).
- **cfg(test) exclusion**: port the visitor/alias-resolution machinery
  verbatim (`has_cfg_test` :155-164, `deps_outside_test_modules`
  :217-242, `collect_use_tree` :244-315, `PathCollector` :358-420,
  `resolve_path` :422-464). Test-gated imports are pervasive here
  (`tower` in `routes.rs:66`, `crate::test::*` in every handler).
- **Entity placement**: after this task, DB-backed entities like
  `Picture` live in `domain` as pure data types; SQL lives in `app`.
- **Error translation at layer boundaries**: infra returns its own error
  type; `app/error.rs` maps it into `WebError` (mirrors existing
  `From<sqlx::Error>` pattern at `src/app/error.rs:23-26`).

Patterns NOT to follow / remove:
- Raw `sqlx::query!` macros in handlers (`dump/web.rs:19,35`) — violates
  the new rules.
- Per-request client construction in handlers (`json.rs:16`).
- Cross-layer `use crate::app::…` from infra (`unsplash.rs:1`).

## Design Decisions

1. **Enforce target architecture now (Q1:B)** — write strict rules and
   fix all current violations within VAR-677. Prevents blessing decay as
   allow-lists.
2. **Port cfg(test) exclusion machinery verbatim (Q2:A)** — proven code,
   identical edition/toolchain; lets rules reason about production deps.
3. **Ban sqlx + reqwest in `interfaces` (Q3:yes)** — via the reference's
   custom-rule mechanism (external-crate ban outside cfg(test)).
4. **Aspirational `domain` rules (Q4:a)** — define them while the module
   is empty; they cost nothing and lock in purity.
5. **Break the cycle by direction, not relocation**: `app → infra` stays
   legal (composition: `AppState.metrics`), `infra → app` becomes
   illegal. Concretely in `src/infra/unsplash.rs`:
   - `Picture` moves to new `src/domain/picture.rs` (serde + sqlx
     `FromRow` derives stay — accepted pragmatic exception for domain).
   - `fetch_random` returns `Result<Picture, UnsplashError>` with
     `UnsplashError` defined in `infra` (thin `String` wrapper);
     `app/error.rs` gains `From<UnsplashError> for WebError`
     (maps to `WebError::External`).
6. **Shared HTTP client in state**: add `pub http: reqwest::Client` to
   `AppState`, built once in `main.rs`; `json.rs` passes `&state.http`.
   Kills the per-request construction and removes reqwest from handlers.
7. **Dump SQL moves to app**: extract the two queries in
   `dump/web.rs:19,35` into typed functions in a new `src/app/dump.rs`;
   handler calls them and maps errors via `WebError`.
8. **interfaces → infra broken via re-export**: `app/state.rs` re-exports
   `pub use crate::infra::metrics::AppMetrics;` (first deliberate
   `pub use` in the crate); `routes.rs:32` and `metrics/web.rs:5` then
   reference only `crate::app::…` paths, which the prefix matcher sees.
9. **Rule set** (prefix-based on logical paths `vardy::*`):
   - `vardy::domain` must_not_depend_on [app, infra, interfaces]
   - `vardy::infra` must_not_depend_on [app, interfaces]; may_depend_on
     [domain]
   - `vardy::app` must_not_depend_on [interfaces]; may_depend_on
     [domain, infra]
   - `vardy::interfaces` must_not_depend_on [infra]; may_depend_on
     [app, domain]; custom rule: no `sqlx`/`reqwest` deps outside
     cfg(test)

## What We're NOT Doing

- Not moving `metrics` out of `infra` or restructuring `AppState`.
- Not touching `sentry`, `templates`, `assets`, `env`, `log` beyond what
  compliance requires.
- Not adding CI workflow changes — the existing nextest job
  (`ci.yml:71-73`) picks the test up automatically.
- Not deleting the orphan uncompilable `src/infra/db.rs` here (separate
  cleanup candidate; note it in the PR description).
- Not enforcing rules on `main.rs` (logical path `vardy` — it must wire
  every layer).
- Not banning other externals in `interfaces` (axum, chrono, prometheus
  remain legal); only sqlx/reqwest per decision 3.
- No route changes → no `ROUTES.md` updates needed.

## Open Risks

- **rust_arkitect string-prefix matching**: rules match logical paths
  textually; macro-generated `crate::` paths or unusual use-tree forms
  could slip past the AST walker (reference documents these limits;
  research did not verify beyond that). Mitigation: the violation-fixing
  tests double as spot checks; revisit if a false negative appears.
- **`sqlx::FromRow` in domain** is an accepted impurity; if later purity
  is wanted, switch queries to `query_as!` (no trait needed).
- **Re-export loophole**: decision 8 means `interfaces` could reach infra
  types through `app` re-exports. Accepted for now (single known case);
  note in code comment that the re-export is the sanctioned surface.
- **nextest execution**: arkitect test runs under nextest like any test;
  compile-time cost of syn parsing the whole crate in test builds is
  real but bounded (dev-dep, parallel profile).
