# Task

Enable GitHub branch protection on `main` for alanvardy/vardy (VAR-860, remediation item REMEDIATION-1: `gh api repos/alanvardy/vardy/branches/main/protection` currently returns 404). Configure `required_status_checks` (strict=true) with contexts matching the actual CI check run names, `required_linear_history`, and stale-review dismissal via the GitHub REST API, verify the applied config with a GET, and keep existing automation (dependabot auto-merge, fly-deploy `workflow_run` on main pushes) working. Decide whether `allow_merge_commit=false` is appropriate now that the rebase/merge convention is documented, and make the change reproducible (in-repo script and/or doc note) rather than a throwaway manual PUT.

Note: the branch's draft PR (gh 59) is an empty placeholder (only touches DELETEME, currently uncommitted-deleted in the worktree); the real work described here is not yet done.

## Why LARGE

**CONVENTION_RISK** + **DESIGN_SIGN-OFF** (plus a mild **UNKNOWNS**): the change rewrites the repo's shared merge/CI convention — every future merge to `main` plus dependabot auto-merge and fly-deploy automation must keep working, required status-check contexts must exactly match the Actions check-run names from `.github/workflows/ci.yml` (test, todos, fmt, clippy, css-drift; naming mismatch blocks merges), and `allow_merge_commit=false` is a human-gated trade-off tied to the rebase-convention documentation (ticket 6).