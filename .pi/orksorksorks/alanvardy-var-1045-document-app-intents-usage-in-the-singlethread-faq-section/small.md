# Task

Document SingleThread's App Intents (Apple Shortcuts) in the SingleThread page's
**FAQ section** so users know these shortcuts exist and how to invoke them.
They shipped in SingleThread PR #205 (`SingleThreadShortcuts: AppShortcutsProvider`
in `SingleThread/AppShortcuts.swift`).

Add FAQ entries covering the three discoverable intents:

| Intent | What it does | Siri phrases | Short title |
| -- | -- | -- | -- |
| `WhatsNextIntent` | Speaks/returns the current next task's title | "What's next in SingleThread", "What is next in SingleThread" | What's Next |
| `CompleteCurrentTaskIntent` | Completes the first visible task | "Complete the current task in SingleThread", "Mark my current task done in SingleThread" | Complete Current Task |
| `SkipCurrentTaskIntent` | Skips the first visible task (durable, survives relaunch) | "Skip the current task in SingleThread", "Skip my current task in SingleThread" | Skip Current Task |

### Surfaces where they appear (worth mentioning in the FAQ)

* **Siri** — the phrases above (works on a physical device; simulator Siri is unreliable).
* **Shortcuts app** — search "SingleThread"; all three are listed with their short titles.
* **Home Screen** — long-press the app icon; the shortcuts appear in the top section.
* **Spotlight** — search by shortcut name.

### Edge-case messages worth a FAQ line

* No Reminders access: *"Enable access in Settings to see your reminders."*
* Genuinely empty list: *"There's nothing to do right now."*
* Everything skipped/excluded/hidden: *"Everything is skipped for now."*
* Free-tier mutation cap reached: *"You've reached the free limit. Upgrade to keep going."*
* EventKit write failed: *"Couldn't update that task. Please try again."*

### Notes

* Intents never prompt for Reminders access — they only act when access is
  already granted (`.fullAccess`), so the FAQ should tell users to grant
  Reminders access first.
* The widget's own Complete/Skip buttons use non-discoverable variants
  (`CompleteReminderIntent` / `SkipReminderIntent`); not user-facing, no need to document.
* Screenshots of the app-icon long-press menu and the Shortcuts app listing
  would help, but are optional.

## Why SMALL

All of A–F hold: single module — the FAQ is data-driven, so the whole change is
additive content in one handler file following the existing `FaqCategory`/
`FaqItem` pattern; approach and content are fully specified (0–2 trivial
placement unknowns); no schema/migration, no new subsystem, no
shared/convention code, no design decision or sign-off; tests are the existing
cascading FAQ tests plus a few local assertions.

## Key files

- `src/interfaces/handlers/singlethread/web.rs` — the `FAQS` const holds all
  FAQ content as `FaqCategory { title, items: [FaqItem { question, answer }] }`
  (categories: Getting started, Features, Privacy, Other). Add the App Intents
  entries here (a new `FaqCategory` such as "Shortcuts", or items under an
  existing category, at your discretion).
- `templates/singlethread.html` — renders FAQ generically via
  `{% for category in faq_categories %}` + `<details class="faq-item">`; no
  change needed for new items.
- Tests live in `#[cfg(test)] mod tests` at the bottom of `web.rs`: they
  cascade via `all_faq_items()` (`faq_all_questions_appear`,
  `faq_all_answers_appear`, `faq_items_no_duplicate_questions`,
  `faq_summary_has_chevron`), so new items are covered automatically — add
  targeted assertions only if useful (e.g. one new question appears in the
  rendered page).
- Rendered HTML is minijinja-autoescaped: assert `&#x27;` for `'` and `&#x2f;`
  for `/`; the existing `html_escape` test helper reproduces this.
- Run `./scripts/test.sh` (format, type-check, lint, tests) before finishing.
  No schema/route/template/CSS changes are expected.