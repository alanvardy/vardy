# Implementation Plan

## Overview

Add branch protection to `main` for `alanvardy/vardy` via an idempotent bash script (`scripts/branch-protection.sh`) using `gh api`, plus a one-line convention note in `AGENTS.md`. Three layers: verify script → apply logic → docs.

---

## Phase 1: Read-only verification script

### Changes

#### 1. Create `scripts/branch-protection.sh` — verify path only
**File**: `scripts/branch-protection.sh`
**Action**: create

```bash
#!/usr/bin/env bash
set -euo pipefail

# ── Constants ───────────────────────────────────────────────────────────────
REQUIRED_CONTEXTS=(
  "CI / Cargo CI Tests"
  "CI / TODO and FIXME"
  "CI / Rust-fmt (Cargo Format)"
  "CI / Clippy (Cargo Clippy Lint Check)"
  "CI / CSS Drift Check"
  "Dependabot Auto Merge / auto-merge"
)

# ── Helpers ─────────────────────────────────────────────────────────────────
REPO=$(git remote get-url origin | sed 's|.*github\.com[:/]||; s|\.git$||')
OWNER="${REPO%%/*}"
REPO_NAME="${REPO##*/}"
BRANCH="main"

die() { echo "ERROR: $*" >&2; exit 1; }

# ── Verify: branch protection ───────────────────────────────────────────────
verify_protection() {
  local resp
  resp=$(gh api --silent "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" 2>&1) || {
    if echo "$resp" | grep -qi "Not Found"; then
      die "branch protection NOT CONFIGURED on $BRANCH (GET returned 404)"
    fi
    die "failed to GET protection: $resp"
  }

  local mismatches=0

  # required_status_checks.strict
  local strict
  strict=$(gh api --silent "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
    --jq '.required_status_checks.strict')
  if [ "$strict" != "true" ]; then
    echo "FAIL: required_status_checks.strict = $strict (expected true)"
    mismatches=$((mismatches + 1))
  fi

  # required_status_checks.contexts (sorted compare)
  local current_ctxs expected_ctxs
  current_ctxs=$(gh api --silent "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
    --jq '[.required_status_checks.contexts[]] | sort | join("\n")')
  expected_ctxs=$(printf '%s\n' "${REQUIRED_CONTEXTS[@]}" | sort)
  if [ "$current_ctxs" != "$expected_ctxs" ]; then
    echo "FAIL: required_status_checks.contexts mismatch"
    diff <(echo "$expected_ctxs") <(echo "$current_ctxs") || true
    mismatches=$((mismatches + 1))
  fi

  # required_linear_history
  local linear
  linear=$(gh api --silent "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
    --jq '.required_linear_history.enabled')
  if [ "$linear" != "true" ]; then
    echo "FAIL: required_linear_history.enabled = $linear (expected true)"
    mismatches=$((mismatches + 1))
  fi

  # required_pull_request_reviews
  local dismiss_stale count
  dismiss_stale=$(gh api --silent "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
    --jq '.required_pull_request_reviews.dismiss_stale_reviews')
  count=$(gh api --silent "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
    --jq '.required_pull_request_reviews.required_approving_review_count')
  if [ "$dismiss_stale" != "true" ]; then
    echo "FAIL: required_pull_request_reviews.dismiss_stale_reviews = $dismiss_stale (expected true)"
    mismatches=$((mismatches + 1))
  fi
  if [ "$count" != "0" ]; then
    echo "FAIL: required_pull_request_reviews.required_approving_review_count = $count (expected 0)"
    mismatches=$((mismatches + 1))
  fi

  # enforce_admins
  local enforce
  enforce=$(gh api --silent "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
    --jq '.enforce_admins.enabled')
  if [ "$enforce" != "false" ]; then
    echo "FAIL: enforce_admins.enabled = $enforce (expected false)"
    mismatches=$((mismatches + 1))
  fi

  if [ "$mismatches" -gt 0 ]; then
    die "branch protection verification FAILED — $mismatches mismatch(es)"
  fi
  echo "OK: branch protection matches expected config"
}

# ── Verify: repo merge settings ─────────────────────────────────────────────
verify_repo_settings() {
  local mismatches=0

  local merge_commit squash_merge rebase_merge
  merge_commit=$(gh api --silent "repos/$OWNER/$REPO_NAME" --jq '.allow_merge_commit')
  squash_merge=$(gh api --silent "repos/$OWNER/$REPO_NAME" --jq '.allow_squash_merge')
  rebase_merge=$(gh api --silent "repos/$OWNER/$REPO_NAME" --jq '.allow_rebase_merge')

  if [ "$merge_commit" != "false" ]; then
    echo "FAIL: allow_merge_commit = $merge_commit (expected false)"
    mismatches=$((mismatches + 1))
  fi
  if [ "$squash_merge" != "false" ]; then
    echo "FAIL: allow_squash_merge = $squash_merge (expected false)"
    mismatches=$((mismatches + 1))
  fi
  if [ "$rebase_merge" != "true" ]; then
    echo "FAIL: allow_rebase_merge = $rebase_merge (expected true)"
    mismatches=$((mismatches + 1))
  fi

  if [ "$mismatches" -gt 0 ]; then
    die "repo merge settings verification FAILED — $mismatches mismatch(es)"
  fi
  echo "OK: repo merge settings match expected config (rebase-only)"
}

# ── Apply: branch protection ────────────────────────────────────────────────
apply_protection() {
  # Build contexts JSON array
  local ctx_json
  ctx_json=$(printf '%s\n' "${REQUIRED_CONTEXTS[@]}" | jq -R . | jq -s .)

  gh api --silent -X PUT "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
    --input - <<EOF
{
  "required_status_checks": {
    "strict": true,
    "contexts": $ctx_json
  },
  "enforce_admins": false,
  "required_pull_request_reviews": {
    "dismiss_stale_reviews": true,
    "required_approving_review_count": 0
  },
  "required_linear_history": true,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "restrictions": null
}
EOF
  echo "OK: branch protection applied"
}

# ── Apply: repo merge settings ──────────────────────────────────────────────
apply_repo_settings() {
  gh api --silent -X PATCH "repos/$OWNER/$REPO_NAME" --input - <<EOF
{
  "allow_merge_commit": false,
  "allow_squash_merge": false,
  "allow_rebase_merge": true
}
EOF
  echo "OK: repo merge settings applied"
}

# ── Main ────────────────────────────────────────────────────────────────────
main() {
  local cmd="${1:-verify}"

  case "$cmd" in
    verify)
      echo "=== Verifying branch protection for $REPO/$BRANCH ==="
      verify_protection
      echo ""
      verify_repo_settings
      echo ""
      echo "All checks passed."
      ;;
    apply)
      echo "=== Apply branch protection to $REPO/$BRANCH ==="
      echo ""
      echo "This will:"
      echo "  - Require 6 status checks to pass before merging to $BRANCH"
      echo "  - Require linear history (no merge commits, no squashes)"
      echo "  - Enable stale review dismissal (0 required approvals)"
      echo "  - Disable merge-commit and squash-merge at the repo level"
      echo "  - Allow rebase merges only"
      echo ""
      read -r -p "Continue? [y/N] " answer
      if [ "$answer" != "y" ] && [ "$answer" != "Y" ]; then
        echo "Aborted."
        exit 0
      fi
      echo ""

      apply_protection
      apply_repo_settings
      echo ""

      echo "=== Re-verifying after apply ==="
      verify_protection
      echo ""
      verify_repo_settings
      echo ""
      echo "Done — branch protection and repo merge settings applied and verified."
      ;;
    *)
      die "unknown command: $cmd (expected verify or apply)"
      ;;
  esac
}

main "$@"
```

