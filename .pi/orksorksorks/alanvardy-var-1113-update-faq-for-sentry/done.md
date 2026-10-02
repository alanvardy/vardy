# Done

- **What was built**: Content-only FAQ copy update on the SingleThread and
  CheckStitch public pages to disclose that the apps now send anonymous
  crash-report data to Sentry (used only to fix bugs). Reworded the Privacy
  "collect data" answers and the "network requests" answers on both pages so
  they stay honest while naming Sentry; updated the checkstitch
  `faq_privacy_disclosures_documented` test assertion to the new wording.
- **Commit SHA(s)**:
  - `fd4108f` — docs: mention Sentry crash reporting in FAQ privacy answers
  - `50fb780` — chore: remove scaffold placeholder (pre-merge cleanup)
- **Verification**: `cargo nextest run -E 'test(faq_privacy_disclosures_documented)'`
  → 1 passed, 126 skipped; `cargo fmt --all` clean. Full `./scripts/test.sh`
  gate not run (per SMALL workflow, targeted verify only).
- **Reviewer findings**: No blockers, no nits. Confirmed Sentry disclosure is
  honest across all four edited answers, no missed FAQ item, no self
  contradiction, updated test assertion will pass, and no lingering assertions
  on removed copy.
- **Remaining manual items**:
  - Full `./scripts/test.sh` CI gate should be run on the PR before merge.
  - `DELETEME` placeholder removal landed as a separate cleanup commit
    (`50fb780`) — verify it's wanted before merge. No route change, so
    `ROUTES.md` needs no update.