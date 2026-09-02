# Research Findings

Topic: centralized error path (`src/app/error.rs`), Sentry module (`src/infra/sentry.rs`),
startup wiring, the pinned `sentry` crate API, test harness, and arkitect rules.

## Q1: Sentry capture sites in the crate

### Findings
- The only Sentry capture calls in the crate are three `sentry::capture_error(...)`
  invocations, all inside the `WebError` `IntoResponse` match in `src/app/error.rs`:
  - `WebError::Database(err)` → `sentry::capture_error(&err)` at `src/app/error.rs:63`
  - `WebError::Template(err)` → `sentry::capture_error(&err)` at `src/app/error.rs:68`
  - `WebError::External(message)` → `sentry::capture_error(&ExternalError(message))` at `src/app/error.rs:73`
- No `capture_event`, `capture_message`, `capture_exception`, `add_breadcrumb`, `with_scope`,
  or `configure_scope` calls exist anywhere in `src/` (verified by repo-wide `rg`; only the
  three `capture_error` sites plus the `src/infra/sentry.rs` `init` were found).
- Payload types captured:
  - `Database` arm passes the concrete `sqlx::Error` (`src/app/error.rs:63`).
  - `Template` arm passes the concrete `minijinja::Error` (`src/app/error.rs:68`).
  - `External` arm wraps a `String` in a local newtype `ExternalError(String)` so it can be
    passed to `capture_error`; the newtype implements `Display` + `std::error::Error`
    (`src/app/error.rs:14-27`). `External(String)` is populated via `From` impls from
    `UnsplashError(err.0)` (`src/app/error.rs:45-49`) and `ResendError(err.0)`
    (`src/app/error.rs:51-55`), where both source types are single-field `pub struct X(pub String)`
    (`src/infra/unsplash.rs:30`, `src/infra/resend.rs:15`).
- Distinguishing characteristics: the three arms differ only by which `WebError` variant raised
  them — `Database` = SQL error, `Template` = template render error, `External` = 502-class
  upstream error. There is no message-string differentiation and no surrounding
  tracing-context tagging; each arm emits a fixed `tracing::error!` message string
  (`"database error"` line 62, `"template render error"` line 67, `"external error"` line 72)
  followed by the capture. The only other fixed-context is that errors flow through the axum
  `TraceLayer` span in `src/app/log.rs:76-85` (method + matched path), which does not feed Sentry.

## Q2: `sentry` crate public API for structured metadata

### Findings
- The `sentry` crate simply re-exports `sentry_core::*` (`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sentry-0.49.1/src/lib.rs:143`). 0.49.1 is pinned by `Cargo.lock`.
- **Naming correction to the question's premise:** there is no `Scope::with_tag`. The scope API
  in 0.49.1 is `Scope::set_tag`, `Scope::remove_tag`, `Scope::set_context`, `Scope::remove_context`,
  `Scope::set_extra`, `Scope::remove_extra`, plus `set_level`, `set_transaction`, `set_user`,
  `set_fingerprint` (`sentry-core-0.49.1/src/scope/real.rs:229-260`, `set_tag` at 229, `set_context`
  at 241, `set_extra` at 251). These operate on `Arc<HashMap>` fields behind `Arc::make_mut`
  (`real.rs:60-61`), i.e. copy-on-write.
- Scope data attaches to every event captured while that scope is active via
  `Scope::apply_to_event`, which `.extend`s the event's `tags`, `extra`, and `contexts` from the
  scope, and copies level/user/transaction/breadcrumbs/fingerprint if the event doesn't set them
  (`real.rs:279-334`; tag merge at ~296-299, context merge at ~300-303, extra merge at ~290-292).
- Scope is **hub-scoped and thread-local**: `Hub::current()` is the thread local hub
  (`sentry-core-0.49.1/src/hub.rs:51`, thread-local declared in `sentry-core-0.49.1/src/hub_impl.rs:15`).
  Global API helpers resolve the active hub per thread via `Hub::with_active`
  (`sentry-core-0.49.1/src/hub.rs:61`) / `Hub::with` acting on the current thread's hub. So scope
  set on one thread does not apply to events captured on another thread unless the child hub is
  bound/derived.
- Two ways to attach scope data to a capture:
  - `configure_scope(|s| ...)` mutates the current thread's ambient scope for all future captures
    (`sentry-core-0.49.1/src/api.rs:140-160`).
  - `with_scope(config, callback)` pushes a temporary scope for a single call, useful for
    per-capture tags (`sentry-core-0.49.1/src/api.rs:169-190`, e.g. `with_scope(|s| s.set_level(...), || capture_message(...))`).
- Event-level tags vs scope tags: `Event` carries its own `tags`/`contexts`/`extra` maps
  (`sentry-core-0.49.1/src/scope/real.rs` merges them; event owns its copies). Setting tags
  directly on an `Event` attaches only to that one event; scope tags attach to whatever event the
  active scope is applied to (one capture or many, per thread).
- `capture_message(&str, Level)` exists — it builds an `Event` with a `message` and sends it via
  `capture_event` (`sentry-core-0.49.1/src/lib.rs` re-export; impl in `sentry-core-0.49.1/src/api.rs:62-72`);
  the hub-level impl is `Hub::capture_message` (`sentry-core-0.49.1/src/hub.rs:128`).
