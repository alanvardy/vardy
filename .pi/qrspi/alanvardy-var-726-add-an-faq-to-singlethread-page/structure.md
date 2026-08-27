# Structure Outline

## Approach

Add a static FAQ section to the SingleThread page using native HTML `<details>/<summary>` disclosure widgets, driven by a `Vec<FaqItem>` defined in the handler and styled with a minimal `.faq-item` component class. No JS, no DB migration, no new route — content is `&'static str` data passed through the existing minijinja render chokepoint.

---

## Stage 1: Content Model — `FaqItem` struct + FAQ data

Define the FAQ content as a Rust struct and a static constant, laying the data foundation that every later stage consumes. This stage proves the content is well-formed before any rendering code touches it.

**Files**: `src/interfaces/handlers/singlethread/web.rs`

**Key changes**:
- `struct FaqItem { question: &'static str, answer: &'static str }` — new struct in the handler module (above `pub async fn index`)
- `const FAQ_ITEMS: &[FaqItem] = &[...]` — 11 Q&A pairs (the 9 provided + "Is SingleThread free?" + "How do I get started?")

**Tests** (inline `#[cfg(test)] mod tests`):
- `faq_items_all_non_empty` — every question and answer string is non-empty
- `faq_items_count` — exactly 11 items
- `faq_items_no_duplicate_questions` — all questions are unique

**Verify**: `./scripts/test.sh` passes (existing tests unchanged; new unit tests green; no unused-code warnings since tests reference the types)

---

## Stage 2: CSS Component Layer — `.faq-item` class

Add the CSS class that styles the FAQ disclosure widgets. The class normalizes `<details>/<summary>` across browsers and matches the site's visual design. This is a pure-presentation layer that the template will reference next.

**Files**: `css/site.css`

**Key changes**:
- New block in `@layer components` (~10 lines):
  ```css
  .faq-item {
    /* normalizes <details>/<summary> disclosure triangle across browsers */
    /* matches site typography + color tokens */
  }
  .faq-item summary { ... }  /* cursor: pointer, color-muted heading */
  .faq-item .faq-answer { ... }  /* text-muted padding for answer body */
  ```
- Reuse existing CSS custom properties: `var(--color-muted)`, `var(--color-accent)`, `var(--color-surface)`
- Reuse existing Tailwind utilities in template (spacing, typography) — CSS layer only handles component-specific interaction states

**Tests**: CSS drift gate — `./scripts/build-css.sh && git diff --exit-code -- static/site.css` — verifies `static/site.css` is in sync with `css/site.css`

**Verify**: `./scripts/test.sh` passes (drift check green; existing tests still green — no template changes yet, so no rendered HTML to assert)

---

## Stage 3: Template + Handler — wire FAQ into the page

This is the integration stage where the content model and CSS layer meet. The template iterates `FaqItem` vec and emits `<details>/<summary>` markup; the handler passes the vec through the render context. Both land together because neither can be tested independently — the template needs the context variable, and the handler test renders the full page.

**Files**:
- `templates/singlethread.html`
- `src/interfaces/handlers/singlethread/web.rs`

**Key changes**:

Template (`singlethread.html`):
- Insert after line 88 (`</p>` of "Built for quiet productivity") and before line 89 (`<p class="text-2xl text-accent text-center mt-12">`):
  ```html
  <h2 class="heading-section">Frequently Asked Questions</h2>
  <div class="space-y-3">
    {% for item in faq_items %}
    <details class="faq-item">
      <summary>{{ item.question }}</summary>
      <p class="faq-answer text-muted mt-2">{{ item.answer }}</p>
    </details>
    {% endfor %}
  </div>
  ```

Handler (`web.rs`):
- `pub async fn index(...)` — add `faq_items => FAQ_ITEMS` to the `context!{...}` macro call (line ~14)

**Tests** (update existing + add new in `web.rs`):

Update existing:
- `index_serves_ok_html` — add `body.contains("Frequently Asked Questions")` after the "Built for quiet productivity" assert; add one question-text + one answer-text assert as a string probe (e.g. `"Where is my data stored?"` and its answer snippet `"stays on your device"`); verify `<details>` and `<summary>` tags appear in body

New tests:
- `faq_all_questions_appear` — iterate `FAQ_ITEMS` and assert each `.question` string appears in rendered body
- `faq_all_answers_appear` — same for each `.answer` string
- `faq_section_after_quiet_productivity_before_cta` — assert "Built for quiet productivity" appears before "Frequently Asked Questions" appears before "Your reminders. One at a time." (CTA)
- `faq_no_javascript` — assert body does NOT contain `<script` or `onclick`

**Verify**: `./scripts/test.sh` passes (all existing tests updated + new FAQ tests green; clippy clean; CSS drift still green — `.faq-item` is already in `static/site.css` from Stage 2)

---

## Stage 4: Documentation — update `ROUTES.md`

Document the new FAQ section in the `/singlethread` endpoint block.

**Files**: `ROUTES.md`

**Key changes**:
- In the `### GET /singlethread` block (lines 22–37), add one sentence after "feature lists, and a closing CTA line" (line 26): "an FAQ section with collapsible Q&A pairs (native `<details>/<summary>` widgets),"

**Tests**: Manual review — visual diff of the updated `ROUTES.md` block; verify it still follows the `###` … `---` self-contained convention

**Verify**: `./scripts/test.sh` passes (no code changes; gate is a no-op that confirms ROUTES.md is syntactically fine); live-testing visual check via `live-testing` skill to confirm page renders correctly in a browser

---

## Testing Checkpoints

1. **After Stage 1**: `./scripts/test.sh` green — `faq_items_*` unit tests pass; existing tests unchanged
2. **After Stage 2**: `./scripts/build-css.sh && git diff --exit-code -- static/site.css` clean; `./scripts/test.sh` green
3. **After Stage 3**: `./scripts/test.sh` green — all existing tests updated, new FAQ integration tests pass, CSS drift still clean
4. **After Stage 4**: `./scripts/test.sh` green; ROUTES.md diff reviewed; browser visual check passes

---

## What Is NOT Layered (by design)

- **No DB migration / store layer.** FAQ content is static `&'static str` — zero runtime allocations, no tables, no `sqlx migrate add`. This matches the rest of the SingleThread page (all content is literal strings in the template).
- **No service/business-logic layer.** No validation, no processing, no external calls. The FAQ data is a constant.
- **No new route / API layer.** The FAQ is part of the existing `GET /singlethread` handler — one more context variable, same chokepoint.