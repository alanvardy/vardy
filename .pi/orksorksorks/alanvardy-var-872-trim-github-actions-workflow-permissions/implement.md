# Implementation Summary

## Commits

| Phase | Commit | Description |
|-------|--------|-------------|
| 0     | `5a5514d` | Phase 0 (Layer 0): Verification harness baseline |
| 1     | `0244e01` | Phase 1 (Layer 1): fly-deploy.yml — explicit read-only posture |
| 2     | `07ec651` | Phase 2 (Layer 2): ci.yml — narrow baseline, escalate only the test job |
| 3     | `40bd2ce` | Phase 3 (Layer 3): dependabot_auto_merge.yml — trim, switch trigger, add guard |
| 4     | `ccdec54` | Phase 4 (Layer 4): cross-file audit + CI confirmation |
| —     | `f9be117` | (pre-work cleanup) Remove stray DELETEME file orphaned by the interrupted prior session |

All commits pushed to `origin/alanvardy-var-872-trim-github-actions-workflow-permissions` (PR #62, draft). Working tree clean.

## Automated Checks

- [x] Layer 0: actionlint baseline — exactly 2 known info-level SC2086 findings (`ci.yml:45:9`, `ci.yml:116:9` at baseline), zero error findings, across the 3 workflows.
- [x] Layer 0: exit-0 gate `actionlint -ignore 'SC2086'` prints `0`; the two grep detectors fire on the pre-change state (`actions: write` @ ci.yml:30, `contents: write` @ dependabot_auto_merge.yml:11).
- [x] Layer 1: `actionlint fly-deploy.yml` → exit 0; `contents: read` present at workflow level; no `GITHUB_TOKEN` reference introduced.
- [x] Layer 2: `actionlint ci.yml` → exit 0, only the 2 known SC2086 findings (post-edit at `ci.yml:46`/`:117`, shifted +1 by the new `test` job block), nothing on changed lines.
- [x] Layer 2: `grep "actions: write"` empty in ci.yml; `pull-requests: write` only inside the `test` job block; `contents: read` in exactly 3 places (baseline, test job, css-drift job).
- [x] Layer 3: `actionlint dependabot_auto_merge.yml` → 0 findings; `grep "contents: write"` empty; `pull_request_target` trigger present; `if: github.actor == 'dependabot[bot]'` guard present.
- [x] Layer 4: three-file exit-0 gate → `0`; full actionlint → only the 2 known SC2086 findings, nothing on changed lines.
- [x] Layer 4: `grep -R "actions: write" .github/workflows` → nothing; `grep -R "contents: write" .github/workflows` → only `rust-version-bump.yml:13` (documented out-of-scope follow-up).
- [x] Layer 4: workflow-level `permissions:` across the 3 files = `contents: read` only, except dependabot's justified `pull-requests: write` (+ `contents: read`).
- [x] Layer 4: `./scripts/test.sh` fully green — fmt → sqlx prepare → check → tailwind build (zero CSS drift) → clippy → 112/112 nextest tests → no forgotten TODOs.
- [x] Layer 4: diff audit — exactly 3 workflow files changed vs merge-base; no accidental edits elsewhere (`.pi/orksorksorks/` artifacts and plan.md checkbox flips are the only non-workflow diffs, matching repo precedent on origin/main).

## Plan Deviations (small, resolved during implementation)

- **Layer 3 comment rewording**: the plan-prescribed comment `contents: write is unreachable here` contained the literal substring `contents: write`, making the layer's own acceptance grep (and Layer 4's cross-file grep) unsatisfiable. Reworded the YAML comment and the plan.md change-block text to `a contents write token is unreachable here`. Grep semantics untouched.
- **SC2086 line drift**: the 2 known findings shifted `:45`/`:116` → `:46`/`:117` because Layer 2's `test` job block grew the file by 2 lines. Corrected the line numbers in plan.md Layer 2 (by the Phase 2 worker) and Layer 4 (by the main agent) so current-state assertions stay accurate.
- **Local test.db**: `./scripts/test.sh` initially failed `sqlx prepare` (missing local SQLite db). Recreated + migrated via `cargo sqlx database create && cargo sqlx migrate run`. `test.db` is gitignored — no tree pollution.

## Manual Verification Items (from the plan)

- [ ] L0: Confirm the two grep assertions show the *known* hits (ci.yml:30, dependabot_auto_merge.yml:11) — the harness is proven able to catch the drift it will later require to vanish.
- [ ] L1: Read the diff: `permissions: contents: read` sits at workflow level (between `on:` and `jobs:`); no other lines touched.
- [ ] L1: `./scripts/test.sh` still green (Rust gate untouched by a YAML-only change).
- [ ] L2: Read the diff: baseline is exactly the single `contents: read` line; `test` job block lists `contents: read` then `pull-requests: write`; `css-drift` untouched.
- [ ] L2: `./scripts/test.sh` green.
- [ ] L3: Read the full file: header comment preserved; `on: pull_request_target:`; permissions are `pull-requests: write` + `contents: read`; `if: github.actor == 'dependabot[bot]'`; action + inputs unchanged.
- [ ] L3: `./scripts/test.sh` green.
- [ ] L4: Open PR; confirm `push` (main) + `pull_request` CI jobs stay green, and `css-drift` + both codecov uploads succeed under the narrowed token.
- [ ] L4: Dependabot auto-merge left as post-merge follow-up — first dependabot PR after merge is the real functional confirmation (not gateable locally). Note this in the PR description.

## Out of scope (do NOT touch — documented follow-ups)

- `ci-secure.yml` (`packages: read` unconsumed) and `rust-version-bump.yml` (`contents: write` under MYTOKEN PAT) — both intentionally left as-is.
- No action-pinning changes (all SHA/tag/branch pins preserved, incl. `superfly/flyctl-actions@master`).
- No trigger/`concurrency:`/`env:`/secret changes except the dependabot trigger.