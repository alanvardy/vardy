## Review

**Verified:** Change is correct and self-consistent. Repo-wide greps for `watch-list` / `retired` hit only the three new references (`web.rs:198`, `routes.rs:223`, `templates/singlethread.html:67`) plus `.pi/` session-scratch docs — nothing left assumes the asset is absent. The retired-404 test's deletion is correct: the asset now exists, so a 404 assertion is obsolete; serving coverage is retained via the table test (content-type `image/png` + `max-age=31536000` at `routes.rs:216-240`). Alt text matches the verified image (reminder + Complete green check + Skip orange slashed circle), is static template text (no minijinja escaping nuance), and uses short unique substrings per AGENTS.md. Layout is a non-issue: aspect ratios are 502/410 ≈ 1.224 vs 440/359 ≈ 1.226 (~0.1% apart), so at equal `max-w-[12rem]` width the two cards render to within <1px of the same height. No new CSS classes (verbatim reuse), no route change (ROUTES.md untouched is correct), `.sqlx` metadata untouched is correct.

### BLOCKERS
none

### FIXES WORTH DOING NOW
none

### OPTIONAL IMPROVEMENTS
- templates/singlethread.html:67 — the new img omits `width`/`height` attributes, so the browser reports layout shift on load; consistent with the sibling watch-detail card, so optional only.

### IGNORE/DEFER
- `.pi/orksorksorks/alanvardy-var-1012-add-newer-screenshots/plan.md:34`, `implement.md:20`, `done.md:71` — scratch worklogs still describe the shot as "retired" (records of superseded commit 267853b); they're untracked per-session memory, not repo deliverables, and the gate doesn't read them, so no action in this delta.
- templates/singlethread.html:66-71 — wrist-row cards have no figcaption while the upper "four screenshots" row uses figures with captions; pre-existing style choice (watch-detail sibling is also caption-less), not introduced here.

Merge verdict: **OK**