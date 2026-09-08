# Task

Add a new entry to the SingleThread FAQ (VAR-782). The Q&A text is dictated
verbatim by the ticket — no wording changes:

- **Q:** Why can't I filter reminders by flagged reminders?
- **A:** Apple unfortunately doesn't expose flags to outside apps, if they
  ever do I will support it!

The entry fits the "Other" category of the app's FAQ. The right answer: Apple
doesn't expose the Reminders "flagged" state to third-party apps, so the app
cannot filter on it (and will when Apple permits).

## Why SMALL

Single-module content addition to one static data structure; Q&A supplied by
the ticket so no design decision or research; no schema/API/UI change; all
existing FAQ tests iterate the FAQ array dynamically, so they cover the new
entry without modification. Criteria A–F all hold.

## Key files (if the recon found any)

- `src/interfaces/handlers/singlethread/web.rs` — `const FAQS: &[FaqCategory]`
  (categories: Getting started / Features / Privacy / Other). Add a
  `FaqItem { question, answer }` to the "Other" category.
- Tests: `#[cfg(test)] mod tests` in the same file iterate `FAQS` dynamically
  (`faq_all_questions_appear`, `faq_all_answers_appear`,
  `faq_items_no_duplicate_questions` …) — they cover a new entry with no
  edits, provided the question text doesn't duplicate an existing one.
- `templates/singlethread.html` renders `faq_categories` generically — no
  template change needed.