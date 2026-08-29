# Design Document: Capture WebError::External (502) to Sentry

## Current State

The `WebError` enum in `src/app/error.rs:10-16` centralizes all handler errors
through a single `IntoResponse` chokepoint (`:42-69`). Five arms:

| Arm | Sentry? | Status |
|-----|---------|--------|
| `Database(sqlx::Error)` (`:12`) | `capture_error(&err)` at `:48` | 500 |
| `Template(minijinja::Error)` (`:11`) | `capture_error(&err)` at `:53` | 500 |
| `External(String)` (`:14`) | **none** — `tracing::error!` only (`:57`) | 502 |
| `NotFound` (`:13`) | none | 404 |
| `TooManyRequests { retry_after_secs }` (`:15`) | none | 429 |

The `External` arm's payload is a plain `String`, not an `Error`-implementing
type, so `sentry::capture_error` cannot accept it directly. A comment at
`:60` documents the intent: "Client fault, like `External`: log nothing to
Sentry" — but 502 (Bad Gateway) is a server-side failure, not a client fault.

`External` variants are constructed via two `From` impls (`:30-40`) that move
error strings verbatim from `ResendError(pub String)` (`src/infra/resend.rs:14-15`)
and `UnsplashError(pub String)` (`src/infra/unsplash.rs:29-30`). The strings carry
upstream identity + failure stage (transport, non-2xx, parse).

Sentry is initialized at `src/main.rs:19-21`, gated on `env.enable_sentry`. In
tests, `Env` sets `enable_sentry: false` (`src/test/mod.rs:66-67`), so no
Sentry client is ever bound during testing. The arkitect rules
(`src/test/arkitect.rs`) impose no restriction on `app` calling the `sentry`
crate — the current `Database`/`Template` calls at `error.rs:48,53` already
pass the arkitect test.

## Desired End State

The `External` arm captures to Sentry with the same one-liner pattern as
`Database` and `Template`, by wrapping the `String` in an
`Error`-implementing newtype and calling `sentry::capture_error`.

**Verification**: The existing `external_error_is_502` test (`error.rs:95`)
still passes (status remains 502). The `Database`/`Template` arms are
unchanged. `tracing::error!` is still emitted — the arm has parity with the
other two capturing arms.

## Patterns to Follow

- **Colocated error capture** — `sentry::capture_error` lives in the
  `IntoResponse` match arm alongside `tracing::error!`, not in a separate
  function or layer. Follow the existing pattern at `error.rs:46-55` (both
  `Database` and `Template` arms do log + capture in adjacent lines).

- **Error-implementing wrapper** — The codebase already wraps external errors
  for `From` conversion (`src/app/error.rs:30-40`). A small newtype
  implementing `std::error::Error` follows the same local-wrapper pattern.

- **Test assertions stay status-only** — No existing test asserts on Sentry
  capture behavior; the harness disables Sentry globally (`src/test/mod.rs:66-67`).
  The `external_error_is_502` test (`error.rs:95`) covers the arm; we do not
  introduce new Sentry-specific test assertions.

- **DO NOT** introduce a capture wrapper in `src/infra/sentry.rs` for this
  PR. That file is currently `init` + panic-hook only (`sentry.rs:1-48`).
  Centralized capture enrichment belongs in a follow-up ([VAR-741]).

## Design Decisions

1. **Wrap `String` in `ExternalError` newtype, call `capture_error`**:
   Chosen over `capture_message` (which puts text in `message` instead of
   structured `exception` values — inconsistent with `Database`/`Template`)
   and over `capture_event` (more code for no benefit). The newtype is ~6
   lines: `#[derive(Debug)] struct ExternalError(String);` +
   `impl Display` + `impl Error`. This matches the existing pattern at
   `error.rs:48,53` of passing `&err` to `capture_error`.

2. **Keep `tracing::error!`**: The `Database` and `Template` arms both log
   AND capture (`error.rs:47-48, 52-53`). Dropping the log for `External`
   would diverge from the established arm pattern. Tracing output remains
   useful for local debugging independent of Sentry.

3. **No Sentry-capture test assertions**: Consistent with the codebase — zero
   existing tests verify Sentry behavior, and all tests disable Sentry
   (`src/test/mod.rs:66-67`). Asserting only HTTP status (502) matches
   `database_error_is_500` (`error.rs:89`) and `template_error_is_500`
   (`:82`).

4. **Source enrichment deferred to [VAR-741]**: Adding `source=resend`,
   `source=unsplash`, `source=database`, `source=template` tags across all
   Sentry capture sites needs a centralized function in `src/infra/sentry.rs`.
   That work changes every capture call site and is best done as a dedicated
   follow-up, not bundled into this focused External-arm fix.

5. **Newtype lives in `src/app/error.rs`**: The `ExternalError` newtype is
   tightly coupled to the `WebError` enum and its `IntoResponse` impl. Placing
   it alongside `WebError` follows the codebase convention of colocating
   error-related types (`src/app/error.rs:1-69` already holds `WebError`, all
   its `From` impls, and `IntoResponse`).

## What We're NOT Doing

- **NOT** extracting a shared Sentry capture helper — that's [VAR-741].
- **NOT** changing `Database` or `Template` arms — they're already correct.
- **NOT** adding Sentry-scope enrichment (tags, breadcrumbs, user context)
  to the `External` arm — [VAR-741].
- **NOT** introducing Sentry test assertions — matches existing convention.
- **NOT** changing the `WebError` variant from `External(String)` to
  `External(ExternalError)` — the inner type change would ripple through
  `From` impls (`error.rs:32,38`) and handler-level tests that match on
  `WebError::External` (`src/app/picture.rs:318`). Wrapping inline in the
  match arm is minimally invasive.
- **NOT** changing the comment at `error.rs:60` — the comment will be
  removed since `External` is no longer "client fault, log nothing to
  Sentry."

## Open Risks

- **String vs structured data**: The `External` payload is a free-text
  `format!` string from `src/infra/resend.rs` and `src/infra/unsplash.rs`.
  Sentry's `capture_error` with `ExternalError` will put the string into
  `exception` values where grouping/deduplication may be less effective than
  if the error had a stable `Display` prefix. Mitigation: the existing
  strings already follow a consistent template (`"resend request failed:
  {e}"`, etc.), so Sentry grouping by exception type + message prefix should
  work reasonably well.

- **No handler-level `Template` Sentry coverage**: Research noted
  (`research.md` "Open Areas") that no handler-level test forces a template
  render failure through the `Template` arm's capture path — only unit-level
  (`error.rs:82`). This is an existing gap unrelated to this change, but any
  future refactor of the error path should add handler-level coverage.

---

Next: run `/4_structure`