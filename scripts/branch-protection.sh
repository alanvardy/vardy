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