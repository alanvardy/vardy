# Structure Outline

> **Post-rebase note (large-review)**: this outline's "item count stays 16" /
> "exactly 16 items remain" references predate the VAR-782 flagged-reminder FAQ
> landing on the rebased branch; the final FAQ has 17 items (see `plan.md`).

## Approach

One-for-one replacement of a single FAQ item in the data-driven `const FAQS`
(`src/interfaces/handlers/singlethread/web.rs`) with a consolidated
"what can't I see" answer that enumerates the user-visible reminder values the
app cannot access/display. The design already bottomed out at the
presentational/content layer: schema, data-access, service, routes, and
templates are all untouched (design.md "What We're NOT Doing"). There is no
classic schema→store→service→API→UI stack to build — the "code" here **is**
the content. So the horizontal decomposition is three stages stacked on the
existing, already-green FAQ renderer and test harness: **(1) content**, **(2)
data swap**, **(3) verification**.

---

## Stage 1: FAQ content layer — finalize the copy

Delivers the final question + answer strings, meeting the prose conventions.
This is the only place the substance lives; it must be reviewable on its own
before any edit hits the const.

**Files**: none yet (copy lands in Stage 2)

**Key changes**:
- `question: &'static str` — keep the "Why can't I…" voice (design DD-6);
  direction: `"Why can't I see some of my reminder details?"`
- `answer: &'static str` — 1–3 sentences, first-person, **no list markers**;
  enumerate the 10 values as a comma-separated clause inside prose, not a
  bulleted/ordered list. Honest framing "Apple doesn't give third-party apps
  access" (drop the unverifiable "broken internal API" claim, design DD-3).
- **Stays raw**: apostrophes/slashes written literally — autoescape is applied
  at render, never pre-escaped in the const (`&#x27;`/`&#x2f;` is test-side only).

**Tests** (prose-constraint review, no code assertions):
- word count ≤ ~62 (11–62-word band, research.md Q1) — the design draft runs
  ~64 words, so trim to fit or confirm the ceiling is acceptable
  (design.md "Open Risks: Length vs. convention tension")
- 1–3 sentences; zero list markers (`-`, `*`, `1.`, `<ul>`, backticks)
- first-person founder voice; warm closing matching
  "…if Apple ever opens them up I would love to show them all"
- 1-for-1 replacement: 10 values listed, no internal EventKit metadata
  (`creationDate`, `attendees`, recurrence sub-fields — excluded, design DD-2)

**Verify**: manual copy review against design.md "Draft copy direction" and
`conventions.md` prose constraints. Gate: reviewer approves the wording.

---

## Stage 2: FAQ data layer — apply the swap

Delivers the edited `const FAQS`: old "url or file attachments" item removed,
new item in its place, item count stays 16, category stays "Features".

**Files**: `src/interfaces/handlers/singlethread/web.rs` (replace lines 48–49)

**Key changes**:
- `FAQS: &[FaqCategory]` — replace the `FaqItem` at `web.rs:48-49` (in the
  "Features" category block, `web.rs:36-61`) with the Stage-1 copy.
- `struct FaqItem { question: &'static str, answer: &'static str }` — shape
  unchanged; **no new types, no signature changes**.
- Downstream layer contract is unchanged: handler still serde-marshals `FAQS`
  to `serde_json::Value` (`web.rs:114-123`) → template loop
  (`templates/singlethread.html:91-100`). No template, route, or ROUTES.md edit.

**Tests** (existing suite — **no new assertions**; count test was deliberately
deleted, commit `ca47860`):
- `faq_all_questions_appear` — new question renders
- `faq_all_answers_appear` — new answer renders in escaped form (exercises any
  `'`/`/` in the copy via the `html_escape` helper)
- `faq_items_all_non_empty` — no empty strings
- `faq_items_no_duplicate_questions` — replacement didn't duplicate
- `faq_items_grouped_under_category_headings` — item stays under "Features"

**Verify**: replacement compiles and scratch tests pass —
`cargo nextest run -E 'test(faq_)'` (or just `cargo nextest run`). If the FAQ
checks are green, proceed to Stage 3.

---

## Stage 3: Verification layer — full gate + manual render

Delivers a green project gate plus an eyeballed rendered page. No new code is
expected here; this stage proves Stage-2's swap didn't drift CSS or break the
build.

**Files**: none (unless the CSS-drift check unexpectedly fires)

**Verify**:
- `./scripts/test.sh` — the full gate (fmt → `sqlx prepare -- --tests` →
  check → `build-css.sh` + `git diff --exit-code -- static/site.css` → clippy
  `-D warnings` → nextest → TODO grep). Text-only edit → **no CSS drift
  expected**; if the drift check fires, run `./scripts/build-css.sh` and commit
  the regenerated `static/site.css` (AGENTS.md "Tests").
- Manual/live: boot locally and load `GET /singlethread`
  (`src/interfaces/routes.rs:49`, unchanged) — confirm the new Q&A renders,
  old "url or file attachments" text is absent, exactly 16 items remain, and
  the surrounding `<details>`/`<summary>` structure is intact.

---

## Testing Checkpoints

1. **After Stage 1** — copy passes prose-constraint review → advance to the swap.
2. **After Stage 2** — `cargo nextest run` green on the FAQ tests (4 loops) and
   item count still 16 → advance to the full gate.
3. **After Stage 3** — `./scripts/test.sh` green with no CSS drift → done;
   commit the single-file change.

---

## Horizontal-applicability note

This design deliberately needs **no** schema/store/service/API layer and **no**
stub (design.md "Patterns to Follow" / "What We're NOT Doing"). The one real
"layer that gains behavior" is the FAQ content itself, and the existing
`faq_*` suite already closes the loop by auto-asserting every item's escaped
render and uniqueness — which is why no new test assertions are required. The
three stages above are the honest bottom-up ordering of that content-only
change, each with its own checkpoint.