# Design Discussion

Branch: `alanvardy-var-813-list-all-reminder-values-i-cant-access-in-faq`
Phase: large-design (3 of QRSPI)

> **Post-rebase note (large-review)**: written before the branch rebased onto
> `main` (`a419d32`), which added the VAR-782 flagged-reminder FAQ. "Current
> State" says 16 items and declares VAR-782 "not an ancestor of HEAD" / "Keep
> `FAQS` item-count 16" — those are pre-rebase; the final FAQ has 17 items
> (see `plan.md`).

## Current State

The SingleThread marketing FAQ is a data-driven const in the vardy web app
(this repo, Rust/axum). Refs are repo-relative unless `SingleThread*`.

- **Data types**: `struct FaqItem { question: &'static str, answer: &'static str }`
  and `struct FaqCategory { title: &'static str, items: &'static [FaqItem] }`
  (`src/interfaces/handlers/singlethread/web.rs:8-16`).
- **Content**: `const FAQS: &[FaqCategory]` (`web.rs:18`) holds 4 categories /
  16 items: "Getting started" (2), "Features" (7, `web.rs:36-61`), "Privacy" (3),
  "Other" (4, `web.rs:86-99`). No serde on the types; the handler marshals `FAQS`
  to `serde_json::Value` (`web.rs:114-123`).
- **The item we will replace** — the existing "why can't I see" answer
  (`web.rs:48-49`, entire):
  > Q: Why can't I see url or file attachments?
  > A: I would love to implement this but Apple's internal API for this has
  > been broken for a while, if or when this is fixed I would be delighted to
  > add it.
- **Rendering**: template loops categories → `<details>`/`<summary>` items
  (`templates/singlethread.html:91-100`). No template change is needed to add or
  swap an item.
- **Tests**: inline `#[cfg(test)]` module (`web.rs:131-453`) auto-covers every
  item via `all_faq_items()` (`web.rs:136-141`),
  `faq_all_questions_appear` (`:253`), `faq_all_answers_appear` (`:276`),
  `faq_items_all_non_empty` (`:428`), `faq_items_no_duplicate_questions`
  (`:443`). Minijinja HTML autoescape is asserted via a local `html_escape`
  helper (`web.rs:415-425`; `'` → `&#x27;`, `/` → `&#x2f;`).
- **Prose conventions** (research Q1): first-person founder voice ("I", "me"),
  warm/conversational, answers 11–62 words / 1–3 sentences, **no lists
  anywhere**, category titles short noun phrases. The "Why can't I…" pattern
  appears in exactly two items today (`web.rs:47-49` attachments, `:87-88`
  dictation) — the closest style templates for an unavailable-feature answer.
- **No flags/tags/filtering FAQ item exists on this branch.** THE VAR-782 item
  ("Why can't I filter reminders by flagged reminders?") lives in commits
  `a419d32`/`e9e17e5` on `main`/`var-782` but is **not an ancestor of HEAD**
  (research Q1).

## Desired End State

A single, authoritative "what can't I see" FAQ answer that:

1. **Replaces** the existing "url or file attachments" item at `web.rs:48-49`
   in the **Features** category (Q2-A, Q4-A).
2. Lists the **user-visible** reminder values the app cannot access or display
   (Q1-A), enumerated from the EventKit headers (research Q2) cross-checked
   against SingleThread's actual read sites (research Q3/Q4):
   - **links/URLs** — `url` property exists (`EKCalendarItem.h:80`), never read
     (research Q4)
   - **file attachments** — no EventKit surface at all (Q4)
   - **tags** — no EventKit surface (Q4)
   - **flags / favorites** — no EventKit surface (Q4)
   - **location** — `location` exists (`EKCalendarItem.h:78`), never read (Q4)
   - **start dates** — `startDateComponents` (`EKReminder.h:36`), never read (Q3)
   - **alarm time & sound** — app reads only `hasAlarms`
     (`SingleThread…/ReminderDisplay.swift:19`); `relativeOffset`,
     `absoluteDate`, `soundName`, etc. never shown (Q3/Q4)
   - **completion timestamp** — only the `isCompleted` boolean is used;
     `completionDate` never read (Q3)
   - **time zone** — `timeZone` (`EKCalendarItem.h:84`), never read (Q4)
   - **custom repeat schedules** — display reads only the first rule's
     frequency + interval (`SingleThread…/ReminderRecurrenceFormatter.swift:14-15`,
     `:18-39`); `recurrenceEnd`, `daysOfTheWeek` display, etc. never shown (Q4)
3. Uses the honest framing **"Apple doesn't give third-party apps access"**
   (Q3-A), dropping the unverified "broken internal API" claim.
4. Includes **tags/flags** inline so the item is self-contained (Q5-A).

**Verification**: after the swap, `./scripts/test.sh` passes with no new
assertions required — the existing loops already assert every item's escaped
text renders and no question duplicates. Autoescape note: if the final copy
contains apostrophes or slashes, the natural `'`/`/` are fine in the source
string; the tests compare against `html_escape`-ed forms, so the const itself
stays raw.

## Patterns to Follow

- **Data-driven FAQ**: edit only `const FAQS` (`web.rs:18-103`); no template,
  route, or DB change. Handler serde-marshals (`web.rs:114-123`) → template loop
  (`templates/singlethread.html:91-100`).
