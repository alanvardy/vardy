# Research Findings

Two codebases: the vardy web app (this repo, Rust/axum; refs like `src/…`, `templates/…` are repo-relative) and the SingleThread Apple suite at `/Users/vardy/dev/SingleThread` (refs like `SingleThreadCore/…`, `SingleThread/…` are relative to that root; HEAD `19105b7`). EventKit SDK headers at `/Applications/Xcode.app/Contents/Developer/Platforms/WatchSimulator.platform/Developer/SDKs/WatchSimulator26.5.sdk/System/Library/Frameworks/EventKit.framework/Headers/`. All findings from agents' persisted outputs, with targeted re-verification of the one contradiction (`EKCalendarItem.attachments`, §Q4).

> **Post-rebase note (large-review)**: this file is the research-phase snapshot,
> written before the branch rebased onto `main` (`a419d32`), which brought in
> the VAR-782 "Why can't I filter reminders by flagged reminders?" FAQ. The
> Q1/Q5 claims "16 items", "no FAQS item about flagged reminders … exists on the
> current branch", and "`a419d32` … is not an ancestor of this branch" are
> pre-rebase facts; the final tree has 17 items (see `plan.md`).

---

## Q1: SingleThread FAQ structure, rendering, docs, tests, prose conventions, history

### Findings
- Types: `struct FaqCategory { title: &'static str, items: &'static [FaqItem] }` and `struct FaqItem { question: &'static str, answer: &'static str }` — `src/interfaces/handlers/singlethread/web.rs:8-16`. Const: `const FAQS: &[FaqCategory]` — web.rs:18.
- 4 categories / 16 items: "Getting started" (web.rs:23-28, 2 items), "Features" (web.rs:36-61, 7 items), "Privacy" (web.rs:69-78, 3 items), "Other" (web.rs:86-99, 4 items). No `serde::Serialize` on the types; FAQS is marshalled to `serde_json::Value` in the handler because the interfaces layer may not depend on serde (comment web.rs:111-114).
- Every item (verbatim, web.rs): Q&A pairs at web.rs:23-24, 27-28, 36-37, 40-41, 44-45, 48-49, 52-53, 56-57, 60-61, 69-70, 73-74, 77-78, 86-87, 90-91, 94-95, 98-99.
- Rendering (templates/singlethread.html:91-100): `<h2 class="heading-section">Frequently Asked Questions</h2>` then `{% for category %}` → `<h3 class="heading-subsection">{{ category.title }}</h3>` (:93), then `{% for item %}` → `<details class="faq-item"><summary><span class="faq-chevron" aria-hidden="true">…</span>{{ question }}</summary><p class="faq-answer text-muted mt-2">{{ answer }}</p></details>` (:96-98). Closing CTA `Your reminders. One at a time.` (:101).
- Docs: `ROUTES.md:22-38` documents `GET /singlethread` (page description with "FAQ section with collapsible Q&A pairs"; Response 200 text/html; Errors 500 via WebError; Rate limit note).
- Tests (inline `web.rs:131-453`): `faq_all_questions_appear` (:253), `faq_all_answers_appear` (:276) — both assert every item's escaped text appears; `faq_items_grouped_under_category_headings` (:366); `faq_summary_has_chevron` (:342); `faq_section_after_quiet_productivity_before_cta` (:299); `faq_no_javascript` (:326); `faq_items_all_non_empty` (:428); `faq_items_no_duplicate_questions` (:443). Minijinja autoescaping handling: tests compare against escaped entities via a local `html_escape` helper (web.rs:415-425) mirroring `AutoEscape::Html` (`'` → `&#x27;`, `/` → `&#x2f;`).
- Prose conventions (all 16 items): first-person founder voice ("I", "me"), conversational/warm (web.rs:37 "Yes!", :61 "It sure does!", :78 "Absolutely not."); answers 11–62 words, 1–3 sentences, **no lists anywhere**; the "Why can't I…" pattern appears in exactly two items (web.rs:47-49 attachments, :87-88 dictation = the closest style templates for an "unavailable feature" answer); category titles short noun phrases ("Getting started", "Features", "Privacy", "Other").
- History: **no FAQS item about flagged reminders, tags, or filtering exists on the current branch** (verified at every revision of web.rs). Prior work exists on `main` but is not an ancestor of this branch: commit `a419d32` "Add FAQ item: why can't I filter reminders by flagged reminders?" adds Q `Why can't I filter reminders by flagged reminders?` / A `Apple unfortunately doesn't expose flags to outside apps, if they ever do I will support it!` — identical in commit `e9e17e5` on branch `alanvardy-var-782-add-faq-item-why-cant-i-filter-by-flagged-reminders`; neither is an ancestor of HEAD here (agents verified with `git merge-base --is-ancestor`). `git log --all -S "tag" -- web.rs` hits are only "tagline"/CSS `tags`, never FAQ items. No historical FAQ item about tags/filtering either.
- The FAQ const currently contains **no** item about flags/tags/filtering.