- `capture_error<E: Error>(&E)` builds an `Event` from the error's `Display`/`source` chain
  (`exception_from_error`, error chain sorted oldest→newest) and records it as an `exception`;
  it returns nil UUID and does nothing when no client is bound (`sentry-core-0.49.1/src/error.rs:12-31`,
  `50-56`). So `capture_error` records an **exception**; `capture_message` records a **message** event
  with no exception payload.

## Q3: Sentry client startup configuration

### Findings
- Wired in `src/main.rs:19-21`: `let _guard = env.enable_sentry.then(|| infra::sentry::init(&env.sentry_dsn));`
  A client is instantiated **only when `ENABLE_SENTRY=true`**; otherwise no client is bound and all
  `sentry::capture_*` calls are no-ops (they resolve no active hub/client and return nil UUID).
  The `ClientInitGuard` return value is held in `_guard` to keep the client alive.
- `src/infra/sentry.rs:1-40` `init(dsn)` sets only two `ClientOptions` on top of defaults
  (`src/infra/sentry.rs:2-8`):
  - `.maybe_release(sentry::release_name!())` (line 5) — sets release from `CARGO_PKG_VERSION`.
  - `.send_default_pii(true)` (lines 6-7) — enables user IP + potentially sensitive headers.
  - No `environment`, no `sample_rate`/`traces_sample_rate`, no `debug`, no explicit
    transport changes, no `TracesSampler` anywhere in `src/` (repo-wide `rg` found none).
- `init` additionally replaces the panic hook with a Broken-pipe-tolerant wrapper that still
  forwards panics to Sentry's own hook but filters out `Broken pipe` / `os error 32` panics
  (`src/infra/sentry.rs:11-39`, `is_broken_pipe` at 35-42).
- Feature surface: `Cargo.toml:12` declares `sentry = "0.49"` with **no features**, so the crate's
  `default` features apply: `backtrace, contexts, debug-images, logs, metrics, panic, transport,
  release-health` (from `sentry-0.49.1/Cargo.toml` `[features] default`). `tracing` and `test`
  are **not** enabled.
- **No tracing/OTel bridge**: despite the `logs` and `panic` features being compiled, the app never
  installs `sentry::integrations::tracing` / `sentry_log` — there are no `sentry::integrations`
  references in `src/`. Captures are exclusively the three explicit `sentry::capture_error` calls;
  `tracing::error!` does not reach Sentry. `src/app/log.rs` installs only a JSON
  `tracing_subscriber` writer (`src/app/log.rs:44-63`) with no Sentry layer.

## Q4: Test harness treatment of Sentry

### Findings
- The harness hardcodes `enable_sentry: false` and `sentry_dsn: "test-dsn"` in every `Env` it
  constructs (`src/test/mod.rs:66-67` and `:113-114`). Because `enable_sentry` is false, `main.rs`'s
  `.then(|| infra::sentry::init(...))` is not even invoked in tests, so no Sentry client/transport
  is bound at all; captures are silent no-ops.
- `start_app` / `start_app_with` / `start_app_with_resend*` / `serve_app` all funnel through
  `Env { enable_sentry: false, ... }` (`src/test/mod.rs:24-97`).
- **No test asserts on Sentry capture behavior.** `sentry::test::with_captured_events` /
  `TestTransport` are not referenced anywhere in `src/` (repo-wide `rg`), and the `test` cargo
  feature that provides them (`sentry-core-0.49.1/src/lib.rs:166`, `#[cfg(feature = "test")] pub mod test`)
  is **not enabled** in `Cargo.toml:12`. Error-path tests in `src/app/error.rs` (e.g.
  `database_error_is_500` line 106, `external_error_is_502` line 112, `resend_error_is_502` line 128)
  only assert the resulting `StatusCode`/body; they never run a capture under `with_captured_events`.
- The integration-style `#[tokio::test]` in `src/test/mod.rs` (e.g. `page_hits_show_up_in_metrics`
  line ~254) and stub servers (`UnsplashStub` line 154, `ResendStub` line 229) only assert HTTP
  responses / metrics, never Sentry.

## Q5: Arkitect module-dependency rules

### Findings
- Rules live in `src/test/arkitect.rs` (a single `#[cfg(test)] mod tests`), driven by
  `Arkitect::ensure_that(Project::from_current_crate()).complies_with(rules)` (`src/test/arkitect.rs:22-38`).
