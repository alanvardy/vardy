# Implementation Plan

## Overview

Add an `ExternalError` newtype wrapping `String` with `Display` + `Error` impls,
then call `sentry::capture_error(&ExternalError(message))` in the `External`
match arm — giving it Sentry parity with the `Database` and `Template` arms.

---

## Phase 1: `ExternalError` newtype + `Error` impl

### Changes

#### 1. Add `ExternalError` struct with `Display` and `Error` impls

**File**: `src/app/error.rs`
**Action**: modify — insert newtype and impls between the `WebError` enum and the `From` impls

```rust
/// Newtype wrapper so `WebError::External(String)` can be passed to
/// `sentry::capture_error` the same way the `Database` and `Template`
/// arms pass their inner error types.
#[derive(Debug)]
struct ExternalError(String);

impl std::fmt::Display for ExternalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ExternalError {}
```

**Placement**: Insert a blank line after the closing `}` of `pub enum WebError { … }` (line 16), then the newtype block, then a blank line before `impl From<minijinja::Error> for WebError`.

#### 2. Add unit tests for `ExternalError`

**File**: `src/app/error.rs`
**Action**: modify — add two test functions inside the existing `#[cfg(test)] mod tests` block

```rust
#[test]
fn external_error_implements_error() {
    let err = ExternalError("boom".into());
    assert_eq!(err.to_string(), "boom");

    // Bound-check: &ExternalError satisfies `impl Error`.
    fn assert_error(_: &dyn std::error::Error) {}
    assert_error(&err);
}

#[test]
fn external_error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ExternalError>();
}
```

**Placement**: Insert after the existing `sqlx_error_converts_via_from` test (after line ~131 in the current file), before the closing `}` of `mod tests`.

### Verification

#### Automated
- [x] `cargo nextest run` — `external_error_implements_error` and `external_error_is_send_sync` pass; no other tests affected

#### Manual
- [ ] Confirm the new type compiles and is not referenced by any other code (grep `ExternalError` in `src/` returns only the new definition + tests)

---

## Phase 2: Wire `ExternalError` into the `External` match arm

### Changes

#### 1. Add `sentry::capture_error` call to the `External` arm

**File**: `src/app/error.rs`
**Action**: modify — insert one line into the `WebError::External(message)` arm

**Current** (`IntoResponse` match arm):
```rust
            WebError::External(message) => {
                tracing::error!(error = %message, "external error");
                (StatusCode::BAD_GATEWAY, "bad gateway").into_response()
            }
```

**New**:
```rust
            WebError::External(message) => {
                tracing::error!(error = %message, "external error");
                sentry::capture_error(&ExternalError(message));
                (StatusCode::BAD_GATEWAY, "bad gateway").into_response()
            }
```

#### 2. Remove the now-incorrect comment

**File**: `src/app/error.rs`
**Action**: modify — delete the comment line between the `External` arm and the `TooManyRequests` arm

**Delete**:
```
            // Client fault, like `External`: log nothing to Sentry.
```

The comment is no longer accurate — `External` now captures to Sentry, and `TooManyRequests` remains the only arm that doesn't.

### Verification

#### Automated
- [x] `./scripts/test.sh` — full gate passes: format, sqlx prepare, check, CSS build + drift check, clippy, nextest (all targets), and forgotten-TODOs grep

#### Manual
- [ ] `cargo nextest run` — verify the existing `external_error_is_502`, `resend_error_is_502`, `post_resend_failure_returns_502`, `upstream_failure_is_502`, `random_upstream_failure_502`, and `malformed_upstream_json_missing_user_links_is_502` tests all still pass (confirm no status-code regressions)
- [ ] `cargo test test_architectural_rules` — arkitect rules still pass (the `sentry` crate call in `vardy::app` remains permitted)
- [ ] Visual inspection: the `External` arm now has `tracing::error!` followed by `sentry::capture_error` on adjacent lines, matching the `Database` and `Template` arm patterns exactly