## Q2: Complete EventKit reminder data surface (headers read in full)

All property names and read/set annotations are as declared in the headers.

### EKReminder own props (EKReminder.h; inherits EKCalendarItem, NOT EKEvent)
- `startDateComponents` (settable, copy, EKReminder.h:36; nil timezone = floating date; setting without hour/minute/second implies allDay)
- `dueDateComponents` (settable, EKReminder.h:44; on iOS a due date requires a start date or save fails `EKErrorNoStartDate`)
- `completed`/`isCompleted` (settable, linked to completionDate: setting completed=YES sets completionDate=now, EKReminder.h:61, 48-54)
- `completionDate` (settable, may be nil while isCompleted if completed elsewhere, EKReminder.h:63)
- `priority` (settable NSUInteger, RFC 5545 0=none,1-4 high,5 medium,6-9 low; `EKReminderPriority…` constants, EKReminder.h:64-76)
- Factory `+ reminderWithEventStore:` (EKReminder.h:13)
- **There is no `dueDate` (NSDate) property and no `isFloating` boolean** — only date components; a grep for `isFloating` returns only doc-comment text (EKReminder.h:33, :42).

### Inherited `EKCalendarItem` (EKCalendarItem.h)
- Writable: `title` (:77), `location` (plain string; structured only on EKEvent, :78), `notes` (:79), `URL` (:80), `timeZone` (:84), `alarms` (array, :101), `recurrenceRules` (:120), `calendar` (:33, null for new items).
- Read-only: `calendarItemIdentifier` (:43), `calendarItemExternalIdentifier` (:70), `lastModifiedDate` (:82), `creationDate` (:83), `hasAlarms` (:91), `hasRecurrenceRules` (:92), `hasAttendees` (:93), `hasNotes` (:94), `attendees` (:97), deprecated `UUID` (:22).
- Mutators (settable via method): `addAlarm:/removeAlarm:` (:110, :116), `addRecurrenceRule:/removeRecurrenceRule:` (:120-125).
- Not present: `structuredLocation`, `organizer`, `allDay`, `startDate`/`endDate` (those are EKEvent-only; reminders have neither an NSDate start/end nor allDay boolean). `isFloating` does not exist.

### EKAlarm (EKAlarm.h) — every property
- `relativeOffset` (settable, :50), `absoluteDate` (settable, :58), `structuredLocation` (:66), `proximity` (:73), `type` (READONLY, :82), `emailAddress` (settable, :90), `soundName` (settable, :99), `url` (settable, deprecated, :110).
- Constructors: `alarmWithAbsoluteDate:` (:31), `alarmWithRelativeOffset:` (:41).
- **`acknowledgedDate` does NOT exist** in any header (grep). `EKAlarmProximity` None/Enter/Leave (EKTypes.h:208-213); `EKAlarmType` Display/Audio/Procedure/Email (EKTypes.h:219-226).
- Availability: `type`/`emailAddress`/`soundName`/`url` are macOS-only (`NS_AVAILABLE(10_8, NA)` — NA in the iOS/watchOS slot), and `url` is deprecated on macOS (EKAlarm.h:108-110). On watchOS an alarm exposes only relativeOffset, absoluteDate, structuredLocation, proximity (plus readonly `type`).

