# Implementation Summary

Single-file FAQ content swap: the "Why can't I see url or file attachments?"
FAQ item in `const FAQS` (`src/interfaces/handlers/singlethread/web.rs:47-50`)
was replaced with a consolidated "Why can't I see some of my reminder details?"
answer enumerating the 10 user-visible reminder values SingleThread cannot
access. No schema, store, service, route, template, or CSS change.

## Commits

| Phase | Commit | Description |
|-------|--------|-------------|
| 1     | — (review-only, no commit) | FAQ content layer — copy finalized and approved |
| 2     | `17cd954` | FAQ data layer — apply the swap |
| 3     | `6299a86` | Verification layer — full gate |

Phase 1 produces no diff (the plan's own "After" copy was pasted verbatim in
Phase 2); its plan.md doc fixes and checkbox flips rode along in the Phase 2
and Phase 3 commits.

## Automated Checks

- [x] Phase 1 — delegated reviewer **APPROVE** verdict on the copy (computed,
      not assumed): 62 words (em dash excluded), 2 sentences, no list markers,
      first-person voice, exactly 10 enumerated non-internal values, honest
      "Apple doesn't give third-party apps access" framing, raw literals
      (3 apostrophes + 1 em dash, zero slashes). Byte-identity between the
      Phase 1 copy and the Phase 2 "After" block confirmed.
- [x] Phase 2 — `cargo nextest run -E 'test(faq_)'`: 8 passed, 0 failed
      (`faq_all_questions_appear`, `faq_all_answers_appear` exercising the
      apostrophes via `html_escape`, `faq_items_all_non_empty`,
      `faq_items_no_duplicate_questions`, `faq_items_grouped_under_category_headings`,
      `faq_summary_has_chevron`, `faq_section_after_quiet_productivity_before_cta`,
      `faq_no_javascript`).
- [x] Phase 2 — `cargo check --all-targets` clean.
- [x] Phase 2 — `git diff` of `web.rs` is exactly the two string literals,
      nothing else (`@@ -45,8 +45,8 @@`).
- [x] Phase 3 — `./scripts/test.sh` green end-to-end: `cargo fmt --all` clean;
      `cargo sqlx prepare -- --tests` zero `.sqlx/` drift; `cargo check
      --all-targets` clean; `./scripts/build-css.sh` + `git diff --exit-code
      -- static/site.css` — no CSS drift; `cargo clippy --all-targets
      --all-features --locked -- -D warnings` clean; `cargo nextest run` —
      112 passed, 0 failed; TODO grep — no matches.

## Manual Verification Items (from the plan)

Confirmed by the user (not checked off):

- [ ] Boot locally (`cargo run`, serves on `http://localhost:3000`, per
      `src/main.rs:40-41`) and load `GET http://localhost:3000/singlethread`
      (route unchanged, `src/interfaces/routes.rs:49`):
  - [ ] New Q&A ("Why can't I see some of my reminder details?" + the
        consolidated answer) renders under the "Features" heading.
  - [ ] Old "url or file attachments" text is absent.
  - [ ] Exactly 17 `<details class="faq-item">` items remain (was 16 at plan
        time — an earlier merged PR added a "filter by flagged reminders" FAQ);
        surrounding `<details>`/`<summary>` structure and the closing CTA are
        intact.
  - [ ] Apostrophes render as `&#x27;` in the raw HTML (autoescape working),
        not raw `'`.

## Verified deviations / observations (from the plan's final-state notes)

- **FAQ item count is 17, not 16.** The plan was snapshot against a 16-item
  FAQS; an earlier merged PR ("Why can't I filter reminders by flagged
  reminders?", `a419d32`) predates this branch, so the const had 17 items all
  along. The swap was 1-for-1 (count preserved), no test asserts a count (the
  count test was deliberately deleted, `ca47860`). `plan.md` now says 17.
- **Phase 1 note corrected** per the delegated review: the answer contains
  **3** apostrophes ("doesn't", "Apple's", "I'd") — the plan's "four" overcounts
  (four is only reached by adding the question's "can't"). Also reworded the
  "no `-` anywhere" checklist item — the mid-word hyphen in "third-party" is
  not a list marker. The copy itself was correct as planned.
- **No new test assertions added**, per design "Patterns to Follow"; the
  existing `faq_*` loops cover render/duplicate/emptiness.
- **Push note**: the Phase 2 push required `--force-with-lease` — the mandated
  `git rebase origin/main` rewrote branch history; verified the remote carried
  no unique content before force-pushing. Phase 3 pushed clean fast-forward.
- **Transient sqlx incident (resolved)**: on the fresh checkout `test.db` did
  not exist; the Phase 3 worker's first prepare run deleted the committed
  `.sqlx/query-*.json` metadata. Restored via `git checkout -- .sqlx`,
  provisioned the DB with `cargo sqlx database setup`, and the final gate
  confirmed regenerated metadata is byte-identical to committed (no drift).
- **No refactors or out-of-scope edits**. The five QRSPI artifact docs
  (`conventions.md`, `design.md`, `implement.md`, `research.md`, `structure.md`)
  were left untracked by the implementation phases; they are committed in the
  large-review pass so the PR is self-contained (QRSPI convention: "each phase
  commits its own artifact").