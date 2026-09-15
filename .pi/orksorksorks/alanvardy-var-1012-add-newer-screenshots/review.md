## Review

Read: full diff, `src/interfaces/handlers/singlethread/web.rs` (tests), `src/interfaces/routes.rs` (routing + tests), `templates/singlethread.html`, `AGENTS.md`, `.gitignore`, `templates/layout.html`, repo-wide greps for `watch-list` / `singlethread-shot|singlethread-watch` / `.container`. All 11 tool calls used; not re-reading anything.

The change is correct and self-consistent: template, route-table test, 404 test, and page-content assertions all agree with the verified image contents; the retired asset has zero remaining references anywhere in the repo; the new 404 test is meaningful (ServeDir is mounted with no `.fallback()` at `src/interfaces/routes.rs:57-61`, so a missing file genuinely yields 404); the new HTML assertions use static (non-escaped) template text and short unique substrings per AGENTS.md conventions; the comment on web.rs:193-194 is substantively accurate (settings.jpg is a list screen, swipe.jpg is dark mode with the complete/skip action bar).

### BLOCKERS
none

### FIXES WORTH DOING NOW
- src/interfaces/handlers/singlethread/web.rs:193 — comment wording "copy corrects to match the VAR-1012 screenshots" is ungrammatical/ambiguous (likely "copy corrections" or "copy corresponds"); content is accurate, phrasing isn't. One-line fix in a line the diff already touches.

### OPTIONAL IMPROVEMENTS
- templates/singlethread.html:56-60 — the 4th figure can wrap: at the md breakpoint (`container` ≈ 768px per Tailwind defaults) four 14rem cards + 3×24px gaps ≈ 968px exceed the line, so the iPad card wraps alone onto a second row, left-aligned, sitting above the centered "On your wrist" row; add `justify-center` to the figure row or verify visually at tablet width (statically unconfirmable — `.container` widths live in minified site.css).
- templates/singlethread.html:58 — alt "One reminder at a time on iPad" nearly duplicates the first figure's alt (line 43, "…on iPhone"); screen-reader users hear two near-identical labels for different screens — consider describing the action bar instead (e.g. "…with Complete, Skip, and Delete buttons on iPad").
- .gitignore — `.pi/` is not ignored, so the three workflow-artifact `.md` files ship in the commit; add `.pi/` to `.gitignore` if the tracker doesn't want them tracked (per instructions they're documentation-only).

### IGNORE/DEFER
- Deleted `static/singlethread-watch-list.png`: repo-wide grep confirms no remaining references except the new 404 test (src/interfaces/routes.rs:252); since `asset_url()` panics on a registered-but-missing asset, any template re-introduction would fail the gate, so no negative page assertion is a genuine coverage gap.
- Removed `watch-list` presence assertion (web.rs) is not a coverage regression — it asserted an asset that no longer exists by design.
- ROUTES.md: correctly untouched — no route path or parameter changed, satisfying AGENTS.md's routes-doc rule.
- "Complete or skip" assertion is unambiguous (body copy uses lowercase "complete, skip, or delete"), and the settings/watch alt assertions match verified screen contents exactly.
- alt/figcaption duplication ("On iPad" caption vs alt text) and the single-card wrist row (flex `justify-center` centers a lone card cleanly) need no change.

**Merge verdict: OK** (with the two trivial nits under FIXES WORTH DOING NOW / OPTIONAL addressed at leisure; nothing blocks).