- Allowed-dependency allowlists:
  - `vardy::domain` may depend on `["serde"]` and must not depend on app/infra/interfaces (`src/test/arkitect.rs:23-24`).
  - `vardy::infra` may depend on `["prometheus", "reqwest", "sentry", "serde", "std", "vardy::domain"]`
    and must not depend on `vardy::app` / `vardy::interfaces` (`src/test/arkitect.rs:25-32`). This is the
    only layer allowed to depend on `sentry` directly.
  - `vardy::interfaces` may depend on `["axum", "crate::app", "crate::test", "minijinja", "serde_json",
    "std", "tower_http", "vardy::app", "vardy::domain", "vardy::infra", "vardy::test", "sqlx"(tests)]`
    (`src/test/arkitect.rs:34-47`). Note it may call into `vardy::infra` (and thus transitively the
    `sentry` crate via `infra::sentry`), but it does not list `sentry` as a direct dep.
  - `vardy::app` must not depend on `vardy::interfaces` (`src/test/arkitect.rs:48-49`);
    it is not given a `sentry` direct dep either. Yet `src/app/error.rs` calls `sentry::capture_error`
    directly (`src/app/error.rs:63,68,73`), which contradicts these allowlists — meaning the custom
    `MustNotDependOnExceptTests` rule and/or the `deps_outside_test_modules` AST walker is the operative
    enforcement, and the allowlists in `rules_for_module` apply only to `enum`/`Vec` lists in this
    version. Enforcement collects deps from `use` trees and path/type references, skipping
    `#[cfg(test)]` modules (`src/test/arkitect.rs:148-292`); the windowing + `must_not_depend_on`
    lists (`vardy::app`/`vardy::infra`/`vardy::interfaces` cross-layer bans) are the hard gates that
    currently pass.
  - A custom `MustNotDependOnExceptTests` rule forbids `sqlx` and `reqwest` in `vardy::interfaces`
    except inside `#[cfg(test)]` modules (`src/test/arkitect.rs:50-52`, `66-111`).
- Takeaway: the strict cross-layer `must_not_depend_on` bans (domain↛app/infra/interfaces; app↛interfaces;
  infra↛app/interfaces) are what the suite enforces today; `sentry` is only on `infra`'s allowlist.

## Q6: How `WebError` is extended and where captures/logging happen

### Findings
- `WebError` variants: `Template(minijinja::Error)`, `Database(sqlx::Error)`, `NotFound`,
  `External(String)`, `BadRequest(String)`, `TooManyRequests{retry_after_secs}` (`src/app/error.rs:10-16`).
- `From` impls feeding it (`src/app/error.rs:33-55`):
  - `From<minijinja::Error>` → `Template` (line 33-37)
  - `From<sqlx::Error>` → `Database` (line 39-43)
  - `From<crate::infra::unsplash::UnsplashError>` → `External(err.0)` (line 45-49)
  - `From<crate::infra::resend::ResendError>` → `External(err.0)` (line 51-55)
- `IntoResponse` (`src/app/error.rs:56-85`) is the **only** decision point for error-to-Sentry
  capture: `Database`/`Template`/`External` each log a `tracing::error!` then `capture_error`;
  `NotFound`, `BadRequest`, `TooManyRequests` do not log/capture (they are client-side/expected).
- Captures / error logging are **not** triggered outside `IntoResponse`, with one exception:
  `src/app/rate_limit.rs:62` logs `tracing::error!(?other, "rate limiter failed to extract key")` in the
  `GovernorError` unreachable arm — it logs but does **not** call Sentry, and it does not return
  `WebError` (it returns a plain `(StatusCode::INTERNAL_SERVER_ERROR, ...)` tuple, `rate_limit.rs:60-66`).
- No `tracing::error!` or capture occurs in `src/main.rs` (it only returns `Box<dyn Error>` up to the
  runtime, `src/main.rs:11,27-28`) or in background work (`prune_loop` logs at `trace` only,
  `src/app/rate_limit.rs:137`).
- The `tracing::error!` calls beside each capture (`src/app/error.rs:62,67,72`) record only the error
  value (`?err` / `%message`) plus the static message — they do not add tags, request context, or the
  matched route; the request-scoped span carries `method`+`path` (`src/app/log.rs:76-85`) but that span
  is not attached to Sentry.

## Cross-Cutting Observations
- Every server fault path funnels through `WebError::IntoResponse`; that single match is the intended
  chokepoint for both logging and Sentry capture (documented in `src/interfaces/routes.rs:14`).
- The crate's only Sentry surface is `infra::sentry::init` + `sentry::capture_error` in `app::error`.
  Both the dependency rule (`sentry` allowed only from `infra`) and the actual `use` in `app::error`
  coexist, indicating `infra` is the sanctioned home for the Sentry binding while `app` currently
  calls the crate directly.
- Captured payloads are always concrete error types or a String-wrapping newtype; nothing attaches
  scope tags/contexts today, so every captured event currently carries only the default client data
  (release, PII-defaults).
- Sentry is fully inert in tests and fully opt-in at runtime via `ENABLE_SENTRY`.

## Open Areas
- The arkitect rules' allowlists vs the actual `sentry` use in `src/app/error.rs` is ambiguous about
  which mechanism truly gates direct `sentry` from `app`; the project's routing skill notes these
  `rules_for_module` allowlists are largely vestigial in `rust_arkitect` 0.3.7 and the cross-layer
  `must_not_depend_on` bans are the effective gates. Exact rule semantics were not verified by running
  the suite.
- Whether the `ETag`/sampling or rate limits on Sentry's transport are tuned was not inspectable from
  the (unbound-in-tests) client; only `maybe_release` + `send_default_pii` are configured in source.