### EKRecurrenceRule (EKRecurrenceRule.h)
- Settable only via the recurrence constructors (rules are immutable after init, EKRecurrenceRule.h:17-19): `initRecurrenceWithFrequency:interval:end:` (:39) and the full designated init with all day/count fields (:60-71). Properties: `recurrenceEnd` (settable, :91), `frequency` (readonly, :97), `interval` (readonly, :108), `firstDayOfTheWeek` (readonly, :119), `calendarIdentifier` (readonly, :84) and the read-only detail arrays `daysOfTheWeek` (:144), `daysOfTheMonth` (:154), `daysOfTheYear` (:163), `weeksOfTheYear` (:172), `monthsOfTheYear` (:187), `setPositions` (:189). `EKRecurrenceFrequency` Daily/Weekly/Monthly/Yearly (EKTypes.h:70-74).
- `EKRecurrenceEnd` (endDate OR occurrenceCount, EKRecurrenceEnd.h:39/:45/:51/:57); `EKRecurrenceDayOfWeek` (dayOfTheWeek + weekNumber, EKRecurrenceDayOfWeek.h:31, :36).

### EKCalendar (EKCalendar.h)
- Writable: `source` (only at creation), `title` (:72), `CGColor` (CoreGraphics CGColorRef; this is the only color surface on watchOS, :107).
- Read-only: `calendarIdentifier` (:66), `type` (:80, EKCalendarType Local/CalDAV/Exchange/Subscription/Birthday), `allowsContentModifications` (:86), `subscribed`/`isSubscribed` (:92), `immutable`/`isImmutable` (:100), `supportedEventAvailabilities` (:123), `allowedEntityTypes` (:130).
- **No `isDefaultReminderList`** — default is store-level `defaultCalendarForNewReminders:` (EKEventStore.h:154). No "favorite"/flagged state anywhere on EKCalendar.

### Negative surface (grep-verified across every header): tags, flags, favorites, attachments
- `favorite|flagged|flag|tag|attachment|acknowledged` — grep of the entire Headers dir returns **only**: `EKErrorNotificationsCollectionFlagNotSet` (EKError.h, a Notification error code), `EK_LOSE_FRACTIONAL_SECONDS` (defines), and a YAML key `Tags:` in `EventKit.apinotes` (file format). **No favorite/flag/tag/attachment/acknowledgedDate property exists** on EKReminder, EKCalendarItem, EKAlarm, or EKCalendar. Verified again this session: zero `attachment` hits and zero `endDateComponents` in EKCalendarItem.h.

## Q3: Which reminder values SingleThread actually accesses, and where

All sites verified at file:line in `/Users/vardy/dev/SingleThread`.

| Field | Read sites | Write sites |
|---|---|---|
| title | ReminderDisplay.swift:12; ReminderSort.swift:102; MenuBarExtraOptions.swift:17; WatchReminderView.swift:384; UITestingSeed.swift:160 | EventKitStoring.swift:57; ReminderStore.swift:333-336 (addReminder); InMemoryEventStore.swift:109; UITestingSeed.swift:148; previews + ui-test seams (ContentView+Previews.swift:14, WatchAppViewModel.swift:111) |
| notes | ReminderDisplay.swift:13 → ReminderNotesFormatter (& ReminderSkip.swift:130-158) | EventKitStoring.swift:58; InMemoryEventStore.swift:110; UITestingSeed.swift:149; previews |
| dueDateComponents (`.date`) | ReminderDisplay.swift:14; ReminderSort.swift:71-72; ReminderStore.swift:459 (date window filter); MenuBarExtraOptions.swift:19; RescheduleSheet.swift:55 | EventKitStoring.swift:59; InMemoryEventStore.swift:111; ReminderStore.swift:367 (reschedule); SkippedReminderSyncService.swift:268-315 (watch→phone wire) |
| startDateComponents | never | never |
| priority | ReminderDisplay.swift:15 (marker); ReminderSort.swift:56-57; ReminderSkip.swift:77-126 (level/rank maps) | UITestingSeed.swift:151; AppViewModel.swift:244 (`--ui-testing`); previews/seams |
| calendar / list | calendar?.title — ReminderStore.swift:150 (excluded-lists), ReminderDisplay.swift:16, ReminderSort.swift:86-87, MenuBar list (EKCalendar.title, ReminderStore.swift:481-483) | EventKitStoring.swift:63 (makeReminder); InMemoryEventStore.swift:115; UITestingSeed.swift:142,153; previews |
| calendarItemIdentifier | ReminderStore.swift (149,191-192,223-224,241,303,317,383,424,565,591,659,676); PendingCompletionLogic.swift:10; ContentView.swift:421,435; ContentViewModel.swift:217 (deep link); ContentView+ActionMenu.swift:185; WatchReminderView.swift:254,302,427; ReminderDeepLink.swift:16 | **never** (EventKit-generated; UITestingSeed.swift:40 documents) |
| isCompleted / completionDate | InMemoryEventStore.swift:60 (filter); PendingCompletionLogic.swift:22 (defensive); fetch predicate | ReminderStore.swift:244 (complete, save), :276 (undo save); `completionDate`/`completed` **never** touched by app |
| alarms | only `hasAlarms` — ReminderDisplay.swift:19; consumed ReminderCardView.swift:126-131, WatchReminderView.swift:295-299, NextThingWidget (ShowsAlarms) | **alarms array never written** (only `ReminderDisplayTests.swift:79` test constructs EKAlarm) |
| recurrence | hasRecurrenceRules ReminderDisplay.swift:17; ReminderRecurrenceFormatter.swift:14-15 reads ONLY the first rule — `rules?.first`, then `first.interval` and `first.frequency` (:18-39); other rule fields (end, byDay, etc.) never read | `addRecurrenceRule` EventKitStoring.swift:60-62; InMemoryStore.swift:112-113; ReminderDictationParser.swift:220,233,242,255 (builds rules); previews |
| url field | **never read** anywhere (deep link built from identifier, ReminderDeepLink.swift) | only preview fixture ContentView+Previews.swift:18 |
| location / structuredLocation | **never** | **never** |

