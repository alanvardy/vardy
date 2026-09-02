# Implementation Plan

## Overview

Centralize Sentry error capture behind one public function in `src/infra/sentry.rs`
that tags every event with a typed `source` (`database` | `template` | `external`),
migrate the three `WebError::IntoResponse` arms to call it, and remove the direct
`sentry::` dependency from `src/app/error.rs` (resolving the arkitect `sentry`-in-`app`
ambiguity). Build bottom-up: enum + dev-dep → capture function + e2e test → call-site migration.

Build is **bottom-up** and each stage must leave `./scripts/test.sh` green before advancing.

---

## Stage 1: `ErrorSource` enum + `sentry` `test` feature

### Changes

#### 1. `ErrorSource` enum + tag mapping
**File**: `src/infra/sentry.rs`
**Action**: modify

Append below the existing `is_broken_pipe` helper (bottom of file, before no other items —
the file currently has only `init` + `is_broken_pipe`).

```rust
/// Identifies which `WebError` arm triggered a Sentry capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSource {
    Database,
    Template,
    External,
}

impl ErrorSource {
    /// The `source` tag value attached to a captured Sentry event.
    pub fn as_tag(&self) -> &'static str {
        match self {
            ErrorSource::Database => "database",
            ErrorSource::Template => "template",
            ErrorSource::External => "external",
        }
    }
}
```

#### 2. `sentry` `test` dev-dependency
**File**: `Cargo.toml`
**Action**: modify

Add to the existing `[dev-dependencies]` block (feature unifies with the `sentry = "0.49"`
in `[dependencies]`; the `test` feature maps to `sentry-core/test` = `["client", "release-health"]`):

```toml
sentry = { version = "0.49", features = ["test"] }
```

#### 3. Enum mapping unit test
**File**: `src/infra/sentry.rs`
**Action**: modify

Add an inline `#[cfg(test)] mod tests` at the bottom of the file (the file currently has no
test module):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_source_variants_map_correctly() {
        assert_eq!(ErrorSource::Database.as_tag(), "database");
        assert_eq!(ErrorSource::Template.as_tag(), "template");
        assert_eq!(ErrorSource::External.as_tag(), "external");
    }
}
```

### Verification
#### Automated
- [x] `cargo check --all-targets` passes (enum + dev-dep compile cleanly)
- [x] `cargo nextest run --lib` passes — `error_source_variants_map_correctly` is green and no existing tests break

#### Manual
- [ ] `cargo tree -e features -i sentry` (or a plain `cargo check --tests`) shows the `test`
      feature resolving without pulling in a second `sentry` version — confirm `Cargo.lock`
      is unchanged in its `sentry` entry (only feature resolution changes)

---

## Stage 2: `capture_error_with_source` + end-to-end capture test

### Changes

#### 1. Centralized capture function
**File**: `src/infra/sentry.rs`
**Action**: modify

Add immediately after the `ErrorSource` impl (above the `#[cfg(test)]` module):

```rust
/// Captures `err` in Sentry with a `source` tag identifying the error category.
///
/// Centralizes enrichment: every capture site must call this rather than
/// `sentry::capture_error` directly. No-op when no client is bound.
pub fn capture_error_with_source<E: std::error::Error>(err: &E, source: ErrorSource) {
    sentry::with_scope(
        |scope| scope.set_tag("source", source.as_tag()),
        || sentry::capture_error(err),
    );
}
```

Notes:
- `with_scope` (verified in `sentry-core-0.49.1/src/api.rs:184`) pushes a temporary scope
  for this one call; the tag does not leak to later captures on the thread.
- `Scope::set_tag<V: ToString>(&mut self, key: &str, value: V)`
  (`sentry-core-0.49.1/src/scope/real.rs:229`) takes `source.as_tag()` directly.
- `sentry::capture_error<E: Error + ?Sized>(&E)` (`sentry-core-0.49.1/src/api.rs:50`) —
  the generic bound `E: std::error::Error` is sufficient; no `'static`/`Send`/`Sync` needed.

#### 2. End-to-end capture test
**File**: `src/infra/sentry.rs`
**Action**: modify

Add inside the `#[cfg(test)] mod tests` introduced in Stage 1:

```rust
#[test]
fn capture_includes_source_tag() {
    let events = sentry::test::with_captured_events(|| {
        let err = std::io::Error::new(std::io::ErrorKind::Other, "boom");
        capture_error_with_source(&err, ErrorSource::Database);
    });

    assert_eq!(events.len(), 1, "expected exactly one captured event");
    assert_eq!(
        events[0].tags.get("source").map(String::as_str),
        Some("database")
    );
}
```