- **Item shape**: one `FaqItem` with `question` + `answer` (`web.rs:8-16`).
- **Question voice**: keep the "Why can't I…" pattern (`web.rs:47-49`, `:87-88`).
- **Answer voice**: first person, warm, 1–3 sentences, **no lists** — enumerate
  the values as a comma-separated clause inside prose, not a bulleted/ordered
  list (research Q1). Match the broken-item's closing tone ("…if Apple ever
  opens it up I would love to add it").
- **Tests**: do not add a new hardcoded count assertion (a count test was
  deliberately deleted, commit `ca47860`); rely on the all-questions/all-answers
  loops. Keep `FAQS` item-count 16 (a 1-for-1 replacement).
- **Gate**: `./scripts/test.sh` (fmt → `sqlx prepare -- --tests` → check →
  CSS build + `git diff --exit-code -- static/site.css` → clippy `-D warnings` →
  nextest → TODO grep). A text-only FAQ edit introduces **no Tailwind change**, so
  no `static/site.css` regeneration is expected — but if the gate's CSS-drift
  check fires, run `./scripts/build-css.sh` and commit the artifact.
- **No routes/ROUTES.md change**: route stays `GET /singlethread`
  (`src/interfaces/routes.rs:49`); FAQ copy is not a route/param change
  (AGENTS.md "## Routes").

### Patterns NOT to follow

- **The "broken internal API" claim** (`web.rs:48-49`) — not corroborated by the
  SDK headers (`url` is a plain writable property; attachments/tags/flags simply
  have no surface). Do not replicate an unverifiable "Apple's API is broken"
  assertion (research Q5).
- **Bulleted/ordered lists in FAQ answers** — breaks the uniform prose style
  across all 16 items.
- **Escaped entities in the source const** — write raw apostrophes/slashes; the
  autoescape is applied at render and asserted in tests via `html_escape`
  (`web.rs:415-425`).

## Design Decisions

1. **Consolidate (replace) the url/attachments item** — chosen because the new
   item supersedes it entirely (both name URLs and file attachments); keeping
   both would answer the same thing twice with two different reasonings (Q2-A).
2. **Enumerate user-visible values only** — the 10 values listed in "Desired
   End State". Internal EventKit metadata (`creationDate`, `lastModifiedDate`,
   `attendees`/organizer, `calendarItemExternalIdentifier`, recurrence sub-fields
   like `daysOfTheMonth`/`setPositions`) is excluded: a reader would never miss
   it, and including it turns a FAQ answer into a spec dump (Q1-A).
3. **"Apple doesn't give third-party apps access" framing** — accurate for
   tags/flags/files (no SDK surface) and consistent-tone for the rest; drops the
   shaky "broken for a while" claim (Q3-A).
4. **Place in "Features"** — capability caveat belongs with the feature
   descriptions, adjacent to where the old item lived (`web.rs:36-61`) (Q4-A).
5. **Include tags/flags inline** — makes the answer exhaustive and independent
   of VAR-782, which is not an ancestor of this branch (Q5-A).
6. **Question phrasing** — keep the "Why can't I…" voice; final wording proposed
   in the plan, but direction: *"Why can't I see some of my reminder details?"*
   rather than re-using the narrow "url or file attachments" title.

Draft copy direction (final wording in `/5_plan`):
> Q: Why can't I see some of my reminder details?
> A: Apple doesn't give apps like SingleThread access to everything in
> Reminders. You can see the title, notes, due date, list, priority and whether
> it repeats or has an alarm, but links, file attachments, tags, flags, location,
> start dates, the alarm's time and sound, and when you completed a reminder
> stay hidden behind Apple's doors — if Apple ever opens them up, I would love
> to show them all.

## What We're NOT Doing

- **Not** listing internal EventKit metadata (`creationDate`, `lastModifiedDate`,
  `attendees`, `calendarItemExternalIdentifier`, recurrence internals) in the
  answer.
- **Not** editing the SingleThread Swift codebase (`/Users/vardy/dev/SingleThread`)
  — its read sites were only surveyed to enumerate the inaccessible set.
- **Not** changing `templates/singlethread.html`, `routes.rs`, or `ROUTES.md`.
- **Not** touching Tailwind/CSS (`static/site.css` should not drift).
- **Not** adding a hardcoded FAQ-count test or new test assertions.
- **Not** creating child tickets — all work stays on this ticket.
- **Not** modifying VAR-782's flagged-filter item even though it overlaps in
  subject; it is out of scope and not on this branch.
- **Not** asserting "Apple's internal API is broken" — the research couldn't
  substantiate it.

## Open Risks

- **VAR-782 overlap**: if `main`'s "why can't I *filter* by flagged reminders?"
  item merges later, "flags" will appear in two FAQ answers (filter vs. see).
  Acceptable — distinct concerns — but flag in any review of the eventual
  merged FAQ.
- **Over-claiming on `url`**: `url` is a writable EventKit property (not an
  absent surface) — the app merely doesn't read it. The "Apple doesn't give
  access" framing must stay soft ("kept out of reach") rather than asserting the
  property doesn't exist.
- **Length vs. convention tension**: enumerating 10 values pushes toward 3
  sentences; the "1–3 sentences, ~62-word max" prose convention is a soft
  ceiling. Final copy should be tuned in `/5_plan`; if it can't fit comfortably,
  raise it before implementing.
- **Autoescape in final copy**: if the answer gains a contraction or slash,
  tests assert the escaped form (`&#x27;`, `&#x2f;`) — the const stays raw but
  the plan must note which assertions (none new, but the answer-appears loops)
  will exercise it.