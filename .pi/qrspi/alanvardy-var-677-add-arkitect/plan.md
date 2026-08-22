# Implementation Plan

## Overview

Add `rust_arkitect`-based architectural boundary enforcement as a cfg(test)-gated test, fixing every existing layering violation so strict rules pass: `domain` pure, `infra → app` broken, `interfaces → infra` broken via sanctioned re-exports, and `sqlx`/`reqwest` banned from `interfaces` production code.

## Deviations from structure.md (all resolved, none blocking)

1. **Phase 1 — infra rule encoded as blacklist, not allow-list.** Structure says `infra may_depend_on [domain]`. Verified against rust_arkitect 0.3.7 source: `MayDependOnRule` checks **all** deps (external crates included, cfg(test) code included) against the allow-list, so a bare `["vardy::domain"]` list fails immediately on `reqwest`/`serde`/`prometheus`/`sentry`. Encoding as `it_must_not_depend_on(&["vardy::app", "vardy::interfaces"])` (the form design decision 9 actually specifies) captures the same intent without a decaying external-crate allow-list.
2. **Phase 3 — two extra files touched.** The stock `must_not_depend_on` rule does **not** exclude cfg(test) code, and it scans production code too. Enabling `interfaces must_not_depend_on [infra]` therefore also flags:
   - `src/interfaces/routes.rs:62` — `use crate::infra::metrics::AppMetrics;` inside the cfg(test) block → switch to the `app` re-export.
   - `src/interfaces/handlers/unsplash/json.rs:4` — production `use crate::infra::unsplash::fetch_random;` → add a sanctioned re-export in `src/app/picture.rs` (same pattern as decision 8) and import through `app`.
3. **Phase 4 — `DumpEntry` must move to `app/dump.rs`** (structure left this optional). If it stayed in the handler, `app::dump::list` returning a handler type would create `app → interfaces` — a forbidden edge.
4. **Phase 5 — `src/test/mod.rs` also needs the new `http` field.** It constructs `AppState` literals twice (`:29`, `:57`); structure's file list omitted it.

---

## Phase 1: Enforcement harness (rules that already hold)

### Changes

#### 1. Add dev-dependency
**File**: `Cargo.toml`
**Action**: modify

```toml
[dev-dependencies]
tower = "0.5"
rust_arkitect = "0.3.7"
```

