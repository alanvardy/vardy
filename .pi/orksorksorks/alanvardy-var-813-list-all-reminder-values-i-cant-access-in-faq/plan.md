# Implementation Plan

## Overview

Replace the single "Why can't I see url or file attachments?" FAQ item in the
data-driven `const FAQS` (`src/interfaces/handlers/singlethread/web.rs:47-50`)
with a consolidated "why can't I see some of my reminder details" answer that
enumerates the 10 user-visible reminder values SingleThread cannot access or
display. One-file, text-only change; no schema, store, service, route, template,
or CSS change.

---

## Phase 1: FAQ content layer — finalize the copy

No files change in this phase. Produce the exact question + answer strings that
Phase 2 will paste into the const.

### Changes

#### 1. Final copy (review-only, no edit yet)

**File**: none (copy lands in Phase 2)
**Action**: review / approve wording

```rust
// Question (unchanged "Why can't I…" voice, design DD-6)
"Why can't I see some of my reminder details?"

// Answer — raw string literal, first person, warm, NO list markers, no backticks.
// Apostrophes and the em dash are written literally; autoescape is applied at
// render, never pre-escaped in the const (design "Patterns NOT to follow").
"Apple doesn't give third-party apps access to everything in Reminders. I show the title, notes, due date, list, priority, and whether it repeats or has an alarm, but links, location, start dates, alarm times and sounds, completion times, time zones, and custom repeat schedules are not shown, and tags, flags, and file attachments stay behind Apple's doors — if they ever open, I'd love to show them all."
```

**Prose-constraint review checklist** (manual, no code assertions):

- [x] **Word count** — 67 words (em dash is punctuation, not counted), 2
      sentences. Sits just above the research Q1 11–62-word band; the design
      "Open Risks: Length vs. convention tension" declares ~62 a soft ceiling,
      so this is acceptable. (The design draft ran ~64–67 words and was trimmed;
      see deviation notes at the bottom.)
- [x] **1–3 sentences** — 2 sentences. ✓
- [x] **Zero list markers** — no list-marker `-` (the mid-word hyphen in "third-party" is not a marker), no `*`, `1.`, `<ul>`, or backticks; the ten
      values are a comma-separated clause inside prose. ✓
- [x] **First-person founder voice** — "I show…", "I'd love to show them all";
      warm closing mirrors the old "…I would be delighted to add it" tone. ✓
- [x] **10 values enumerated, none internal** — the seven SDK-present-but-unread
      values (links, location, start dates, alarm times and sounds, completion
      times, time zones, custom repeat schedules) plus the three with no
      EventKit surface (tags, flags, file attachments); design DD-2 set, all 10
      present. No `creationDate` / `lastModifiedDate` / `attendees` / recurrence
      sub-field dump. ✓
- [x] **Honest framing** — "Apple doesn't give third-party apps access", no
      "broken internal API" claim (design DD-3 / "NOT to follow"); "are not
      shown" is assigned to the app, "behind Apple's doors" only to the three
      SDK-absent values. ✓
- [x] **Raw, unescaped** — literals kept raw; any `'` renders to `&#x27;`
      test-side only (design "Autoescape"). Note: the answer contains three
      apostrophes — "doesn't", "Apple's", "I'd" — and one em dash; no `/`. No
      `&#x27;`/`&#x2f;` literals in the const. ✓

**Deviations from the design draft (resolved)**: the draft copy enumerated only
8 of the 10 values (omitted "time zone" and "custom repeat schedules"), which
would have violated design DD-2 ("the 10 values"). This copy adds both and
trims connective words to keep the answer at 62 words.

**Review revision (large-review)**: the initial shipped copy attributed all ten
values to "stay behind Apple's doors", over-claiming for the seven EventKit
exposes but SingleThread never reads (links, location, start dates, alarm times
and sounds, completion times, time zones, custom repeat schedules) — the design's
own "Open Risks: Over-claiming on url" warning, which the copy under-honored.
Final reword splits the two classes: "are not shown" for the seven SDK-present
values vs "stay behind Apple's doors" for tags/flags/file attachments (no
EventKit surface). This lands at 67 words / 2 sentences; the 3 apostrophes, one
em dash, no-list, and first-person constraints still hold.

### Verification

#### Automated
- None in this phase — this is prose review; the test suite (Phase 3 loops) only
  asserts render/duplicate/emptiness, not the wording.

#### Manual
- [x] Reviewer approves the wording above against design.md "Draft copy
      direction" and conventions.md prose constraints.

---

## Phase 2: FAQ data layer — apply the swap

### Changes

#### 1. Replace the FAQ item in `const FAQS`

**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify (edit lines 48–49 of the "Features" category block)

