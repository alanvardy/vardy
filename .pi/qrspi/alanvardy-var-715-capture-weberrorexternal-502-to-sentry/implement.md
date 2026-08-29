# Implementation Summary

## Commits
| Phase | Commit | Description |
|-------|--------|-------------|
| 1     | 5e9d865 | ExternalError newtype + Error impl |
| 2     | 7cd3622 | Wire ExternalError into the External match arm |

## Automated Checks
- [x] `cargo nextest run` — `external_error_implements_error` and `external_error_is_send_sync` pass; no other tests affected (Phase 1)
- [x] `./scripts/test.sh` — full gate passes: format, sqlx prepare, check, CSS build + drift check, clippy, nextest (all targets), and forgotten-TODOs grep (Phase 2)
- [x] `cargo nextest run` — pre-existing 502 tests still pass (no status-code regressions): `external_error_is_502`, `resend_error_is_502`, `post_resend_failure_returns_502`, `upstream_failure_is_502`, `random_upstream_failure_502`, `malformed_upstream_json_missing_user_links_is_502` (Phase 2)
- [x] `cargo test test_architectural_rules` — arkitect rules still pass; `sentry` call in `vardy::app` remains permitted (Phase 2)

## Manual Verification Items (from the plan)
- [ ] Confirm the new type compiles and is not referenced by any other code (grep `ExternalError` in `src/` returns only the new definition + tests)
- [ ] Visual inspection: the `External` arm now has `tracing::error!` followed by `sentry::capture_error` on adjacent lines, matching the `Database` and `Template` arm patterns exactly

## Notes
- No deviations from the plan; codebase matched the plan's expectations exactly.
- The `External` arm now captures to Sentry via the `ExternalError` newtype, giving it parity with the `Database` and `Template` arms. `TooManyRequests` remains the only arm that logs nothing to Sentry (now with the stale comment removed).
- The QRSPI scratch docs (`design.md`, `questions.md`, `research.md`, `structure.md`, `task.md`) under `.pi/qrspi/alanvardy-var-715-capture-weberrorexternal-502-to-sentry/` remain untracked — decide later whether to commit them.