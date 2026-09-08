# Research Questions

## Context

Two codebases are in scope: the vardy web app's SingleThread FAQ
(`src/interfaces/handlers/singlethread/web.rs` and `templates/singlethread.html`)
and the SingleThread Apple app suite under `../SingleThread`
(SingleThreadCore, iOS app, Watch app, Widget, macOS extras, and tests), which
reads Apple Reminders through the EventKit framework. The EventKit SDK headers
that SingleThread's build targets define the full reminder data surface.
Survey existing structure and conventions, the EventKit data model, and which
reminder fields are actually consumed by each surface.

## Questions

1. How is the SingleThread FAQ structured in the vardy app — the `FaqItem`/`FaqCategory`
   data types, categories, and every item's question+answer in
   `src/interfaces/handlers/singlethread/web.rs` — and how is it rendered
   (`templates/singlethread.html`), documented (`ROUTES.md`), and tested (inline
   `#[cfg(test)]` tests, minijinja autoescaping)? What are the prose conventions
   across all existing items (tone, first person, length, no lists, category
   titles)? Does the FAQ — the current `FAQS` const or any prior version in git
   history — contain any item about flagged reminders, tags, or filtering?

2. What is the complete EventKit reminder data surface that SingleThread's build
   targets? From the EventKit SDK headers (e.g.
   `/Applications/Xcode.app/Contents/Developer/Platforms/WatchSimulator.platform/Developer/SDKs/WatchSimulator26.5.sdk/System/Library/Frameworks/EventKit.framework/Headers/`),
   enumerate every property a reminder can carry — `EKReminder`, inherited
   `EKCalendarItem` fields, `EKAlarm` fields, `EKRecurrenceRule` details, and
   `EKCalendar` state — noting which are read-only, write-only, deprecated, or settable
   (e.g. `addAlarm`/`addRecurrenceRule`). Cite header files and property names.

3. Which reminder values does SingleThread actually access, and where? Build an
   accessed-field table with `file:line` references across SingleThreadCore and
   all surfaces (`ReminderDisplay.swift`, `ReminderStore.swift`,
   `ReminderSort.swift`, `ReminderDeepLink.swift`, `ReminderRecurrenceFormatter.swift`,
   `RescheduleSheet.swift`, `ContentView*`, `WatchReminderView.swift`,
   `NextThingWidget.swift`, `MenuBarExtraOptions.swift`, test stores), covering
   every EKReminder read and write (title, notes, due date, priority, calendar/list,
   identifier, completion, alarms, recurrence). Which fields does each surface
   (iOS vs Watch vs Widget vs macOS) consume differently?

4. Which reminder values are part of the Apple Reminders model but have no access
   site anywhere in SingleThread? Survey fields with zero read/write occurrences
   (url, location, alarm contents — relativeOffset/absoluteDate/structuredLocation/
   proximity/type/emailAddress/soundName, startDateComponents, completionDate,
   attendees, timeZone, creation/lastModified dates, recurrence details beyond the
   first rule's frequency+interval) and confirm each with greps. Separately: does
   Apple's Reminders UI expose values that do not exist on the EventKit surface at
   all — tags, flagging/favorites (e.g. `EKCalendar` favorite/flagged state), file
   attachments — and where would that state live?

5. What does the existing "Why can't I see url or file attachments?" FAQ item say,
   exactly, and what evidence or reasoning about Apple's API does it cite? Is there
   matching language anywhere in the SingleThread codebase (comments, README, docs)
   about unsupported, broken, or inaccessible Apple APIs for urls, files, flags,
   tags, or other reminder values?

6. What conventions govern a change that adds an FAQ item? Detail the test helpers
   and assertions in `web.rs` (`all_faq_items`, `faq_all_questions_appear`,
   `faq_all_answers_appear`, duplicate checks), the HTML-escaping assertions
   (escaped `&#x27;`, `&#x2f;`), the `./scripts/test.sh` gate including the CSS-drift
   check on `static/site.css`, and any routes/template constraints.