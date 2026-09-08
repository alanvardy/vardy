# Conventions (shared appendix)

Factual, reference-only. Two repos: the vardy web app (this repo, Rust) and
`/Users/vardy/dev/SingleThread` (Swift). All `web.rs`/`templates/`/`scripts/` refs
are repo-relative; `SingleThread*` refs are relative to `/Users/vardy/dev/SingleThread`.

## Build / test / lint / verify commands

### vardy web app
- Gate: **`./scripts/test.sh`** (sources `.env` for `DATABASE_URL`; `.env` must exist). Steps, verbatim order (scripts/test.sh:1-22):
  1. `cargo fmt --all` (test.sh:6)
  2. `cargo sqlx prepare -- --tests` (test.sh:8) — regenerates committed `.sqlx/` offline metadata
  3. `cargo check --all-targets` (test.sh:10)
  4. `./scripts/build-css.sh` (test.sh:11) then **`git diff --exit-code -- static/site.css`** (test.sh:12) — CSS-drift gate
  5. `cargo clippy --all-targets --all-features --locked -- -D warnings` (test.sh:15)
  6. `cargo nextest run` (test.sh:17)
  7. TODO grep: `! rg -i -s -g '*.rs' 'FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO\s|todo\s' src` (test.sh:19-20) — fail-on-match
- CSS build alone: `./scripts/build-css.sh` (pinned Tailwind standalone CLI **v4.3.3**; downloads to `target/tailwindcss-cli/tailwindcss`; compile `css/site.css` → `static/site.css --minify`) — build-css.sh:6-8, :37-39.
- `cargo sqlx prepare` needs `DATABASE_URL` — from `.env` (`sqlite:test.db`); `.env_template` documents it. Compile-time query macros need `SQLX_OFFLINE=true` when no live DB.
- Reset DB: `./scripts/reset_db.sh`. No Makefile (glob `**/Makefile*` = nothing).
- CI (`.github/workflows/ci.yml`): `test` job — PRs `cargo nextest run --profile ci`, main `cargo llvm-cov nextest --profile ci --all-features --lcov`; `todos` — `./scripts/lint_string.sh`; `fmt` — `cargo fmt --all -- --check`; `clippy` — same flags as gate; `css-drift` job repeats build-css + `git diff --exit-code -- static/site.css`.
- `scripts/lint_string.sh`: `find . -name '*.rs' -print0 | xargs -0 grep -E "$1" | wc -l`, nonzero → fail.
- Other workflows: `fly-deploy.yml`, `ci-secure.yml` (CodeQL), `dependabot_auto_merge.yml`, `rust-version-bump.yml`.

### SingleThread (Swift) — from `/Users/vardy/dev/SingleThread`
- `Makefile` exists at repo root (build/test entry points; not read in detail — Structure/Plan should open it before running Swift gates).
- Test targets: `SingleThreadTests`, `SingleThreadWatchTests`, `SingleThreadUITests`, `SingleThreadWatchUITests`. SingleThreadCore is a SwiftPM package (`SingleThreadCore/Package.swift`).
- EventKit is **read-only on watchOS** — mutations are `__WATCHOS_PROHIBITED` in EKEventStore.h (saveReminder/removeReminder/commit etc.).

## Test-suite inventory — vardy web app (this repo)

| Path | Covers | Platform gating |
|---|---|---|
| `src/interfaces/handlers/singlethread/web.rs` `#[cfg(test)] mod tests` (web.rs:133-453) | FAQ/FAQ structure: `faq_all_questions_appear` (:253), `faq_all_answers_appear` (:276), `faq_items_grouped_under_category_headings` (:366), `faq_summary_has_chevron` (:342), `faq_section_after_quiet_productivity_before_cta` (:299), `faq_no_javascript` (:326), `faq_items_all_non_empty` (:428), `faq_items_no_duplicate_questions` (:443); page: `index_serves_ok_html` (:148), wallpaper fallback (:212), credit-mode (:232) | none (inline `#[test]`/`#[tokio::test]`) |
| `src/app/templates.rs` `#[cfg(test)]` (:27-57) | minijinja autoescape pinning (`html_names_are_escaped_and_others_are_not`), `asset_url` function | none |
| `src/test/mod.rs` | `start_app`, `start_app_with`, `test_client`, `seed_wallpaper_no_url` helpers — shared harness | none (all rust tests) |