Notes (verified against pinned sources):
- **No `sentry::init` is needed** — `sentry::test::with_captured_events`
  (`sentry-core-0.49.1/src/test.rs`) creates a `TestTransport`, binds a client to a fresh
  hub, and runs the closure under `Hub::run`. The closure's `capture_error_with_source`
  resolves the active test hub automatically. (This corrects `structure.md`'s sad-path note,
  which claimed `init` was required — calling `sentry::init` inside the closure would bind
  the *global* hub and would not affect the test hub's capture.)
- The event's tags live at `Event.tags: Map<String, String>` (`sentry-types-0.49.1/src/protocol/v7.rs:1723`),
  a `BTreeMap`/`serde_json::Map`; `events[0].tags.get("source")` is the correct accessor
  (there is no `tag_value` accessor on `Event`).
- The sad path is asserted by the `events.len() == 1` check: if the client were somehow not
  bound, no event would be captured and the length assertion would fail.

### Verification
#### Automated
- [x] `cargo nextest run --lib capture_includes_source_tag` passes — one event captured, `source == "database"`
- [x] `cargo nextest run --lib` passes — the Stage 1 enum test still passes alongside the new test

#### Manual
- [ ] Temporarily change the assertion to `Some("wrong")` and confirm the test fails
      (proves the tag assertion is actually reading the captured event, not vacuously passing),
      then revert.

---

## Stage 3: Call-site migration — `app::error` drops direct `sentry::`

### Changes

#### 1. `src/app/error.rs` — replace three `sentry::capture_error` calls
**File**: `src/app/error.rs`
**Action**: modify

Replace the three capture calls in the `IntoResponse` match (keep the `tracing::error!`
lines and the returned `(StatusCode, ...)` tuples unchanged):

```rust
WebError::Database(err) => {
    tracing::error!(error = ?err, "database error");
    crate::infra::sentry::capture_error_with_source(&err, ErrorSource::Database);
    (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
}
WebError::Template(err) => {
    tracing::error!(error = ?err, "template render error");
    crate::infra::sentry::capture_error_with_source(&err, ErrorSource::Template);
    (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
}
WebError::External(message) => {
    tracing::error!(error = %message, "external error");
    crate::infra::sentry::capture_error_with_source(
        &ExternalError(message),
        ErrorSource::External,
    );
    (StatusCode::BAD_GATEWAY, "bad gateway").into_response()
}
```

Add a `use` at the top of the file so the arms read cleanly (this is the only new import):

```rust
use crate::infra::sentry::{capture_error_with_source, ErrorSource};
```

This removes the last direct `sentry::` path reference in non-test code under `vardy::app`.

#### 2. `src/app/error.rs` — update `ExternalError` doc comment
**File**: `src/app/error.rs`
**Action**: modify

**Keep** the `ExternalError` newtype and its `Display`/`Error` impls (they are unit-tested at
`external_error_implements_error` and `external_error_is_send_sync`, and Design Decision #5
requires keeping the typed-error wrapper). Only replace its doc comment, which currently
describes it as a `capture_error`-bound hack:

```rust
/// Typed-error wrapper for `WebError::External`'s `String` payload.
///
/// Lets the `External` arm satisfy the `E: std::error::Error` bound on
/// `capture_error_with_source` while preserving the value as an exception
/// (with its error chain) in Sentry rather than a message event.
#[derive(Debug)]
struct ExternalError(String);
```

> Note: `structure.md` Stage 3 first says "Remove the `struct ExternalError`…" and later
> "`ExternalError` **stays**". The design's Decision #5 is authoritative — the newtype stays.

### Verification
#### Automated
- [ ] `./scripts/test.sh` passes end-to-end (format → sqlx prepare → check → CSS build/drift → clippy → tests → TODO grep)
- [ ] Existing `src/app/error.rs` tests all pass unmodified — they assert `StatusCode`/body only, never capture behavior
- [ ] `src/test/arkitect.rs` `test_architectural_rules` passes — `vardy::app` no longer references `sentry::` outside `#[cfg(test)]`; the AST walker (`deps_outside_test_modules`) reports zero `sentry` dependency from `app`

#### Manual
- [ ] `rg "sentry::" src/app` returns no matches outside test code (only `crate::infra::sentry::…` calls remain)
- [ ] `rg "capture_error" src` shows exactly four sites: the single wrapper inside
      `capture_error_with_source` (in `infra::sentry`) plus the three migrated call sites
      in `app::error` — none of which invoke `sentry::capture_error` directly

---

## Testing Checkpoints

| Stage | Checkpoint — `./scripts/test.sh` must be green after… |
|-------|--------------------------------------------------------|
| 1     | Enum + dev-dep added, `error_source_variants_map_correctly` passes |
| 2     | `capture_error_with_source` added, `capture_includes_source_tag` passes |
| 3     | Call-sites migrated, all existing tests + arkitect pass |

If any stage fails, fix only within that stage's files before advancing.
Stages 1 and 2 are independently valuable — the enum and function can land on their own
even if Stage 3 needs rework.

## Deviations from `structure.md`

1. **Stage 2 sad-path note corrected**: `sentry::test::with_captured_events` auto-binds a
   client + `TestTransport` via `Hub::run`; the test does **not** call `sentry::init` inside
   the closure (and must not — it would bind the global hub, not the test hub).
2. **Stage 3 `ExternalError` contradiction resolved**: the newtype is **kept** (per Design
   Decision #5), and only its doc comment is rewritten.

No other deviations; file set, enum shape, function signature, tag values, and call-site
mapping all follow `structure.md`/`design.md` as written.
