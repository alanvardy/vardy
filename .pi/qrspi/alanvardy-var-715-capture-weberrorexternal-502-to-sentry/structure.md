# Structure Outline

## Approach

Add a small `ExternalError` newtype (wrapping the `String` payload with
`Display` + `Error`) and call `sentry::capture_error(&ExternalError(message))`
in the `WebError::External` arm — following the exact same colocated
log+capture pattern already used by the `Database` and `Template` arms.

---

## Stage 1: `ExternalError` newtype + `Error` impl

Deliver a `std::error::Error`-implementing wrapper so the `External` arm can
pass its `String` payload to `sentry::capture_error` the same way the
`Database` and `Template` arms pass `&err`.

**Files**: `src/app/error.rs`

**Key changes**:
- `struct ExternalError(String)` — newtype, `#[derive(Debug)]`
- `impl Display for ExternalError` — delegates to `.0`
- `impl std::error::Error for ExternalError` — blanket default methods only
- No changes to `WebError` variants, `From` impls, or `IntoResponse` — this
  stage is pure type scaffolding, independently compilable and testable.

**Tests** (in `#[cfg(test)] mod tests` at `src/app/error.rs`):
- `external_error_implements_error` — construct `ExternalError("boom")`,
  verify `.to_string()` returns `"boom"`, verify `&ExternalError` can be
  passed to functions bounded by `impl Error`.
- `external_error_is_send_sync` — verify the wrapper is `Send + Sync`
  (matching the sentry bound).

**Verify**: `cargo nextest run` passes for the `src/app/error.rs` tests.
No other code references `ExternalError` yet — this stage proves the type
compiles and meets the `Error` contract in isolation.

---

## Stage 2: Wire `ExternalError` into the `External` match arm

With the newtype tested and green, add the one-line `sentry::capture_error`
call to the `External` arm, giving it parity with the `Database` and
`Template` arms.

**Files**: `src/app/error.rs`

**Key changes**:
- In `IntoResponse`, inside `WebError::External(message) => { ... }`:
  - Add: `sentry::capture_error(&ExternalError(message));` — after the
    existing `tracing::error!` call, matching the log-then-capture order of
    the other arms (`error.rs:47-48, 52-53`).
  - Remove the now-incorrect comment: `// Client fault, like \`External\`:
    log nothing to Sentry.` (the `TooManyRequests` arm starts on the next
    line; the comment was between `External` and `TooManyRequests`).

**No new tests needed**: the existing `external_error_is_502` test covers
that the `External` arm still returns 502. No Sentry-specific assertion is
required (consistent with the codebase convention — all tests run with
`enable_sentry: false`, and no existing test asserts on capture behavior).

**Verify**: Full test gate (`./scripts/test.sh`) passes — format, check,
clippy, nextest (all targets, including existing error-path tests and
arkitect rules), and the CSS-drift check.

---

## Testing Checkpoints

- [ ] **After Stage 1**: `cargo nextest run` — `ExternalError` unit tests
  green; newtype compiles, `Error` contract satisfied.
- [ ] **After Stage 2**: `./scripts/test.sh` — full gate green, every
  existing error-path test passes, arkitect rules pass.