**Note on `jq` dependency**: the `apply_protection` function uses `jq` to build the contexts JSON array for the heredoc. `jq` is pre-installed on macOS and available in CI/GitHub Actions runners. The `verify_*` functions use only `gh api --jq` (built-in).

### Verification

#### Automated
- [x] `./scripts/test.sh` passes (unchanged — no Rust/Tailwind changes; this gate confirms nothing broke)
- [x] `bash scripts/branch-protection.sh verify` exits non-zero with "branch protection NOT CONFIGURED on main (GET returned 404)" — proves the verifier correctly detects the current un-configured state

#### Manual
- [ ] Read the script output and confirm it cleanly reports the 404 (no `gh` auth errors, no bash syntax errors)
- [ ] Run `bash scripts/branch-protection.sh` (no args) — defaults to `verify`, same output as above

---

## Phase 2: Idempotent apply

### Changes

#### 1. `scripts/branch-protection.sh` — `apply` subcommand is already in the file
**File**: `scripts/branch-protection.sh`
**Action**: no changes (the full script was written in Phase 1; the `apply` path was already present but the verify path is the only one that would succeed)

Phase 2 is the **execution** of the script's `apply` subcommand, which:
1. Prompts for interactive confirmation
2. PUTs branch protection config to `repos/{owner}/{repo}/branches/main/protection`
3. PATCHes repo merge settings to `repos/{owner}/{repo}`
4. Re-runs `verify_protection` and `verify_repo_settings` to confirm the API now returns the expected config
5. Exits 0 on success, non-zero on any mismatch

