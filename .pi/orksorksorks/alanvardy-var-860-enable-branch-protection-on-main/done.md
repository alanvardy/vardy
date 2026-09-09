# Done

- **Branch / head SHA**: alanvardy-var-860-enable-branch-protection-on-main / 43d7940
- **Mechanical checks**: `./scripts/test.sh` passed (112/112 tests, format, clippy, CSS drift, TODO lint all clean)
- **Review outcome**: all blockers and fixes worth doing now applied, optional improvements applied:

### Blockers fixed
- B1: Removed DELETEME (branch spin-up marker)

### Fixes applied
- F1: Added `--yes` flag for non-interactive apply; `read` on EOF now shows clear abort message
- F2: `verify_protection` now checks `allow_force_pushes` and `allow_deletions`
- F3: AGENTS.md wording fixed: "disabled repo-wide", clarifies apply pushes config, verify is read-only
- F4: Added `usage()` function and `--help`/`-h` support
- F5: Added remediation hint after contexts mismatch diff

### Optional improvements applied
- O1: Added header comment describing script purpose, dependencies, and canonical-list convention
- O2: `BRANCH` derived from `origin/HEAD` with fallback to "main"
- O3: `REPO` validation after git remote parsing
- O4: Dynamic context count (`${#REQUIRED_CONTEXTS[@]}`) instead of hardcoded "6"
- O5: Dependency checks for `gh` and `jq` at script start
- O8: `apply_protection` and `apply_repo_settings` guarded with error hints on partial failure
- O9: Empty `REQUIRED_CONTEXTS` array now caught early

### Deferred
- O6: Consolidate ~9 API calls into fewer GETs (low risk, low value right now)
- O7: Periodic CI verification workflow (separate ticket)

- **Remaining manual items per plan.md**:
  - Merge this PR with `gh pr merge <n> --rebase --delete-branch`
  - Verify via `gh api repos/alanvardy/vardy/branches/main/protection | jq` that new fields (allow_force_pushes, allow_deletions) match
  - Verify via `gh api repos/alanvardy/vardy --jq` that repo merge settings match