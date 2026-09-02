# Structure Outline

## Approach

Centralize Sentry error capture behind a single public function in `src/infra/sentry.rs`
that attaches a typed `source` tag to every event, then migrate the three `IntoResponse`
arms to call it — resolving the arkitect `sentry`-in-`app` ambiguity. Build bottom-up:
enum + dev-dep → capture function + e2e test → call-site migration.

---

## Stage 1: `ErrorSource` enum + `sentry` `test` feature

The lowest layer: define the closed variant set that tags each capture, and wire the
dev-dependency that makes the Stage 2 e2e test possible. No behavioral change yet —
the enum is pure data and can be tested in isolation.

**Files**: `src/infra/sentry.rs`, `Cargo.toml`

**Key changes**:
- `pub enum ErrorSource { Database, Template, External }` — new type with a `&'static str`
  mapping per variant (e.g. `Database` → `"database"`, `Template` → `"template"`,
  `External` → `"external"`).
- `[dev-dependencies]`: add `sentry = { version = "0.49", features = ["test"] }`
  — enables `sentry::test::with_captured_events` for the Stage 2 capture test.

**Tests**:
- `src/infra/sentry.rs` `#[cfg(test)]`: `error_source_variants_map_correctly` — unit test
  that each `ErrorSource` discriminant maps to its expected `&'static str`.

**Verify**: `./scripts/test.sh` passes — the new enum and dev-dep compile cleanly, the
enum test is green, and no existing tests break.

---

## Stage 2: `capture_error_with_source` function + end-to-end capture test

Builds on Stage 1. The single public function that wraps `sentry::capture_error` with
`with_scope` → `scope.set_tag("source", …)`. Proves the tag lands on the real event via
the `test` transport.

**Files**: `src/infra/sentry.rs`

**Key changes**:
- `pub fn capture_error_with_source<E: std::error::Error>(err: &E, source: ErrorSource)`
  — calls `sentry::with_scope(|scope| scope.set_tag("source", source.as_tag()), || sentry::capture_error(err))`.
  Returns nothing (like `capture_error`); no-op when no client is bound.
- `ErrorSource::as_tag(&self) -> &'static str` — public method returning the string mapping.

**Tests**:
- `src/infra/sentry.rs` `#[cfg(test)]`: `capture_includes_source_tag` — runs
  `sentry::test::with_captured_events(|events| { … })`, calls
  `capture_error_with_source(…, ErrorSource::Database)`, asserts exactly one event was
  captured and that `events[0].tags["source"]` equals `"database"` (using the
  `sentry-types` accessor appropriate for 0.49.1).
  > Sad-path note: `sentry::test::with_captured_events` only works when a client is
  > bound. The test must call `sentry::init(sentry::ClientOptions::default())` inside the
  > closure (the `test` transport auto-binds). If `init` is omitted, no event is captured
  > and the assertion on `events.len()` catches it.

**Verify**: `./scripts/test.sh` passes — the capture function compiles, the e2e test is
green, and the Stage 1 enum test continues to pass.

---

## Stage 3: Call-site migration — `app::error` drops direct `sentry::`

Builds on Stage 2. Replace the three `sentry::capture_error` calls in
`IntoResponse` with `crate::infra::sentry::capture_error_with_source(…, ErrorSource::…)`.
No new behavior — same errors captured, same status codes returned, same logging. The
capture just gains a `source` tag and the arkitect `sentry`-in-`app` dependency is
removed.

**Files**: `src/app/error.rs`

**Key changes**:
- Remove the `struct ExternalError` and its `Display` + `Error` impls (lines 14–27)
  — no longer needed since the centralized function is generic over `E: Error` and
  `External` will now pass `&ExternalError(message)` through the same path.
- Replace three `sentry::capture_error(…)` calls:
  - `WebError::Database(err)` → `crate::infra::sentry::capture_error_with_source(&err, ErrorSource::Database)`
  - `WebError::Template(err)` → `crate::infra::sentry::capture_error_with_source(&err, ErrorSource::Template)`
  - `WebError::External(message)` → `crate::infra::sentry::capture_error_with_source(&ExternalError(message), ErrorSource::External)`
- `ExternalError` **stays** — the design decided to keep it (typed error model, error-chain preservation).
  The doc comment on `ExternalError` (line 20–22) is updated to describe it as a typed-error
  wrapper rather than a `capture_error`-bound hack.

**Tests**:
- Existing `src/app/error.rs` tests all pass without modification — they assert
  `StatusCode`/body, never capture behavior, and Sentry is inert in tests.
- The arkitect test (`src/test/arkitect.rs`) continues to pass — `app` no longer
  imports or references `sentry::` directly; the AST walker confirms zero `sentry`
  dependency in non-test code.

**Verify**: `./scripts/test.sh` passes — all tests green, arkitect clean, CSS drift
clean, no forgotten TODOs.

---

## Testing Checkpoints

| Stage | Checkpoint — `./scripts/test.sh` must be green after… |
|-------|--------------------------------------------------------|
| 1     | Enum + dev-dep added, enum unit test passes |
| 2     | `capture_error_with_source` added, e2e capture test passes |
| 3     | Call-sites migrated, all existing tests + arkitect pass |

If any stage fails, fix only within that stage’s files before advancing.
Stages 1 and 2 are independently valuable — the enum and function can land
on their own even if Stage 3 needs rework.
