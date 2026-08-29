# Research Findings

Research conducted against working tree HEAD `4e91ea2` ("Capture WebError::External (502) to Sentry"). All references are `file:line`.

## Q1: WebError `IntoResponse` arm-by-arm — who captures to Sentry, with what payload

### Findings

- Enum definition `src/app/error.rs:10-16`: `Template(minijinja::Error)` (`:11`), `Database(sqlx::Error)` (`:12`), `NotFound` (`:13`, `#[allow(dead_code)]` — only constructed in tests, `:6-8`), `External(String)` (`:14`), `TooManyRequests { retry_after_secs: u64 }` (`:15`). `#[derive(Debug)]` only — **`WebError` does not implement `std::error::Error`** (`:9`).
- `IntoResponse` impl at `src/app/error.rs:42-69`; match is on owned `self` (`fn into_response(self)`, `:43`), so each arm binding moves its payload out of the enum.

| Arm | tracing::error! | sentry::capture_error | Capture payload | HTTP result |
|---|---|---|---|---|
| `NotFound` (`:45`) | no | no | — | 404 "not found" |
| `Database(err)` (`:46-50`) | yes (`:47`, `error = ?err`) | yes (`:48`) | `&sqlx::Error` — `&err`, borrow of the match-bound inner error | 500 "internal server error" |
| `Template(err)` (`:51-55`) | yes (`:52`, `error = ?err`) | yes (`:53`) | `&minijinja::Error` — `&err`, borrow of the match-bound inner error | 500 "internal server error" |
| `External(message)` (`:56-59`) | yes (`:57`, `error = %message`, Display of String) | **no** | — | 502 "bad gateway" |
| `TooManyRequests { retry_after_secs }` (`:61-66`) | no | no | — | 429, `retry-after` header (`:63`), "too many requests" |

- Comment at `:60` ("Client fault, like `External`: log nothing to Sentry") documents the intent: only the two 500-class server arms capture; the 502 `External` and 429 client-fault arms log-only / silent.
- `sentry::capture_error` free function signature: `pub fn capture_error<E: Error + ?Sized>(error: &E) -> Uuid` (vendored `sentry-core-0.49.1/src/error.rs:50`). Both call sites satisfy `E` with `sqlx::Error` and `minijinja::Error`. Because the `External` payload is a `String` and there is no `Error`-implementing wrapper around it, it cannot be passed to `capture_error` as-is.
- **There is no capture wrapper in `src/infra/sentry.rs`** — the file holds `init` (`:1-9`), the panic-hook replacement (`:11-39`), and `is_broken_pipe` (`:43-48`). The only `sentry::capture_error` calls in the entire codebase are `src/app/error.rs:48` and `:53`.

## Q2: Where `WebError::External` is constructed; Resend/Unsplash error strings

### Findings

- Construction sites (exactly two production + two test):
  - `From<UnsplashError> for WebError` — `src/app/error.rs:30-34`, moves `err.0` verbatim into `WebError::External(err.0)` (`:32`).
  - `From<ResendError> for WebError` — `src/app/error.rs:36-40`, same verbatim `.0` move (`:38`).
  - Test constructions: `src/app/error.rs:96` (`WebError::External("boom".into())`); a match-arm pattern only at `src/app/picture.rs:318` (in the random-with-error unit test, `:315-321`).
- Both newtypes are tuple structs with a single `pub String` field: `ResendError(pub String)` at `src/infra/resend.rs:14-15`; `UnsplashError(pub String)` at `src/infra/unsplash.rs:29-30`. Both are `#[derive(Debug)]` only.
- Propagation: infra errors flow up via `?` in `src/app/picture.rs` (`fetch_and_insert`, `:61-66`, calling `fetch_random`) and `src/app/contact.rs` (`send`, `:25-34`); the boundary conversion is only in the two `From` impls. No transformation, re-prefixing, or formatting happens in either impl — the inner string is byte-for-byte the `WebError` payload.

| Path | Exact template | Location | Semantic info embedded |
|---|---|---|---|
| Resend transport | `"resend request failed: {e}"` | `src/infra/resend.rs:40` | upstream identity, failure stage, reqwest error Display (URL/conn/timeout) |
| Resend non-2xx | `"resend returned status {status}: {body}"` | `resend.rs:53-55` | status code + upstream body truncated to **500 chars** (`response.text().await.unwrap_or_default().chars().take(500)`, `:50-52`); empty body → trailing `": "` |
| Resend parse | **none exists** — 2xx short-circuits to `Ok(())` without reading the body (`resend.rs:58`); comment at `:57` documents why | | |
| Unsplash transport | `"unsplash request failed: {e}"` | `src/infra/unsplash.rs:46` | upstream identity, transport failure, reqwest Display |
| Unsplash non-2xx | `"unsplash returned status {}"` | `unsplash.rs:49-51` | status only — **body never read** (differs from Resend) |
| Unsplash parse | `"unsplash response parse failed: {e}"` | `unsplash.rs:58` | upstream identity, parse stage, reqwest/serde error Display |