- **No `setValue`/`changedKeys`/`changedProperties` anywhere** (repo-wide grep). Every write is direct property assignment + `eventStore.save(reminder, commit: true)` (ReminderStore.swift:247,279,369; EventKitStoring.swift:29; InMemoryEventStore.swift:87).

### Surface differences
- **iOS** (ContentView.swift, ContentView+iOS.swift, ContentView+ActionMenu.swift, ContentViewModel.swift, ReminderCardView.swift): everything visual via `ReminderDisplay` (CardView :73-138 consumes priorityMarker/titleAttributed/dueDate/listName/recurrenceSummary/hasAlarms/notes); direct EK reads limited to `calendarItemIdentifier` + `dueDateComponents?.date` (RescheduleSheet hasDueTime). Writes through ReminderStore (called from ActionMenu/RescheduleSheet/DictationViewModel.swift:77-80 → addReminder).
- **Watch** (SingleThreadWatch/WatchReminderView.swift, WatchReminderViewModel.swift): also `ReminderDisplay`-driven (:242); reads `calendarItemIdentifier` (nudge) and relays actions. Watch is relay-only: `SkippedReminderSyncService` sends `completeReminderIdentifier`/`delete`/`reschedule` payloads (ReminderStore.swift:218-237; SkippedReminderSyncService.swift:258-287) and carries only identifier + dueDateComponents components (:268-315). EventKit is read-only on watchOS (`__WATCHOS_PROHIBITED` on all mutations).
- **Widget** (SingleThreadWidget/NextThingWidget.swift): zero direct EKReminder access — `store.listContent` wraps `ReminderDisplay` into `NextThingEntry` (:128-154); display consumes the same fields; Complete/Skip via intents (ReminderIntents.swift:9-50), identifiers only.
- **macOS** (MenuBarExtraOptions.swift, ContentView+ActionMenu, SortOption+Presentation): MenuBarExtraOptions.swift:15-19 reads `title` and `dueDateComponents?.date` directly from `store.visibleReminders.first`; SortOption+Presentation is display-only; `ReminderSort` (Core) does sorting on priority/dueDateComponents/calendar?.title/title.
- **Shared Core**: `ReminderDisplay.swift:11-19` is the single mapping point (title, notes, dueDateComponents?.date, priority, calendar?.title, hasRecurrenceRules, recurrenceRules (first rule only), hasAlarms). The widget/macOS/iOS/Watch all read only this struct.

## Q4: Reminder values in the model but with ZERO access sites anywhere

Verified by field-by-field `rg` across all 8 source/test targets + `git log -S`/branch scan; only the sites below exist.