### Verification

#### Automated
- [x] `./scripts/test.sh` passes (unchanged)
- [x] `bash scripts/branch-protection.sh verify` exits 0 — all fields match expected config

#### Manual
- [ ] `bash scripts/branch-protection.sh apply` — confirm interactive prompt, then confirm all verify steps pass
- [ ] Run `bash scripts/branch-protection.sh apply` a **second time** — must exit 0 with "All checks passed" (idempotent; no double-apply side effects)
- [ ] Verify via `gh api repos/alanvardy/vardy/branches/main/protection | jq`:
  - `.required_status_checks.strict` is `true`
  - `.required_status_checks.contexts` contains all 6 expected contexts
  - `.required_linear_history.enabled` is `true`
  - `.required_pull_request_reviews.dismiss_stale_reviews` is `true`
  - `.required_pull_request_reviews.required_approving_review_count` is `0`
  - `.enforce_admins.enabled` is `false`
- [ ] Verify via `gh api repos/alanvardy/vardy --jq '{merge_commit: .allow_merge_commit, squash_merge: .allow_squash_merge, rebase_merge: .allow_rebase_merge}'`:
  - `merge_commit` is `false`
  - `squash_merge` is `false`
  - `rebase_merge` is `true`
- [ ] Sanity-check live merge flow: merge this PR with `gh pr merge <n> --rebase --delete-branch` — must succeed (the required checks on this PR should pass and the rebase should work)

---

## Phase 3: Document the convention

### Changes

#### 1. Update `AGENTS.md` — "Commits and PRs" section
**File**: `AGENTS.md`
**Action**: modify

**Old text** (lines 46–49):
```markdown
## Commits and PRs
- `main` is the base branch when reviewing code
- If a session resumes onto a branch with uncommitted changes, treat them
  as suspect (orphans from an interrupted session) — compare against
  `plan.md` and the Linear ticket before keeping or reverting
```

**New text**:
```markdown
## Commits and PRs
- `main` is the base branch when reviewing code
- If a session resumes onto a branch with uncommitted changes, treat them
  as suspect (orphans from an interrupted session) — compare against
  `plan.md` and the Linear ticket before keeping or reverting
- Merge PRs only with `--rebase` (`gh pr merge <n> --rebase --delete-branch`);
  merge-commit and squash merges are disabled on `main`
- Branch protection on `main` is managed by `scripts/branch-protection.sh` —
  run it after any CI job `name:` change; the script is the canonical list of
  required-check contexts
```

### Verification

#### Automated
- [x] `./scripts/test.sh` passes (unchanged — no Rust/Tailwind changes; confirms nothing broke)

#### Manual
- [ ] Re-read the "Commits and PRs" section in `AGENTS.md` — confirm the two new bullets are present and accurate
- [ ] Confirm no other doc file needs the same note (per research Q3: no `CONTRIBUTING.md`, no `docs/`; only `AGENTS.md`, `README.md`, `ROUTES.md`)

---

## Completion Gate

- [x] `./scripts/test.sh` passes
- [x] `bash scripts/branch-protection.sh verify` exits 0
- [x] `gh api repos/alanvardy/vardy/branches/main/protection` returns the expected config (all 6 checks, linear history, review config, `enforce_admins: false`)
- [x] `gh api repos/alanvardy/vardy` shows `allow_merge_commit: false`, `allow_squash_merge: false`, `allow_rebase_merge: true`
- [x] `AGENTS.md` "Commits and PRs" section documents rebase-only convention and points at the script
- [ ] Commit contains both `scripts/branch-protection.sh` and `AGENTS.md` changes