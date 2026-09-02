# Implementation Summary

## Commits

| Phase | Commit | Description |
|-------|--------|-------------|
| 1     | `142cbbd` | ErrorSource enum + sentry test feature |
| 2     | `b7884e3` | capture_error_with_source + e2e capture test |
| 3     | `14eb3f5` | migrate app::error to capture_error_with_source |

All three commits are pushed to `origin/alanvardy-var-741-add-source-enrichment-to-all-sentry-error-captures`.

## Automated Checks

- [x] `cargo check --all-targets` passes (Stage 1)
- [x] `error_source_variants_map_correctly` passes — enum maps to `database`/`template`/`external` (Stage 1)
- [x] `capture_includes_source_tag` passes — exactly one event captured, `source == "database"` (Stage 2)
- [x] Full `./scripts/test.sh` gate green end-to-end — fmt → sqlx prepare → check → CSS drift → clippy `-D warnings` → **112/112 tests** → TODO grep (verified by parent after all phases)
- [x] `test_architectural_rules` (arkitect) passes — `vardy::app` no longer references `sentry::` outside `#[cfg(test)]`
- [x] Existing `src/app/error.rs` tests pass unmodified (StatusCode/body assertions unaffected)
- [x] `Cargo.lock` unchanged in its `sentry` entry — single `sentry` 0.49.1, `test` feature resolves without a second version

## Deviations / Notes

1. **Clippy `io_other_error` lint fix (approved via supervisor)**: Stage 2's committed test used
   `std::io::Error::new(ErrorKind::Other, "boom")`, which clippy (`-D warnings`) rejects. The line in
   `src/infra/sentry.rs` was amended to `std::io::Error::other("boom")` (semantics identical) and
   committed inside Phase 3. Stage 2's test is therefore not byte-identical to what was initially
   committed in `b7884e3`.
2. **Plan's `cargo nextest run --lib` adapted**: `vardy` is a binary-only crate (no lib target), so
   `--lib` is not a valid target. Workers used equivalent `cargo nextest run` / name-filter runs; the
   full gate runs plain `cargo nextest run`.
3. **Per-stage gate note**: `./scripts/test.sh` cannot be green after Stage 1 alone because clippy
   flags `dead_code` on the as-yet-unused `ErrorSource`/`as_tag`/`capture_error_with_source` (not
   consumed until Stage 3). This is inherent to the plan's bottom-up batching; the gate is green at
   the end state. No `#[allow(dead_code)]` was invented (not in the plan).
4. **Push required `--force-with-lease` for Phase 1**: the parent's pre-run `git rebase origin/main`
   rewrote the branch; the rewritten commits differ in SHA from what the worker fetched. Verified
   diffs show no content loss. Remote is now at `14eb3f5`.

## Manual Verification Items (from the plan)

- [ ] Stage 1: `cargo tree -e features -i sentry` (or a plain `cargo check --tests`) shows the `test`
      feature resolving without pulling in a second `sentry` version — confirm `Cargo.lock` is
      unchanged in its `sentry` entry. (Evidence gathered: `Cargo.lock` has a single `sentry` 0.49.1.)
- [ ] Stage 2: temporarily change the assertion to `Some("wrong")` and confirm the test fails, then
      revert. (Proves the tag assertion reads the captured event, not vacuously passing.)
- [ ] Stage 3: `rg "sentry::" src/app` returns no matches outside test code. (Evidence gathered: the
      only match is the `use crate::infra::sentry::{ErrorSource, capture_error_with_source}` import.)
- [ ] Stage 3: `rg "capture_error" src` shows exactly four sites — the single wrapper
      (`sentry::capture_error` inside `capture_error_with_source` at `src/infra/sentry.rs:77`) plus
      the three migrated call sites in `app::error` (lines 66, 71, 76) — none invoking
      `sentry::capture_error` directly.