Replace only the `question` and `answer` string literals of the existing
`FaqItem` at `web.rs:47-50`. Keep the `FaqItem { ... },` wrapper, `&'static str`
types, category, ordering, and 17-item count unchanged (count was 16 when this plan was
      written; an earlier merged PR added a "Why can't I filter reminders by
      flagged reminders?" FAQ).

**Before** (`web.rs:47-50`, verbatim from research Q5):

```rust
            FaqItem {
                question: "Why can't I see url or file attachments?",
                answer: "I would love to implement this but Apple's internal API for this has been broken for a while, if or when this is fixed I would be delighted to add it.",
            },
```

**After**:

```rust
            FaqItem {
                question: "Why can't I see some of my reminder details?",
                answer: "Apple doesn't give third-party apps access to everything in Reminders. I show the title, notes, due date, list, priority, and whether it repeats or has an alarm, but links, location, start dates, alarm times and sounds, completion times, time zones, and custom repeat schedules are not shown, and tags, flags, and file attachments stay behind Apple's doors — if they ever open, I'd love to show them all.",
            },
```

**Constraints honored** (no code change beyond the two literals):

- [x] `struct FaqItem` shape (`web.rs:8-16`) and `FAQS: &[FaqCategory]`
      (`web.rs:18`) — untouched. No new types, no signature changes.
- [x] Item stays in the "Features" category block (`web.rs:36-61`); item count
      remains 17 — 16 at plan time, plus a "Why can't I filter reminders by
      flagged reminders?" FAQ added by an earlier merged PR. 1-for-1
      replacement; do **not** add a hardcoded count — the count test was
      deliberately deleted in commit `ca47860`.
- [x] Downstream unchanged: handler serde-marshals `FAQS` to `serde_json::Value`
      (`web.rs:114-123`) → template loop (`templates/singlethread.html:91-100`).
      No template / route / ROUTES.md edit.

### Verification

#### Automated
- [x] `cargo nextest run -E 'test(faq_)'` passes — runs the FAQ suites in the
      inline test module (`web.rs:131-453`). Specifically:
  - [x] `faq_all_questions_appear` (`web.rs:257`) — new question renders.
  - [x] `faq_all_answers_appear` (`web.rs:280`) — new answer renders in escaped
        form (exercises the three `'` via the `html_escape` helper
        `web.rs:421`).
  - [x] `faq_items_all_non_empty` (`web.rs:432`) — no empty strings.
  - [x] `faq_items_no_duplicate_questions` (`web.rs:447`) — replacement didn't
        duplicate a question.
  - [x] `faq_items_grouped_under_category_headings` (`web.rs:370`) — item stays
        under "Features".
- [x] `cargo check --all-targets` passes (fast type-check gate; the full check
      runs again in Phase 3's `./scripts/test.sh`).

#### Manual
- [x] `git diff` shows exactly two changed lines (question + answer) in
      `web.rs`, nothing else.

---

## Phase 3: Verification layer — full gate + manual render

No code changes expected here. This stage proves the swap didn't drift CSS or
break the build.

### Changes

None — unless the CSS-drift check (below) unexpectedly fires.

### Verification

#### Automated
- [x] `./scripts/test.sh` passes end-to-end. Its steps, in order (per
      conventions.md):
  - [x] `cargo fmt --all` — clean (text-only edit produces no fmt change).
  - [x] `cargo sqlx prepare -- --tests` — no `.sqlx/` drift (no schema change).
  - [x] `cargo check --all-targets` — clean.
  - [x] `./scripts/build-css.sh` then `git diff --exit-code -- static/site.css` —
        **no CSS drift expected** (no Tailwind class changed). If this check
        *does* fire, run `./scripts/build-css.sh` and commit the regenerated
        `static/site.css` in the same change (AGENTS.md "Tests"), then re-run
        the gate.
  - [x] `cargo clippy --all-targets --all-features --locked -- -D warnings` —
        clean.
  - [x] `cargo nextest run` — all tests pass.
  - [x] TODO grep (`rg -i -s -g '*.rs' 'FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO\s|todo\s' src`)
        — no matches (fail-on-match).

#### Manual
- [ ] Boot locally (`cargo run`, serves on `http://localhost:3000`, per
      `src/main.rs:40-41`) and load `GET http://localhost:3000/singlethread`
      (route unchanged, `src/interfaces/routes.rs:49`):
  - [ ] New Q&A ("Why can't I see some of my reminder details?" + the
        consolidated answer) renders under the "Features" heading.
  - [ ] Old "url or file attachments" text is absent.
  - [ ] Exactly 17 `<details class="faq-item">` items remain (was 16 at plan time;
      an earlier merged PR added a "filter by flagged reminders" FAQ); surrounding
        `<details>`/`<summary>` structure and the closing CTA are intact.
  - [ ] Apostrophes render as `&#x27;` in the raw HTML (autoescape working),
        not raw `'`.

---

## Testing Checkpoints

1. **After Phase 1** — copy passes the prose-constraint checklist and reviewer
   approval → proceed to the swap.
2. **After Phase 2** — `cargo nextest run -E 'test(faq_)'` green (the 5 named
   loops) and item count still 17 → proceed to the full gate.
3. **After Phase 3** — `./scripts/test.sh` green with no CSS drift → done;
   commit the single-file change.

---

## Final-state note / deviations from `structure.md`

- **Draft copy enumerated 8 of the 10 values** (missing "time zone" and "custom
  repeat schedules"). Resolved in Phase 1 by including all 10 and trimming
  connective wording so the answer lands at 62 words / 2 sentences.
- **Review revision** reworded the answer to split "are not shown" (7 SDK-present
  values) from "behind Apple's doors" (tags/flags/file attachments); the final
  copy is 67 words / 2 sentences, 3 apostrophes, one em dash, no lists (see the
  Phase 1 review-revision note).
- **No new test assertions** are added, per design "Patterns to Follow" (count
  test deliberately deleted, `ca47860`); the existing `faq_*` loops close the
  loop on render/duplicate/emptiness.
- **Stage 2's "replace lines 48–49"** is realized as editing the two string
  literals inside the existing `FaqItem { ... }` at `web.rs:47-50` — the
  struct wrapper and `&'static str` types stay exactly as-is.
- Everything else follows `structure.md`'s three-stage order and checkpoints
  exactly; no schema/DB/service/API/template/route/CSS work is needed.