- **ZERO occurrences** (verified): `url` read, `link`, `location`/`.location` on reminder (only unrelated `NSRange(location:)` ReminderDictationParser.swift:277-285), `structuredLocation`, all alarm contents (`relativeOffset`, `absoluteDate` except test-only construction `ReminderDisplayTests.swift:79`), `proximity`, `alarmType/type`, `emailAddress`, `soundName`, `startDateComponents` read, `completionDate` (read/write), `attendees`/organizer, `timeZone` on EK types, `allDay`/`isAllDay`, `creationDate`, `lastModifiedDate`, `displayOrder`, `calendarItemExternalIdentifier`, recurrence `daysOfTheMonth`/`monthsOfTheYear`/`weeksOfTheYear`/`daysOfTheYear`/`setPositions` (only explicit nil args at the constructor ReminderDictationParser.swift:255-262). `daysOfTheWeek` IS used (weeklyRule: ReminderDictationParser.swift:252-263 + parser tests).
- **Tags / flags / favorite / attachments do not exist in the SDK at all** (headers negative, see Q2) — there is no EventKit surface for them, in any SingleThread source or anywhere in either repo. The word "flag" in the codebase refers only to in-app state (ResumptionGate, Entitlement, show-* Watch states, AppViewModel ui-test flags) — a branch scan shows the only flag branch `alanvardy-var-778-add-toggle-for-flagged-reminders` contains a planning stub only (task.md/questions.md, zero code). `favorite(s)` → zero everything. `attachment` → only accessibility-test modifiers (SingleThreadTests.swift:90,96,136; SwipePromptTests.swift:47,53). `EKCalendarItem` appears in **zero** app source files.
- One prior-research note (`var-691` research.md) listed `attachments` on `EKCalendarItem` — **contradicted by the actual headers** (no such property; re-verified this session). The SDK surfaces `EKCalendarItem.attachments` does not exist.

## Q5: The existing "url or file attachments" FAQ item, and matching language in SingleThread

- Verbatim (`src/interfaces/handlers/singlethread/web.rs:48-49`, **entire item**):

```
Q: Why can't I see url or file attachments?
A: I would love to implement this but Apple's internal API for this has been broken for a while, if or when this is fixed I would be delighted to add it.
```

No markdown, no backticks; the only punctuation is the apostrophe in "can't". Evidence cited: **a single claim — "Apple's internal API for this has been broken for a while"** — no links, no "EventKit" mentioned, no mention of "unsupported" or any API reference.

**Matching language in SingleThread:** none. The Swift repo has no FAQ, no README.md; `rg -i 'faq'` on all .md → 0 hits; `"broken for a while"` → 0 hits. The closest notes (all in `SingleThread/.pi/orks…/…` prior research/design dirs):
- `var-691/…/research.md` — surveyed EK surface (lists `url`, `location`; see §4 correction about `attachments`) and "Properties NOT read in production".
- `var-742/…/design.md:8` — priority is RFC 5545 0-9, "iOS 26 Urgent toggle is a separate flag, orthogonal to priority" (:55), "Properties like url, location, start…, completionDate, lastModified…, alarm details, recurrence beyond the first — are never read anywhere in the codebase" (:39).
- `SingleThreadCore/…/ReminderStore.swift:294-296, 322-323, 350-352` — "On watchOS (where EventKit is read-only): removes it locally and relays the deletion to the iPhone via `onDeleteReminder`" / "EventKit is read-only, so this always returns false" / "iOS only — EventKit is read-only on watchOS".
- Historical Swift Display work: commits `1b1bd63` "Phase 1: Display URLs…", `74b089d` "Show a usable url link", `4d38e67` "Display urls and notes" — that URL-in-card code no longer exists in current ContentView (only the deep-link `x-apple-reminderkit://REMCDReminder/<id>` opener seam remains at ReminderDeepLink.swift).
- All "url" hits in app code are the deep-link seam and wallpaper/Unsplash URLs — none touch reminder URL values; the only `reminder.url =` write is the canvas preview fixture (`ContentView+Previews.swift:18`).

## Q6: Conventions that govern adding an FAQ item

(All refs in this repo.)