- After construction the string is consumed only by `tracing::error!(error = %message, ...)` at `src/app/error.rs:57`; the client always sees the static 502 "bad gateway" body — the string is server-side observability only.

## Q3: What `Database`/`Template` arms capture today; sentry 0.49 API surface

### Findings

- Capture today: `Database` passes `&err` where `err: sqlx::Error` (`src/app/error.rs:46-48`); `Template` same with `minijinja::Error` (`:51-53`). Both types implement `std::error::Error`: `sqlx::Error` via `#[derive(Debug, thiserror::Error)]` (vendored `sqlx-core-0.9.0/src/error.rs:30-32`; dep at `Cargo.toml:14`), `minijinja::Error` (`minijinja-2.24.0/src/error.rs:371`; dep at `Cargo.toml:8`).
- Dependency: `sentry = "0.49"` at `Cargo.toml:12` (no feature overrides), resolved `0.49.1` (`Cargo.lock:2685-2687`). Default features activated: `backtrace, contexts, debug-images, logs, metrics, panic, release-health, transport` (transport = `reqwest` + `native-tls` + `tokio`). Disabled: `tracing`, `tower`, `log`, `actix`, `anyhow`, etc.
- Capture APIs available in 0.49.1 (paths under `~/.cargo/registry/src/index.crates.io-*/`):
  - `sentry::capture_message(msg: &str, level: Level) -> Uuid` — `sentry-core-0.49.1/src/api.rs:62`; builds `Event { message: Some(...), level, .. }` (`hub.rs:128-136`). Not feature-gated.
  - `sentry::capture_error<E: Error + ?Sized>(&E) -> Uuid` — `error.rs:50`; delegates to `Hub::with_active` (`error.rs:78-82`), no-op → `Uuid::nil()` when no client bound (`hub.rs:61-76`; `error.rs:11-26`).
  - `sentry::capture_event(Event<'static>) -> Uuid` — `api.rs:41`; `Hub::capture_event` — `hub.rs:113`; `Client::capture_event(event, Option<&Scope>)` — `client/mod.rs:442` (`#[cfg(feature = "client")]`, enabled).
  - `sentry::event_from_error<E: Error + ?Sized>(&E) -> Event<'static>` — `error.rs:82`; walks the error chain into an `Event` whose **`message` stays `None`** — text lives only in `exception` values (`error.rs:94-109`).
  - `sentry::protocol::Event` (`sentry-types-0.49.1/src/protocol/v7.rs:1635`) — **all fields public** (`:1635-1733`), incl. `message: Option<String>` (`:1660-1662`); `Event::default()` (`:1735`, level defaults to `Level::Error`), `Event::new()` (`:1771`), `Event::into_owned()` (`:1776`).
  - **No `.message()` / `.level()` / `.with_*()` builder methods exist on `Event` in 0.49.1** — assembly is via struct literal / field assignment.
  - `Level` enum: `Debug|Info|Warning|Error|Fatal` (`v7.rs:661-673`; default `Info`). `Hub::capture_log` / `Client::capture_log` exist under `logs` feature (enabled), gated on `ClientOptions::enable_logs` (`client/mod.rs:561-565`).

## Q4: Sentry init/config; when a client is active; disabled behavior

### Findings

