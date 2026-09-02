# Design Discussion

## Current State

Every server fault funnels through `WebError::IntoResponse` (`src/app/error.rs:56-85`) —
the single chokepoint for both `tracing::error!` logging and Sentry capture
(`src/interfaces/routes.rs:14`). Today exactly three arms capture to Sentry, each
inlining its own `sentry::capture_error` call:

- `WebError::Database(err)` → `sentry::capture_error(&err)` (`src/app/error.rs:63`)
- `WebError::Template(err)` → `sentry::capture_error(&err)` (`src/app/error.rs:68`)
- `WebError::External(message)` → `sentry::capture_error(&ExternalError(message))` (`src/app/error.rs:73`)

The `External` arm wraps its `String` in a local `ExternalError` newtype
(`src/app/error.rs:14-27`) purely so it satisfies `sentry::capture_error`'s
`E: std::error::Error` bound. `NotFound`, `BadRequest`, and `TooManyRequests` are
expected/client-side faults and deliberately log/capture nothing (`src/app/error.rs:76-89`).

**No enrichment exists today.** Every captured event carries only default client data
(release + PII defaults, `src/infra/sentry.rs:2-8`). No tags, no contexts, no
request metadata — and the request-scoped tracing span (`src/app/log.rs:76-85`) is
never bridged to Sentry (no `sentry::integrations` anywhere in `src/`).

Two structural facts shape the design:

1. **Arkitect layering**: `vardy::infra` is the only layer allowed a direct `sentry`
   dependency (`src/test/arkitect.rs:25-32`), yet `src/app/error.rs` calls
   `sentry::capture_error` directly today — an unresolved ambiguity the research
   flagged. `app` *may* depend on `infra` (it already imports
   `crate::infra::unsplash` / `crate::infra::resend`, `src/app/error.rs:45-55`).
2. **Sentry is inert in tests and opt-in at runtime**: every test `Env` sets
   `enable_sentry: false` (`src/test/mod.rs:66-67,113-114`), and `main.rs` only
   calls `init` when `ENABLE_SENTRY=true` (`src/main.rs:19-21`). The `sentry` crate's
   `test` feature (which provides `sentry::test::with_captured_events`) is not
   enabled (`Cargo.toml:12`), and no test asserts capture behavior.

## Desired End State

A single public function in `src/infra/sentry.rs` is the **only** place that
attaches enrichment to a Sentry error capture. All three `IntoResponse` arms call it,
and any future `WebError` arm must go through it. Concretely:

- `src/infra/sentry.rs` gains a public `capture_error_with_source` function and a
  public `ErrorSource` enum.
- Each capture is tagged with `source = "database" | "template" | "external"`,
  identifying the `WebError` arm that raised it.
- `src/app/error.rs` drops its direct `sentry::` import and calls
  `crate::infra::sentry::capture_error_with_source(...)` instead — resolving the
  arkitect ambiguity noted above.
- One test verifies the tag actually lands on the captured event (via the `test`
  transport), proving the wiring end-to-end rather than only the enum value.

## Patterns to Follow

- **Centralized capture through `infra::sentry`** — matches the sanctioned home for
  the Sentry binding (`src/test/arkitect.rs:25-32`) and the existing
  `infra::sentry::init` entry point (`src/infra/sentry.rs:1`).
- **Per-capture scope via `sentry::with_scope`**, not `configure_scope`: `with_scope`
  pushes a temporary scope for one call (`sentry-core-0.49.1/src/api.rs:169-190`),
  while `configure_scope` mutates the thread-local ambient hub and would leak the
  `source` tag onto every later capture on that thread (`src/api.rs:140-160`,
  `hub.rs:51`). The tag must attach to exactly one event.
- **Typed enum for the closed variant set** — mirrors `WebError` itself
  (`src/app/error.rs:10-16`): a compile-time-constrained discriminant, not a free
  string.
- **Newtype-wrapper pattern for `String` payloads** (`ExternalError`,
  `src/app/error.rs:14-27`) — keep it; it is already unit-tested
  (`src/app/error.rs:151-156`).
- **Inline `#[cfg(test)] mod tests`** at the bottom of the source file
  (`src/infra/sentry.rs`), per project convention.

**Pattern NOT to follow**: the current per-arm `sentry::capture_error` inlining in
`src/app/error.rs:63,68,73`. It scatters the Sentry surface across `app` and requires
each future arm to remember to add its own tags. This is exactly what VAR-741 removes.

