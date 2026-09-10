# Done

- **Branch / head SHA**: `alanvardy-var-872-trim-github-actions-workflow-permissions` / `467e2e3`
- **Mechanical checks**:
  - `actionlint` on all 3 workflows: 0 error findings, only the 2 known pre-existing `SC2086` info findings (mold-install `run:` blocks in `ci.yml`, never touched)
  - `actionlint -ignore 'SC2086'` exit-0 gate: `0`
  - `./scripts/test.sh` (Rust gate): fully green — fmt, sqlx prepare, check, CSS-drift (no diff), clippy `-D warnings`, 112/112 nextest tests, no forgotten TODOs
  - Grep sweep: `actions: write` gone repo-wide; `contents: write` at `dependabot_auto_merge.yml:15` (restored — see below) and `rust-version-bump.yml:13` (documented out of scope)
- **Review outcome**:
  - **Blocker found and fixed**: `enablePullRequestAutoMerge` (the GraphQL mutation called by `fastify/github-action-merge-dependabot@v3` under `use-github-auto-merge: true`) requires **both** `contents: write` and `pull-requests: write`. The repo's `research.md` Q4 conclusion — that `contents: write` is unreachable under `use-github-auto-merge` — was incorrect. `contents: write` has been restored with an accurate inline comment citing the GraphQL mutation name.
  - **NIT fixed**: stale `(:79)` line reference in `ci.yml:41` comment corrected to `(:85)` (`report_type: test_results` line after the Layer 2 test-job block insertion).
  - **All 4 parallel reviewers confirmed**: (1) `pull_request_target` trigger + actor guard are correct and secure (no checkout, no RCE surface); (2) ci.yml two-level permission pattern with replacement semantics is correct; (3) fly-deploy.yml placement is valid YAML; (4) no action pins loosened, no out-of-scope files touched.
- **Remaining manual items**:
  - Post-merge: confirm first dependabot PR actually auto-approves/auto-merges under the `pull_request_target` + `contents: write` + `pull-requests: write` configuration. This is the real runtime confirmation — not gateable locally.
  - `ci-secure.yml` (`packages: read` unconsumed) and `rust-version-bump.yml` (`contents: write` under MYTOKEN PAT) remain documented out-of-scope follow-ups.
  - PR CI must stay green with both codecov uploads succeeding under the narrowed `test`-job token.