- App wrapper `src/infra/sentry.rs:1-9`: `sentry::init((dsn, ClientOptions::default().maybe_release(sentry::release_name!()).send_default_pii(true)))`. `release_name!()` expands to `vardy@0.1.0` (`sentry-core-0.49.1/src/macros.rs:17-38`). All other options are defaults (`clientoptions.rs:829-865`): `debug: false`, `traces_sampling_strategy: Disabled` (no performance tracing configured), `event_sampling_strategy: FixedRate(1.0)`, `send_default_pii: true`, `shutdown_timeout: 2s`, `enable_logs: true`. **No `enable_tracing` option exists in 0.49.1** (zero grep matches in crate sources); the `tracing` cargo feature (→ `sentry-tracing`) is not default and **not activated** in this build (`cargo tree -i sentry-tracing` is empty).
- Init is gated at startup: `src/main.rs:19-21` — `env.enable_sentry.then(|| infra::sentry::init(&env.sentry_dsn))`; guard is `Option<ClientInitGuard>` held for process lifetime.
- Env plumbing: `Env` struct field `enable_sentry: bool` (`src/app/env.rs:11`); `SENTRY_DSN` read unconditionally via `get_string_env` — missing/empty **panics at boot** ("must be set and non-empty", `env.rs:38-43`); `ENABLE_SENTRY` via `get_bool_env` accepts only literal `true`/`false`, anything else panics (`env.rs:45-49`). So there is no runtime state in which Sentry runs "unconfigured": either the app boots with `ENABLE_SENTRY=false` (init never called) or with a non-empty DSN + `true`.
- Invalid (non-empty, non-placeholder) DSN with `ENABLE_SENTRY=true` panics inside `sentry::init`: tuple-DSN conversion `.expect()`s `IntoDsn`/`Dsn::from_str` (`sentry-core-0.49.1/src/clientoptions.rs:867-881`; `sentry-types-0.49.1/src/dsn.rs:177-232`).
- Client "active" condition: `Client::is_enabled` = `options.dsn.is_some() && envelope_sender.is_enabled()` (`client/mod.rs:437-438, 651-677`) — true here whenever `ENABLE_SENTRY=true` with a valid DSN.
- Disabled-state behavior of a capture call: `capture_error` → `Hub::with_active` **short-circuits without invoking the closure** when no client is bound (`hub.rs:61-76`; `is_active_and_usage_safe` = `top.client.is_some_and(|c| c.is_enabled())`, `hub_impl.rs:112-118`); even if reached, `Hub::capture_event` returns `Uuid::nil()` when `top.client` is `None` (`hub.rs:113-125`). Net: with `ENABLE_SENTRY=false`, every capture call (existing `error.rs:48,53`, and any future one) is a **silent no-op returning `Uuid::nil()`** — no network, no panic; callers discard the `Uuid` (`error.rs:48,53`).
- **The `External` arm today has no Sentry call at all** (`error.rs:56-59`, log + 502 only).
- No Sentry↔tracing bridge: app tracing is a JSON `tracing_subscriber::fmt` layer with `EnvFilter` `info,tower_http=info` (`src/app/log.rs:43-52`) + `TraceLayer` (`:55-68`), applied at `src/main.rs:55`. The only Sentry runtime hook is the panic integration whose hook the wrapper rewraps (`sentry.rs:11-39`, dropping `Broken pipe`/`os error 32` panics via `is_broken_pipe`, `:43-48`).

## Q5: Test harness vs. Sentry; error-path test assertions

### Findings

- The harness never calls `sentry::init` and never reads the Sentry env vars. `serve_app` builds `Env` directly with `sentry_dsn: "test-dsn"` and `enable_sentry: false` (`src/test/mod.rs:66-67`); the metrics-port harness repeats it (`:113-114`). Hand-built `AppState` unit tests in `src/app/picture.rs` use `sentry_dsn: String::new()` + `enable_sentry: false` (`:206-207, 263-264, 306-307`). Since init exists only at `src/main.rs:20-21` (gated on the flag), no Sentry client ever exists in tests.
- **No test anywhere asserts on Sentry capture behavior** — zero matches in `src/` for `sentry::test`, `with_captured_events`, capture counts, or mocks. The only `sentry` string in a test file is the arkitect infra allowlist entry (`src/test/arkitect.rs:27`, a compile-time dependency rule).
- Error-path tests assert only status codes (and occasionally exact bodies / inner types):
  - Database → 500: unit `database_error_is_500` (`src/app/error.rs:89`); integration `health_returns_500_when_database_is_dead` (`src/interfaces/routes.rs:200-201`, kills pool then `GET /health`). Both arms assert HTTP status; neither observes Sentry.
  - Template → 500: unit `template_error_is_500` only (`src/app/error.rs:82`); no handler-level missing-template test.
  - External → 502: `external_error_is_502` (`src/app/error.rs:95`), `resend_error_is_502` (`:101`), `post_resend_failure_returns_502` (`src/interfaces/handlers/contact/web.rs:127`, via `start_resend_stub` `src/test/mod.rs:237`), `upstream_failure_is_502` (`src/interfaces/handlers/unsplash/json.rs:124`) and `random_upstream_failure_502` (`:328`) via `start_unsplash_stub` (`src/test/mod.rs:179`), `malformed_upstream_json_missing_user_links_is_502` (`json.rs:140`), app-level `random_upstream_failure_returns_error` (`src/app/picture.rs:284`, asserts `Err(WebError::External)`).
  - External failure swallowed → 200 fallback: `index_still_renders_when_wallpaper_fetch_fails` in both `src/interfaces/handlers/home/web.rs:108` and `singlethread/web.rs:143`.
  - Client-fault → 429: `over_limit_requests_get_429_with_exact_body_and_retry_after` (`src/interfaces/routes.rs:160`), `post_too_many_requests_returns_429` (`contact/web.rs:141`).
