#!/usr/bin/env bash
# Verify and apply GitHub branch protection and repo merge settings for main.
#   verify  (default) — read-only check that protection + merge settings match
#   apply   — PUT protection + PATCH merge settings, then re-verify
# Requires: gh CLI (authenticated with repo/admin scope), jq (apply path only).
# The REQUIRED_CONTEXTS array below is the canonical list of required CI check
# contexts. After any CI job name: change in .github/workflows/, update
# REQUIRED_CONTEXTS and re-run `apply`.
set -euo pipefail

# ── Pre-flight ──────────────────────────────────────────────────────────────
command -v gh >/dev/null || die "gh CLI (with repo scope) is required — see https://cli.github.com"
command -v jq >/dev/null || die "jq is required"

# ── Constants ───────────────────────────────────────────────────────────────
REQUIRED_CONTEXTS=(
  "CI / Cargo CI Tests"
  "CI / TODO and FIXME"
  "CI / Rust-fmt (Cargo Format)"
  "CI / Clippy (Cargo Clippy Lint Check)"
  "CI / CSS Drift Check"
  "Dependabot Auto Merge / auto-merge"
)

[ ${#REQUIRED_CONTEXTS[@]} -gt 0 ] || die "REQUIRED_CONTEXTS is empty"

# ── Helpers ─────────────────────────────────────────────────────────────────
REPO=$(git remote get-url origin | sed 's|.*github\.com[:/]||; s|\.git$||')
[[ "$REPO" =~ ^[^/]+/[^/]+$ ]] || die "cannot determine GitHub owner/repo from origin remote"
OWNER="${REPO%%/*}"
REPO_NAME="${REPO##*/}"
BRANCH=$(git symbolic-ref --short refs/remotes/origin/HEAD 2>/dev/null | sed 's|^origin/||' || echo main)

die() { echo "ERROR: $*" >&2; exit 1; }

usage() {
  cat <<EOF
Usage: scripts/branch-protection.sh [verify|apply|--yes]

  verify  (default) read-only check that main protection + repo merge
          settings match REQUIRED_CONTEXTS
  apply   PUT protection + PATCH merge settings, then re-verify
          (interactive unless --yes)
  --yes   non-interactive skip of the apply confirmation prompt

Examples:
  scripts/branch-protection.sh              # verify only
  scripts/branch-protection.sh verify       # explicit verify
  scripts/branch-protection.sh apply        # interactive apply
  scripts/branch-protection.sh apply --yes  # non-interactive apply
EOF
}

# ── Verify: branch protection ───────────────────────────────────────────────
verify_protection() {
  local resp
  resp=$(gh api --silent "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" 2>&1) || {
    if echo "$resp" | grep -qiE "not found|not protected"; then
      die "branch protection NOT CONFIGURED on $BRANCH (GET returned 404)"
    fi
    die "failed to GET protection: $resp"
  }

  local mismatches=0

  # required_status_checks.strict
  local strict
  strict=$(gh api "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
    --jq '.required_status_checks.strict')
  if [ "$strict" != "true" ]; then
    echo "FAIL: required_status_checks.strict = $strict (expected true)"
    mismatches=$((mismatches + 1))
  fi

  # required_status_checks.contexts (sorted compare)
  local current_ctxs expected_ctxs
  current_ctxs=$(gh api "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
    --jq '[.required_status_checks.contexts[]] | sort | join("\n")')
  expected_ctxs=$(printf '%s\n' "${REQUIRED_CONTEXTS[@]}" | LC_ALL=C sort)
  if [ "$current_ctxs" != "$expected_ctxs" ]; then
    echo "FAIL: required_status_checks.contexts mismatch"
    echo "  Fix: update REQUIRED_CONTEXTS in scripts/branch-protection.sh (canonical list)"
    echo "       and/or the job name in .github/workflows/*.yml, then re-run verify"
    diff <(echo "$expected_ctxs") <(echo "$current_ctxs") || true
    mismatches=$((mismatches + 1))
  fi

  # required_linear_history
  local linear
  linear=$(gh api "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
    --jq '.required_linear_history.enabled')
  if [ "$linear" != "true" ]; then
    echo "FAIL: required_linear_history.enabled = $linear (expected true)"
    mismatches=$((mismatches + 1))
  fi

  # required_pull_request_reviews
  local dismiss_stale count
  dismiss_stale=$(gh api "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
    --jq '.required_pull_request_reviews.dismiss_stale_reviews')
  count=$(gh api "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
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
  enforce=$(gh api "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
    --jq '.enforce_admins.enabled')
  if [ "$enforce" != "false" ]; then
    echo "FAIL: enforce_admins.enabled = $enforce (expected false)"
    mismatches=$((mismatches + 1))
  fi

  # allow_force_pushes
  local force_pushes
  force_pushes=$(gh api "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
    --jq '.allow_force_pushes.enabled')
  if [ "$force_pushes" != "false" ]; then
    echo "FAIL: allow_force_pushes.enabled = $force_pushes (expected false)"
    mismatches=$((mismatches + 1))
  fi

  # allow_deletions
  local deletions
  deletions=$(gh api "repos/$OWNER/$REPO_NAME/branches/$BRANCH/protection" \
    --jq '.allow_deletions.enabled')
  if [ "$deletions" != "false" ]; then
    echo "FAIL: allow_deletions.enabled = $deletions (expected false)"
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
  merge_commit=$(gh api "repos/$OWNER/$REPO_NAME" --jq '.allow_merge_commit')
  squash_merge=$(gh api "repos/$OWNER/$REPO_NAME" --jq '.allow_squash_merge')
  rebase_merge=$(gh api "repos/$OWNER/$REPO_NAME" --jq '.allow_rebase_merge')

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
  local yes_flag=false

  # Support --yes anywhere in args
  for arg in "$@"; do
    if [ "$arg" = "--yes" ] || [ "$arg" = "-y" ]; then
      yes_flag=true
    fi
  done

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
      echo "  - Require ${#REQUIRED_CONTEXTS[@]} status checks to pass before merging to $BRANCH"
      echo "  - Require linear history (no merge commits, no squashes)"
      echo "  - Enable stale review dismissal (0 required approvals)"
      echo "  - Disable merge-commit and squash-merge at the repo level"
      echo "  - Allow rebase merges only"
      echo ""
      if [ "$yes_flag" != true ]; then
        read -r -p "Continue? [y/N] " answer || { echo "Aborted (no stdin). Re-run with --yes for non-interactive apply."; exit 0; }
        if [ "$answer" != "y" ] && [ "$answer" != "Y" ]; then
          echo "Aborted."
          exit 0
        fi
      fi
      echo ""

      apply_protection || die "protection apply failed — branch may be partially configured; fix and re-run apply (idempotent)"
      apply_repo_settings || die "repo settings apply failed — branch protection IS applied; re-run apply to finish"
      echo ""

      echo "=== Re-verifying after apply ==="
      verify_protection
      echo ""
      verify_repo_settings
      echo ""
      echo "Done — branch protection and repo merge settings applied and verified."
      ;;
    help|--help|-h)
      usage
      exit 0
      ;;
    --yes|-y)
      # --yes without a subcommand defaults to apply in non-interactive mode
      echo "=== Apply branch protection to $REPO/$BRANCH (non-interactive) ==="
      echo ""
      echo "This will:"
      echo "  - Require ${#REQUIRED_CONTEXTS[@]} status checks to pass before merging to $BRANCH"
      echo "  - Require linear history (no merge commits, no squashes)"
      echo "  - Enable stale review dismissal (0 required approvals)"
      echo "  - Disable merge-commit and squash-merge at the repo level"
      echo "  - Allow rebase merges only"
      echo ""

      apply_protection || die "protection apply failed — branch may be partially configured; fix and re-run apply (idempotent)"
      apply_repo_settings || die "repo settings apply failed — branch protection IS applied; re-run apply to finish"
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