## Design Decisions

1. **Centralized capture function**: introduce `infra::sentry::capture_error_with_source` —
   the task mandates one public function rather than per-call-site tagging.

2. **Typed `ErrorSource` enum** (Q1 → A): `pub enum ErrorSource { Database, Template, External }`
   with a `&'static str` mapping. The `WebError` variants are a closed set, so a typed
   enum gives compile-time correctness and matches the codebase's typed-error style.

3. **Capture only, no logging** (Q2 → A): the function wraps `sentry::capture_error`
   and sets the tag; it does not own `tracing::error!`. The task is scoped to Sentry
   enrichment, and the two arms already log differently (`?err` for `Database`/`Template`
   vs `%message` for `External`, `src/app/error.rs:62,67,72`) — folding logging in would
   widen the contract for no benefit.

4. **Tag mechanism = `sentry::with_scope`** with `scope.set_tag("source", ...)` around a
   single `sentry::capture_error` call. Scope tags are applied to the event at capture
   time (`sentry-core-0.49.1/src/scope/real.rs:296-299`), and `with_scope` scopes the
   tag to one event without polluting the thread-local hub.

5. **Keep `ExternalError` newtype** (Q4 → A): the centralized function stays generic
   `fn capture_error_with_source<E: std::error::Error>(err: &E, source: ErrorSource)`,
   and `External` continues to pass `&ExternalError(message)`. Switching that arm to
   `capture_message` would split the event model (exceptions vs message events) and
   lose error-chain info in Sentry for no gain.

6. **One end-to-end test** (Q3 → "just one"): enable the `sentry` `test` feature in
   `[dev-dependencies]` (`sentry = { version = "0.49", features = ["test"] }`) and add
   a single `#[test]` in `src/infra/sentry.rs` that runs
   `sentry::test::with_captured_events(...)` and asserts the captured event's `source`
   tag equals the enum's mapping. This proves the enrichment lands on the real event,
   which a pure enum-value test would not.

7. **`app::error` drops the direct `sentry::` import**: the three arms call
   `crate::infra::sentry::capture_error_with_source(...)`. This closes the arkitect
   gap (only `infra` touches `sentry`) without moving the capture decision itself —
   `IntoResponse` remains the chokepoint (`src/app/error.rs:56`).

## What We're NOT Doing

- **No tagging of `NotFound`, `BadRequest`, `TooManyRequests`** — expected/client faults
  stay uncaptured (`src/app/error.rs:76-89`).
- **No request/route metadata enrichment** (method, path, user) — out of scope; the
  `source` tag is the only field this ticket adds.
- **No tracing↔Sentry bridge** (`sentry::integrations::tracing`), no breadcrumbs, no
  contexts/extras beyond the `source` tag.
- **No changes to client startup** (`src/infra/sentry.rs` `init`, `src/main.rs:19-21`):
  release/PII options, panic hook, and `ENABLE_SENTRY` gating stay as-is.
- **No `configure_scope`/ambient-scope usage** — per-capture scoping only.
- **No conversion of `External` to `capture_message`** and no removal of `ExternalError`.
- **No test-harness changes** (`src/test/mod.rs` stays `enable_sentry: false`); the new
  test uses the `sentry::test` transport in isolation.

## Open Risks

- **Feature unification**: enabling `sentry`'s `test` feature in `[dev-dependencies]`
  unifies it into test builds crate-wide. It only activates `sentry::test`'s
  `#[cfg(feature = "test")]` module, so the runtime path is untouched — but confirm
  `scripts/test.sh` still passes cleanly.
- **`with_scope` closure capture**: the tag callback runs on the calling thread's hub.
  The `IntoResponse` arms already call capture on the request thread today, so this
  holds; a future move to a background task would silently drop the tag.
- **`Event` tag field shape**: the exact accessor (`Event::tag_value` vs `Event.tags.get`)
  depends on the pinned `sentry-types` version — verify at implementation time.
- **Arkitect enforcement semantics**: the research flagged ambiguity about whether the
  `rules_for_module` allowlists or the cross-layer `must_not_depend_on` bans are the
  operative gate (`src/test/arkitect.rs:23-49`). Removing `sentry::` from `app::error`
  satisfies both; the suite run in `/6_implement` is the real check.