- **Content source**: only `const FAQS` in `src/interfaces/handlers/singlethread/web.rs:18-103`. Categories (`web.rs:20,33,66,83`) → items. No DB, no Markdown, no template change needed for a new item. The handler just serde-marshals (web.rs:114-123) and renders the same template.
- **Test helpers (web.rs test module)**: `all_faq_items()` flattens every category web.rs:136-141; `faq_all_questions_appear` (web.rs:253-274) / `faq_all_answers_appear` (web.rs:276-297) assert every item's escaped text renders; `faq_items_all_non_empty` (web.rs:428-441) checks no empty titles/items/answers; `faq_items_no_duplicate_questions` (web.rs:443-453) checks unique question strings. All run via `start_app()` (in-memory SQLite, `src/test/mod.rs`) + `test_client()`.
- **HTML escaping assertions**: minijinja applies `AutoEscape::Html` for `.html` templates (`templates.rs:8-9`); tests assert the escaped forms — helper `html_escape` (web.rs:415-425) reproduces escaping of ``< > & " ' /`` (e.g. `'` → `&#x27;`, `/` → `&#x2f;`). Examples in tests: `&#x27;` for apostrophes (web.rs:264) and `&#x2f;` for slashes in the wallpaper URL (web.rs:194).
- **The `./scripts/test.sh` gate** (runs: `cargo fmt --all`, `cargo sqlx prepare -- --tests`, `cargo check --all-targets`, `./scripts/build-css.sh` then `git diff --exit-code -- static/site.css` (CSS-drift check), `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo nextest run`, final TODO grep `rg 'FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO' src` with fail-on-match). CI mirrors this in `.github/workflows/ci.yml` (fmt, clippy, nextest `--profile ci`, css-drift job).
- **Template/route constraints**: adding an FAQ item touches neither `routes.rs` (route stays `GET /singlethread`, routes.rs:49) nor `ROUTES.md` (only route/param changes need ROUTES.md, per AGENTS.md "## Routes"). The `FAQ` block structure (h2 + category h3 + `<details>`/`<summary>` + `<p class="faq-answer…">`) is pinned to the templates (singlethread.html:91-100) and tests (:253, :276, :366).
- **CSS-drift**: if you change a Tailwind class, you must also commit `static/site.css` — `./scripts/build-css.sh` (pinned Tailwind standalone CLI v4.3.3) compiles `css/site.css` into it, and `git diff --exit-code -- static/site.css` in test.sh/CI fails otherwise. FAQ editing normally adds only text; no class changes needed.
- **Prose order/tone**: an "unavailable feature" answer should follow the existing "Why can't I…" items (tone, first person, no lists — see Q1 conventions).

---

## Cross-Cutting Observations

- The FAQ is entirely data-driven: `FAQS` → serde → template loop; adding an item is a one-line-const edit, and the test suite already auto-covers "every question/answer renders escaped" and "no duplicates", so a new item needs no new assertions unless it changes structure.
- The app's entire EK reminder surface is funneled through `ReminderDisplay.swift:11-19` (title, notes, date, priority, list, first‑rule recurrence, hasAlarms) — everything else in the model is untouched, so the set of "values the app can't access/display" is precisely: everything listed in §4 that exists (url, location, alarm details, startDate, completionDate, timeZone, creation/modification dates, attendees/organizer, recurrence beyond first frequency+interval) plus the non-existent-in-SDK values (§ tags/flags/attachments).
- The FAQ's existing claim that "Apple's internal API for this has been broken" is **not corroborated by the SDK headers** (which simply expose a writable `url` property), but the claim is the existing precedent for how such "can't access" items are worded. The could-not-exist set (tags, flags, favorites, file attachments) genuinely does not exist as EK properties.
- `EKCalendarItem` (the class that would surface attachment/flagged state) appears in zero SingleThread source files — the app only touches `EKReminder` plus type-derived values.

## Open Areas

- Whether Apple's "broken internal API" claim from item `file attachments` is factually true is outside the code's evidence; the headers simply have no surface for attachments.
- Exact public docs beyond `ROUTES.md` (e.g. any site copy listing "url/file attachments" outside the `raw` FAQ) were not exhaustively searched (only `templates/singlethread.html` was scanned; layout/banners not checked for the phrase).
- Watch/macOS build target availability nuances (e.g. `EKRecurrenceDayOfWeek` per-platform availability) were only partially cross-checked against the Watch SDK; each surfaced value is cited per surface in §3.