- `src/app/error.rs` unit tests (`:82-105`) exercise the exact arms that call `sentry::capture_error` (Database 500 at `:89`, Template 500 at `:82`) without issue — confirming the no-op path when no client is bound.

## Q6: Arkitect architectural rules and the app-layer Sentry call

### Findings

- Rule set built in `test_architectural_rules` at `src/test/arkitect.rs:20-63` (`Project::from_current_crate()`, crate `vardy`):
  - `vardy::domain` may depend only on `["serde"]` (`:23,52`); must not depend on `["vardy::app", "vardy::infra", "vardy::interfaces"]` (`:51`).
  - `vardy::app` must not depend on `["vardy::interfaces"]` (`:54`). **No `and_it_may_depend_on` allowlist for app** — app may reference any external crate or `vardy::infra`/`domain`.
  - `vardy::infra` may depend on `["prometheus", "reqwest", "sentry", "serde", "std", "vardy::domain"]` (`:24-31,57`); must not depend on app/interfaces (`:56`).
  - `vardy::interfaces` may depend on the `interfaces_deps` allowlist (`:33-47,59`), plus a custom rule forbidding **`sqlx`/`reqwest` outside `#[cfg(test)]`** modules (`:60-62`; logic `MustNotDependOnExceptTests` at `:105-129`, scoped to `vardy::interfaces` files `:106-108`, using `deps_outside_test_modules` `:110-111,147-198`).
- The string `"sentry"` appears exactly once in the file — the infra allowlist at `:27`. There is **no rule restricting the `sentry` crate to infra, and no rule forbidding app (or any layer) from calling sentry**. The app layer's only denial is `vardy::interfaces` (`:54`), which a call to the external `sentry` crate does not cross; the analyzer records expression paths via `visit_expr_path` (`:359-367`) so the dependency is visible to the rules and permitted.
- Current code: `src/app/error.rs:48` and `:53` call `sentry::capture_error` from `vardy::app`, which already depends on `vardy::infra` via the `From` impls importing `crate::infra::unsplash::UnsplashError` (`:30-33`) and `crate::infra::resend::ResendError` (`:36-39`). The arkitect test passes on the current tree (verified by running `cargo test test_architectural_rules`; the sqlx `query!` macros required a migrated local SQLite DB first).
- Layered direction encoded: `interfaces → app → infra → domain`, dependencies point inward only; `interfaces` may also reach `infra`/`domain` directly (`:41-42`); `domain` is the bottom layer (`serde` only).

## Cross-Cutting Observations

- All five `WebError` arms funnel through one `IntoResponse` chokepoint (`src/app/error.rs:42-69`), which is also the only place `sentry::capture_error` is called — captures and the HTTP response are colocated per arm.
- The external-error string payloads are all built by `format!` in `src/infra/*` and moved verbatim through two `From` impls (`error.rs:30-40`) to the one log line (`error.rs:57`); every string carries upstream identity + failure stage, and Resend additionally embeds a 500-char body snippet while Unsplash does not read bodies (`resend.rs:53-55` vs `unsplash.rs:49-51`).
- Sentry is deliberately decoupled from the rest of the app: init confined to `main.rs:19-21`, wrapper in `src/infra/sentry.rs`, and the arkitect rules do not force Sentry usage to live in infra (the `sentry` allowlist entry is permissive, not exclusive).
- Every test-built `Env` disables Sentry (`src/test/mod.rs:66-67,113-114`, `picture.rs:206-207,263-264,306-307`), and no test asserts on capture behavior — error-path assertions are status/body-only, exercising the capturing arms indirectly.
- The sentry crate API surface as configured (default features, no `tracing`/`tower` features) offers `capture_message`/`capture_error`/`capture_event` free functions plus public-field `protocol::Event`; captured errors produce events with `message: None` and the error text in `exception` values (`sentry-core-0.49.1/src/error.rs:94-109`).

## Open Areas

- The vendored-source line references for sentry internals (e.g. `sentry-core-0.49.1/...`) were taken from the local registry copy; they describe crate internals, not app code, and could shift in future crate versions.
- The claim that an invalid non-placeholder DSN panics at boot under `ENABLE_SENTRY=true` is derived from reading `Dsn::from_str` + `.expect` in the vendored crate; it was not exercised at runtime (all tests run with the DSN feature disabled).
- No handler-level test forces a template-render failure through the `Template` arm's Sentry capture (`src/app/error.rs:53`) — coverage of that arm is unit-level only (`error.rs:82`).