This updates `Cargo.lock` on first build (required before clippy's `--locked` step passes).

#### 2. Port the arkitect test
**File**: `src/test/arkitect.rs`
**Action**: create

Port `../api/src/test/arkitect.rs` **verbatim** (whole `#[cfg(test)] mod tests`, one test `fn test_architectural_rules()`, custom rule types + cfg(test)-exclusion machinery `has_cfg_test` / `deps_outside_test_modules` / `item_attrs` / `collect_use_tree` / `collect_mod_items` / `PathCollector` / `resolve_path` all inside the test fn), with these substitutions:

- All `#[cfg(test)]` attributes on the inner items are kept (they matter: they keep the helper fns from tripping dead-code/clippy in non-test builds and are what the reference uses).
- Replace the reference's rule block with:

```rust
let project = Project::from_current_crate();

let rules = ArchitecturalRules::define()
    .rules_for_module("vardy::domain")
    .it_must_not_depend_on(&["vardy::app", "vardy::infra", "vardy::interfaces"])
    .rules_for_module("vardy::app")
    .it_must_not_depend_on(&["vardy::interfaces"])
    .build();

let result = Arkitect::ensure_that(project).complies_with(rules);

assert!(
    result.is_ok(),
    "Detected {} violations",
    result.err().unwrap().len()
);
```

- Keep the `MustNotDependOnExceptTests` / `MustNotDependOnExceptTestsBuilder` types and all AST helpers in the file even though Phase 1 doesn't use the custom rule yet — Phases 4–5 need them, and dead-code is suppressed by the `#[cfg(test)]` gating (same as reference).
- Do **not** add a `domain` allow-list or an `infra` rule yet (Phases 2's infra rule comes later; domain blacklist already holds today).

Note on prefix semantics (verified in rust_arkitect 0.3.7 `IsChild`): a rule subject `vardy::app` applies to every file whose logical path equals or starts with `vardy::app::`; root `main.rs` has logical path `vardy` and is matched by no rule.

#### 3. Register the module
**File**: `src/test/mod.rs`
**Action**: modify — add one line at the top of the file:

```rust
mod arkitect;
```

### Verification
#### Automated
- [x] `./scripts/test.sh` passes end to end (fmt → sqlx prepare → check → clippy → nextest → TODO grep)
- [x] `cargo nextest run -E 'test(architectural)'` runs and passes exactly the new test
- [x] `git diff Cargo.lock` shows `rust_arkitect` added (clippy `--locked` requires the committed lockfile)

#### Manual
- [ ] Temporarily add `use crate::interfaces::routes::routes;` anywhere under `src/app/`, confirm `cargo nextest run -E 'test(architectural)'` **fails**, then revert

---

## Phase 2: Move `Picture` to domain; break infra → app

### Changes

#### 1. Domain type
**File**: `src/domain/picture.rs`
**Action**: create — move the struct from `src/app/picture.rs` unchanged (serde + `sqlx::FromRow` derives are the accepted pragmatic exception per design):

```rust
use serde::Serialize;

/// A picture served by the `/unsplash` endpoint, persisted in the
/// `unsplash_pictures` table.
#[derive(Serialize, sqlx::FromRow)]
pub struct Picture {
    pub url: String,
    pub photographer: String,
    pub created_at: String,
}
```

#### 2. Register domain module
**File**: `src/domain/mod.rs`
**Action**: modify — replace the (empty) file content with:

```rust
pub mod picture;
```

#### 3. App queries keep signatures, new import
**File**: `src/app/picture.rs`
**Action**: modify — remove the `Picture` struct definition, add at top:

```rust
use crate::domain::picture::Picture;
```

`latest` / `create` bodies and signatures unchanged.

#### 4. Infra-owned error + new return type
**File**: `src/infra/unsplash.rs`
**Action**: modify

- Replace the `use` line (removes the `crate::app` dependency entirely):

```rust
use crate::domain::picture::Picture;
use reqwest::Client;
use serde::Deserialize;
```

- Add the error type (public field so `app/error.rs` can translate it):

```rust
/// Failure talking to the Unsplash API; translated into
/// `WebError::External` (HTTP 502) at the app layer.
#[derive(Debug)]
pub struct UnsplashError(pub String);
```

- Change `fetch_random` signature and the three `map_err` closures:

```rust
pub async fn fetch_random(
    client: &Client,
    base_url: &str,
    api_key: &str,
) -> Result<Picture, UnsplashError> {
    let response = client
        .get(format!("{base_url}/photos/random"))
        .query(&[("query", "nature")])
        .header("Authorization", format!("Client-ID {api_key}"))
        .send()
        .await
        .map_err(|e| UnsplashError(format!("unsplash request failed: {e}")))?;

    if !response.status().is_success() {
        return Err(UnsplashError(format!(
            "unsplash returned status {}",
            response.status()
        )));
    }

    let body: RandomPhotoResponse = response
        .json()
        .await
        .map_err(|e| UnsplashError(format!("unsplash response parse failed: {e}")))?;

    Ok(Picture {
        url: body.urls.regular,
        photographer: body.user.name,
        created_at: String::new(), // populated by the DB on insert
    })
}
```

- Update the doc comment: "Non-2xx status or parse failure maps to `WebError::External` (HTTP 502) via `From<UnsplashError>`."

#### 5. Error translation
**File**: `src/app/error.rs`
**Action**: modify — add next to the existing `From<sqlx::Error>` impl (mirrors that pattern):

```rust
impl From<crate::infra::unsplash::UnsplashError> for WebError {
    fn from(err: crate::infra::unsplash::UnsplashError) -> Self {
        WebError::External(err.0)
    }
}
```

#### 6. Handler import path
**File**: `src/interfaces/handlers/unsplash/json.rs`
**Action**: modify — change line 2:

```rust
use crate::app::picture::{self};
use crate::domain::picture::Picture;
```

(keep `use crate::infra::unsplash::fetch_random;` for now — Phase 3 removes it). The `?` on the `fetch_random(...)` call now converts `UnsplashError → WebError` via the new `From` impl; no call-site change. The cfg(test) block's `use super::*;` picks up the new `Picture` path automatically; `upstream_failure_is_502` still asserts 502 + `"bad gateway"`.

#### 7. Enable the infra rule
**File**: `src/test/arkitect.rs`
**Action**: modify — extend the rule chain:

```rust
    .rules_for_module("vardy::infra")
    .it_must_not_depend_on(&["vardy::app", "vardy::interfaces"])
```

(Verified safe: after step 4 nothing under `src/infra/` references `crate::app` or `crate::interfaces`, in production or test code.)

### Verification
#### Automated
- [ ] `./scripts/test.sh` passes
- [ ] `cargo nextest run -E 'test(unsplash)'` — all json.rs tests green, especially `upstream_failure_is_502` (asserts status 502 **and** body `"bad gateway"`)
- [ ] `rg -n 'crate::app' src/infra/` returns no hits

#### Manual
- [ ] Confirm `Picture` is defined only in `src/domain/picture.rs` (`rg -n 'pub struct Picture' src/` → one hit)

---

## Phase 3: Break interfaces → infra via sanctioned re-exports

### Changes

#### 1. Re-export `AppMetrics` through app (sanctioned surface)
**File**: `src/app/state.rs`
**Action**: modify — add at top, with the agreed comment:

```rust
/// Sanctioned surface for infra types consumed by `interfaces`; do not
/// import from `crate::infra` outside `src/app` and `main.rs`.
pub use crate::infra::metrics::AppMetrics;
```

(`AppState.metrics` field keeps its `crate::infra::metrics::AppMetrics` path — `app → infra` is legal.)

#### 2. Re-export `fetch_random` through app
**File**: `src/app/picture.rs`
**Action**: modify — add below the imports (deviation note 2; same sanctioned-surface pattern as step 1):

```rust
/// Sanctioned re-export: the Unsplash fetch is implemented in `infra`
/// but `interfaces` must reach it only through `app`.
pub use crate::infra::unsplash::fetch_random;
```

#### 3. Update interfaces references
**File**: `src/interfaces/routes.rs`
**Action**: modify

- Line 32: `pub fn metrics_router(metrics: std::sync::Arc<crate::app::state::AppMetrics>) -> Router {`
- Line 62 (inside cfg(test) `metrics_router_serves_metrics_endpoint`): change `use crate::infra::metrics::AppMetrics;` → `use crate::app::state::AppMetrics;` — required because the stock rule does not exclude test code (deviation note 2)

**File**: `src/interfaces/handlers/metrics/web.rs`
**Action**: modify — line 5: `use crate::app::state::AppMetrics;`

**File**: `src/interfaces/handlers/unsplash/json.rs`
**Action**: modify — line 4: `use crate::infra::unsplash::fetch_random;` → fold into the existing picture import:

```rust
use crate::app::picture::{self, fetch_random};
```

#### 4. Enable the interfaces rule
**File**: `src/test/arkitect.rs`
**Action**: modify — append to the rule chain:

```rust
    .rules_for_module("vardy::interfaces")
    .it_must_not_depend_on(&["vardy::infra"])
```

(Verified safe: after steps 1–3 the only remaining `crate::infra` references are under `src/app/`, `src/main.rs`, and the two re-export lines themselves — all outside the `vardy::interfaces` subject.)

### Verification
#### Automated
- [ ] `./scripts/test.sh` passes
- [ ] `cargo nextest run -E 'test(metrics)'` green (page hits still reported; `metrics_router_serves_metrics_endpoint` still passes)
- [ ] `rg -n 'crate::infra' src/interfaces/` returns no hits

#### Manual
- [ ] `curl localhost:9090/metrics` on a locally booted server still shows `page_views_total`

---

## Phase 4: Dump SQL moves to app; ban sqlx in interfaces

### Changes

#### 1. Typed query functions
**File**: `src/app/dump.rs`
**Action**: create — the two `query_as!`/`query!` macros move here verbatim (still compile-time checked; `app` already has sqlx). `DumpEntry` moves here too — it must, or `app` would depend on `interfaces` (deviation note 3):

```rust
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Serialize, Deserialize)]
pub struct DumpEntry {
    pub id: i64,
    pub body: serde_json::Value,
}

pub async fn list(pool: &SqlitePool, key: &str) -> sqlx::Result<Vec<DumpEntry>> {
    let entries = sqlx::query_as!(
        DumpEntry,
        r#"SELECT id, body AS "body: serde_json::Value" FROM dumps WHERE key = ? ORDER BY id"#,
        key
    )
    .fetch_all(pool)
    .await?;
    Ok(entries)
}

pub async fn create(pool: &SqlitePool, key: &str, body: &str) -> sqlx::Result<()> {
    sqlx::query!("INSERT INTO dumps (key, body) VALUES (?, ?)", key, body)
        .execute(pool)
        .await?;
    Ok(())
}
```

#### 2. Register module
**File**: `src/app/mod.rs`
**Action**: modify — add `pub mod dump;` (alphabetical, between `db` and `env`).

#### 3. Handler becomes sqlx-free
**File**: `src/interfaces/handlers/dump/web.rs`
**Action**: modify

- Delete the `DumpEntry` struct definition; add:

```rust
use crate::app::dump::{self, DumpEntry};
```

- `index` body:

```rust
let entries = dump::list(&state.db, &key).await?;
Ok(Json(entries))
```

- `create` body (error flows through the existing `From<sqlx::Error> for WebError`):

```rust
let serialized = serde_json::to_string(&body).expect("serializing Value cannot fail");
dump::create(&state.db, &key, &serialized).await?;
Ok(StatusCode::CREATED)
```

- In the cfg(test) block, update the two `Vec<crate::interfaces::handlers::dump::web::DumpEntry>` annotations (`:88`, `:112`) to `Vec<crate::app::dump::DumpEntry>`.

#### 4. Custom external-crate ban (sqlx)
**File**: `src/test/arkitect.rs`
**Action**: modify — chain the ported custom rule onto the interfaces subject:

```rust
    .rules_for_module("vardy::interfaces")
    .it_must_not_depend_on(&["vardy::infra"])
    .and_it(Box::new(MustNotDependOnExceptTestsBuilder {
        forbidden: vec!["sqlx".to_string()],
    }))
```

(The `and_it` combinator exists on `ArchitecturalRules<RulesDefined>` — verified in 0.3.7 source. The custom rule's `apply` uses the ported `deps_outside_test_modules`, so sqlx stays legal inside cfg(test) — which json.rs tests and `src/test/mod.rs` rely on.)

#### 5. Refresh offline metadata
The `query!` macros moved files, so `.sqlx/` cache entries must be regenerated — `./scripts/test.sh` does this via `cargo sqlx prepare -- --tests`. If sqlx codegen fails (no reachable `DATABASE_URL`): check `.env` exists and contains `DATABASE_URL=sqlite:data/vardy.db`, then rerun. Fallback if `cargo sqlx` is unavailable: `SQLX_OFFLINE=true cargo check --all-targets` still passes only with correct cached metadata — do not hand-edit `.sqlx/`; fix the environment instead.

### Verification
#### Automated
- [ ] `./scripts/test.sh` passes (its sqlx-prepare step refreshes `.sqlx/` for the moved macros)
- [ ] `cargo nextest run -E 'test(dump)'` — all four dump tests green, asserting both status and body (`[]`, round-trip payload, accumulation order, 400 on bad JSON)
- [ ] `rg -n 'sqlx' src/interfaces/` shows hits only inside `#[cfg(test)]` blocks

#### Manual
- [ ] `git diff .sqlx/` shows regenerated metadata for the two moved queries (and no deleted entries for still-existing queries)

---

## Phase 5: Shared HTTP client; ban reqwest in interfaces

### Changes

#### 1. Client on AppState
**File**: `src/app/state.rs`
**Action**: modify — add field:

```rust
/// Shared outbound HTTP client, built once in `main.rs`.
pub http: reqwest::Client,
```

#### 2. Build once at startup
**File**: `src/main.rs`
**Action**: modify — inside `main`, before the `AppState` literal:

```rust
let http = reqwest::Client::new();
```

and add to the literal: `http,` (field-init shorthand).

#### 3. Test helpers construct AppState too
**File**: `src/test/mod.rs`
**Action**: modify — add `http: reqwest::Client::new(),` to **both** `AppState` literals (`start_app_with` `:29`, `start_app_with_metrics` `:57`). `reqwest` is already imported in this file.

#### 4. Handler uses the shared client
**File**: `src/interfaces/handlers/unsplash/json.rs`
**Action**: modify — in `index`, delete `let client = reqwest::Client::new();` and change the call to:

```rust
let picture = fetch_random(&state.http, &state.unsplash_base_url, &state.env.unsplash_api_key)
    .await?;
```

#### 5. Extend the ban
**File**: `src/test/arkitect.rs`
**Action**: modify — the custom rule's `forbidden` vec becomes:

```rust
        forbidden: vec!["sqlx".to_string(), "reqwest".to_string()],
```

### Verification
#### Automated
- [ ] `./scripts/test.sh` passes
- [ ] `cargo nextest run -E 'test(unsplash)'` green — stub tests (`no_row_triggers_fetch_and_insert`, `second_request_within_window_is_cached`, `stale_row_triggers_refetch`) prove the shared client works end to end
- [ ] `rg -n 'reqwest' src/interfaces/` shows hits only inside `#[cfg(test)]` blocks

#### Manual (deliberate-violation sanity check, then revert)
- [ ] Temporarily add `use crate::app::error::WebError;` under `src/infra/unsplash.rs` → `cargo nextest run -E 'test(architectural)'` **fails** (infra must_not_depend_on app)
- [ ] Temporarily add `let c = reqwest::Client::new();` in a non-test fn under `src/interfaces/` → arkitect test **fails** (custom reqwest ban)
- [ ] Revert both

---

## Testing Checkpoints

After each phase, `./scripts/test.sh` must be green. Resumable state:

1. **Phase 1 done**: `cargo nextest run -E 'test(architectural)'` passes; rules cover domain + app (rules that already hold).
2. **Phase 2 done**: no `crate::app::…` under `src/infra/`; `Picture` lives in `src/domain/picture.rs`; `UnsplashError` + `From` impl exist; infra rule enabled.
3. **Phase 3 done**: no `crate::infra::` references under `src/interfaces/` (prod or test); two sanctioned re-exports in `app`; interfaces→infra rule enabled.
4. **Phase 4 done**: no `sqlx::` outside cfg(test) under `src/interfaces/`; dump queries + `DumpEntry` in `src/app/dump.rs`; `.sqlx/` refreshed; sqlx ban enabled.
5. **Phase 5 done**: no `reqwest::` outside cfg(test) under `src/interfaces/`; `AppState.http` exists (main.rs + both test helpers); both bans enabled; deliberate-violation sanity checks performed and reverted.

## Notes
- No DB migration, route, or `ROUTES.md` changes anywhere — no user-visible behavior changes.
- The Phase 3 re-exports are the documented loophole (interfaces reaching infra types via `app`); accepted per design decision 8.
- `sqlx::FromRow` on the domain `Picture` is an accepted impurity per design.
- The orphan uncompilable `src/infra/db.rs` is not touched; mention it in the PR description as a separate cleanup candidate.
- No CI workflow changes — nextest (`ci.yml:71-73`) picks up the arkitect test automatically.
