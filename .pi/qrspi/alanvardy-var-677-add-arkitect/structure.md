# Structure Outline

## Approach
Port the proven `rust_arkitect` cfg(test)-exclusion machinery from `../api/src/test/arkitect.rs`, then enforce the target dependency graph incrementally: each phase adds one rule **and** fixes the violations that rule exposes, so the suite is green at every phase boundary. Refactor slices here are vertical in the sense that each crosses rule definition → production code fix → handler/test verification.

## Phase 1: Enforcement harness (rules that already hold)
Port the arkitect test verbatim and enable only the rules the codebase already satisfies: `domain` must_not_depend_on [app, infra, interfaces], `app` must_not_depend_on [interfaces], `infra` may_depend_on [domain]. Delivers a working, CI-run architectural gate with zero production churn.

**Files**: `Cargo.toml`, `src/test/arkitect.rs` (new), `src/test/mod.rs`
**Key changes**:
- `rust_arkitect = "0.3.7"` — new dev-dependency
- `mod arkitect;` in `src/test/mod.rs`
- `fn test_architectural_rules()` — ported test; rule types + cfg(test) exclusion machinery (`has_cfg_test`, `deps_outside_test_modules`, `collect_use_tree`, `PathCollector`, `resolve_path`) inside the fn, per reference
- Rules use logical-path prefixes `vardy::domain`, `vardy::app`, `vardy::infra`

**Verify**: `./scripts/test.sh` passes (nextest picks up the new test); manually confirm the test runs: `cargo nextest run -E 'test(architectural)'`.

---

## Phase 2: Move `Picture` to domain; break infra → app
`Picture` becomes a pure domain type; `fetch_random` returns an infra-owned error instead of `WebError`, and `app/error.rs` translates it. Enables the `infra` must_not_depend_on [app] rule.

**Files**: `src/domain/picture.rs` (new), `src/domain/mod.rs`, `src/app/picture.rs`, `src/infra/unsplash.rs`, `src/app/error.rs`, `src/interfaces/handlers/unsplash/json.rs`, `src/test/arkitect.rs`
**Key changes**:
- `src/domain/picture.rs`: `#[derive(Serialize, sqlx::FromRow)] pub struct Picture { url, photographer, created_at }` — moved from `app/picture.rs`
- `app/picture.rs`: `latest`/`create` signatures unchanged, import `crate::domain::picture::Picture`
- `pub struct UnsplashError(String)` — new, in `infra/unsplash.rs`
- `fetch_random(client: &Client, base_url: &str, api_key: &str) -> Result<Picture, UnsplashError>` — return type changed
- `impl From<UnsplashError> for WebError` (→ `WebError::External`) in `app/error.rs` — handler call site needs no change beyond `?`
- Enable rule: `vardy::infra` must_not_depend_on `[app, interfaces]`

**Verify**: `./scripts/test.sh` passes; unsplash handler tests (`json.rs`) still green — error translation preserves HTTP 502 behavior.

---

## Phase 3: Break interfaces → infra via sanctioned re-export
`AppState` re-exports `AppMetrics` through `app`, so `routes.rs` and the metrics handler reference only `crate::app::…`. Enables the `interfaces` must_not_depend_on [infra] rule.

**Files**: `src/app/state.rs`, `src/interfaces/routes.rs`, `src/interfaces/handlers/metrics/web.rs`, `src/test/arkitect.rs`
**Key changes**:
- `app/state.rs`: add `pub use crate::infra::metrics::AppMetrics;` (comment: sanctioned surface for infra types)
- `routes.rs` / `metrics/web.rs`: replace `crate::infra::metrics::AppMetrics` paths with `crate::app::state::AppMetrics`
- Enable rule: `vardy::interfaces` must_not_depend_on `[infra]`; may_depend_on `[app, domain]`

**Verify**: `./scripts/test.sh` passes; metrics endpoint tests still report page hits.

---

## Phase 4: Dump SQL moves to app; ban sqlx in interfaces
Extract the raw `query_as!`/`query!` calls from `dump/web.rs` into typed functions in a new `src/app/dump.rs`; add the custom external-crate rule banning `sqlx` in `interfaces` outside cfg(test).

**Files**: `src/app/dump.rs` (new), `src/app/mod.rs`, `src/interfaces/handlers/dump/web.rs`, `src/test/arkitect.rs`
**Key changes**:
- `app/dump.rs`: `pub async fn list(pool: &SqlitePool, key: &str) -> Result<Vec<DumpEntry>, sqlx::Error>` and `pub async fn create(pool: &SqlitePool, key: &str, body: &str) -> sqlx::Result<()>` — `DumpEntry` stays in the handler (serde type) or moves to `app/dump.rs`; pick whichever keeps the handler free of sqlx
- `dump/web.rs` handlers: call `crate::app::dump::{list, create}`, map errors via `From<sqlx::Error>` (existing)
- Custom rule ported from reference (`it(Box::new(...))`): subject `vardy::interfaces`, forbidding external dep `sqlx` outside `#[cfg(test)]`

**Verify**: `./scripts/test.sh` passes; all four dump handler tests unchanged and green (`cargo sqlx prepare` refreshed for the moved macros).

---

## Phase 5: Shared HTTP client; ban reqwest in interfaces
`reqwest::Client` moves into `AppState`, built once in `main.rs`; the unsplash handler stops constructing clients per request. Extend the custom rule to ban `reqwest` too.

**Files**: `src/app/state.rs`, `src/main.rs`, `src/interfaces/handlers/unsplash/json.rs`, `src/test/arkitect.rs`
**Key changes**:
- `AppState`: add `pub http: reqwest::Client`
- `main.rs`: build `reqwest::Client::new()` alongside the pool, pass into `AppState`
- `json.rs`: `fetch_random(&state.http, ...)` instead of `reqwest::Client::new()`
- Extend custom rule's banned externals: `sqlx`, `reqwest`

**Verify**: `./scripts/test.sh` passes; unsplash stub test (`start_unsplash_stub`) still green. **Final sanity check**: temporarily add `use crate::app::error::WebError;` under `src/infra/` and confirm the arkitect test fails, then revert.

---

## Testing Checkpoints
After each phase, `./scripts/test.sh` must be green (fmt → sqlx prepare → check → clippy `-D warnings` → nextest → TODO grep). Resumable state:

1. **Phase 1 done**: `cargo nextest run -E 'test(architectural)'` passes; rules cover domain/app/infra-legal state.
2. **Phase 2 done**: no `crate::app::…` under `src/infra/`; `Picture` lives in `src/domain/picture.rs`; `UnsplashError` + `From` impl exist; infra rule enabled.
3. **Phase 3 done**: no `crate::infra::` references outside `src/app/`, `src/main.rs`, and the re-export; interfaces→infra rule enabled.
4. **Phase 4 done**: no `sqlx::` outside cfg(test) under `src/interfaces/`; dump queries in `src/app/dump.rs`; sqlx ban enabled.
5. **Phase 5 done**: no `reqwest::` outside cfg(test) under `src/interfaces/`; `AppState.http` exists; both bans enabled; deliberate-violation sanity check performed and reverted.

## Notes
- No DB migration, route, or `ROUTES.md` changes anywhere — no user-visible behavior changes.
- The re-export in Phase 3 is a known, documented loophole (interfaces reaching infra types via `app`); accepted per design decision 8.
- `sqlx::FromRow` on the domain `Picture` is an accepted impurity per design.
- Nothing in this design requires horizontal layering; every phase pairs a rule with its fix.
