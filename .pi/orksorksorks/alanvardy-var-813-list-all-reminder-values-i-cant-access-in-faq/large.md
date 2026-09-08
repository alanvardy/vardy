# Task

Add a SingleThread FAQ entry (in `FAQS` in `src/interfaces/handlers/singlethread/web.rs`) that lists every Apple Reminder value the app cannot access — URLs and file attachments are named in the ticket, tags/flags are already covered by VAR-782's FAQ item, and "what else?" is open. The full set must be enumerated from Apple's EventKit/`EKReminder` surface and the `../SingleThread` codebase (e.g. `ReminderStore.swift`, `ReminderDisplay.swift`, `ReminderDeepLink.swift`) before any copy is written, and the wording must stay consistent with the existing "Why can't I see url or file attachments?" FAQ item.

## Why LARGE

UNKNOWNS — "urls, files, tags, what else?" is an open research question: enumerating the complete set of inaccessible reminder values requires surveying Apple's EventKit reminder model against what SingleThread actually reads, before an accurate (and exhaustive) FAQ answer can be written.