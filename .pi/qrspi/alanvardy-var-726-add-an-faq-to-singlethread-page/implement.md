# Implementation Summary

## Commits

| Phase | Commit | Description |
|-------|--------|-------------|
| 1 | daf4926 | Content Model — `FaqItem` struct + `FAQ_ITEMS` const + 3 sync unit tests |
| 2 | b1e11cb | CSS Component layer — `.faq-item` class (+ regenerated static/site.css) |
| 3 | 2d8ff15 | Template + Handler — wire FAQ `<details>/<summary>` into the page, update existing test, + 4 integration tests |
| 4 | f4a7cd7 | Documentation — update ROUTES.md `/singlethread` block |

## Automated Checks

- [x] `cargo test faq_items` — 3 sync unit tests pass (Phase 1)
- [x] `./scripts/test.sh` passes — 87/87 (Phases 1–2), then 91/91 after adding the 4 FAQ integration tests (Phases 3–4)
- [x] `./scripts/build-css.sh && git diff --exit-code -- static/site.css` — compiled CSS in sync, `.faq-item` rules present (Phase 2)
- [x] `cargo clippy --all-targets --all-features --locked -- -D warnings` — clean, no new warnings (Phase 3)
- [x] 4 new FAQ integration tests pass by name: `faq_all_questions_appear`, `faq_all_answers_appear`, `faq_section_after_quiet_productivity_before_cta`, `faq_no_javascript` (Phase 3)
- [x] arkitect architectural rules pass (Phase 3)
- [x] `cargo nextest run` — no test failures across all phases

## Adaptations (small divergences resolved during implementation)

1. **serde in interfaces layer (Phase 3):** The plan expected to pass `FAQ_ITEMS` directly through `context!`. The arkitect gate forbids the `serde` dependency in the `interfaces/` layer, so the handler marshals `FAQ_ITEMS` into a `Vec<serde_json::Value>` (serde_json is in the allowed list). Rendered output is identical to the plan's intent.
   - Consequence: Phase 1's transient `#[allow(dead_code)]` on `FaqItem`/`FAQ_ITEMS` (added to keep that isolated phase clippy-clean) is removed in Phase 3 once the const is actually used.
2. **Template line numbers (Phase 3):** `templates/singlethread.html` had an extra closing paragraph before the CTA. The FAQ section was inserted between the final "Built for quiet productivity" `<p>` and the `<p class="text-2xl text-accent text-center mt-12">` CTA — matching plan intent; the ordering test is green.
3. **minijinja HTML autoescape (Phase 3):** Answers containing `'` (`I'm`, `you're`, `you'll`) render as `&#x27;`. The new `faq_all_answers_appear` test asserts against a helper that reproduces minijinja's escaping. This is the documented divergence the plan's verification note anticipated.

## Manual Verification Items (from the plan)

- [ ] Review FAQ_ITEMS content for typos and factual accuracy (Phase 1)
- [ ] Confirm "Is SingleThread free?" and "How do I get started?" answers are acceptable (drafted by the agent; human to approve) (Phase 1)
- [ ] `git diff css/site.css` — review the new `.faq-item` rules for visual intent (Phase 2)
- [ ] `git diff static/site.css` — spot-check compiled output contains `.faq-item` rules (Phase 2)
- [ ] `./scripts/test.sh` output shows all 4 new FAQ tests passing by name (Phase 3)
- [ ] `cargo nextest run` output — no test failures (Phase 3)
- [ ] `git diff ROUTES.md` — review updated block, verify `###` … `---` self-contained convention (Phase 4)
- [ ] Open the SingleThread page in a browser (use `live-testing` skill) and visually confirm: FAQ heading between "Built for quiet productivity" and CTA; each Q&A collapsible via `<details>`; clicking a question expands/collapses; disclosure triangle hidden (plain clickable text); hover color transition works; looks right on mobile and wide viewports (Phase 4)