Helpers used by FAQ tests: `all_faq_items()` (web.rs:136-141) flattens `FAQS`; `html_escape()` (web.rs:415-425) reproduces minijinja `AutoEscape::Html` (``< > & " ' /`` → `&lt; &gt; &amp; &quot; &#x27; &#x2f;`). HTML assertions must use escaped forms (`&#x27;` / `&#x2f;`), e.g. web.rs:194, :264. `start_app()` boots real router (in-memory SQLite, random port); `#[sqlx::test]` used in db-dependent tests.

Other handler test suites in-repo (pattern): home (`src/interfaces/handlers/home/web.rs`), contact, dump — same `start_app` + rendered-HTML substring assertion style.

## Test-suite inventory — SingleThread (Swift, `/Users/vardy/dev/SingleThread`)

| Path | Covers | Platform gating |
|---|---|---|
| `SingleThreadTests/ReminderStoreTests.swift` | store fetch/complete/undo/reschedule/recurrence around `calendarItemIdentifier` | none |
| `SingleThreadTests/ReminderDisplayTests.swift` | display mapping (incl. `addAlarm(EKAlarm(absoluteDate:))` at :79) | none |
| `SingleThreadTests/ReminderRecurrenceFormatterTests.swift` | first-rule frequency+interval formatting | none |
| `SingleThreadTests/ReminderDictationParserTests.swift` | NL→EKRecurrenceRule incl. `daysOfTheWeek` (weeklyRule) | none |
| `SingleThreadTests/SingleThreadTests.swift`, `SwipePromptTests.swift` | accessibility modifiers (`AccessibilityAttachmentModifier`) | none |
| `SingleThreadWatchTests/WatchReminderViewRegressionTests.swift`, `WatchSyncPipelineTests.swift`, `ReminderStoreWatchTests.swift` | watch relay pipeline (identifiers + dueDateComponents) | watch |
| `SingleThreadTests/MenuBarExtraOptionsTests.swift`, `MacOSActionButtonChromeTests.swift` | macOS menu-bar extras / action-button chrome | macOS (per-target) |
| `SingleThreadUITests/…`, `SingleThreadWatchUITests/…` | UI tests (not read in detail this pass) | simulator |

No `#if os(…)` gating was observed in the read files; per-target test dirs are the gating mechanism. UI-test seeds come from `--ui-testing` seams (`UITestingSeed.swift`, `WatchAppViewModel.swift:111-151`, `AppViewModel.swift:244`).

## Gotchas (build/verify)

- **CSS drift**: any Tailwind class edit requires regenerating **and committing** `static/site.css` in the same change, or gate/CI fails (test.sh:11-12; AGENTS.md "Tests" section).
- **Escaped-HTML assertions**: rendered page shows `&#x27;` for `'` and `&#x2f;` for `/`; never assert raw apostrophes/slashes in HTML body strings (web.rs:264-265; templates.rs:8-9).
- **No fixed FAQ item-count test** — `faq_items_count` was deleted (`ca47860`); count asserts now come only from all-questions/all-answers loops. Adding an item is safe; **don't introduce a hard-coded count**.
- **sqlx offline metadata** must be refreshed with `cargo sqlx prepare -- --tests` after schema changes; compile-time query macros need `DATABASE_URL` or `SQLX_OFFLINE=true`.
- **One test process at a time** on the random-port in-memory harness is fine; if a port/DB lock appears, `./scripts/reset_db.sh`.
- **ROUTES.md blocks** are `###` → closing `---`; batch edits should use `---` as cut point. FAQ-content changes do NOT touch ROUTES.md or routes.rs (only route parameter changes do — AGENTS.md "## Routes").
- **Money/typography conventions in FAQ prose**: `$USD 2.99` with "$USD" (web.rs:28); FAQ prose uses no lists, contractions, first person (conventions in research.md Q1).