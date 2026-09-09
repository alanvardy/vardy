# Implementation Summary

## Commits

| Phase | Commit | Description |
|-------|--------|-------------|
| 1     | `e06e685` | Phase 1: Read-only verification script (amended; see notes) |
| 2     | —       | No commit — execution-only phase (live GitHub API apply) |
| 3     | `11ad375` | Phase 3: Document the convention (AGENTS.md) |

Notes on the commit structure (deviation from plan's one-commit-per-phase ideal):

- Phase 1 was originally committed as `89caa1a`, then **amended to `e06e685`**
  during Phase 2 to fold in two bug fixes discovered when the verify path was
  first actually exercised (see Automated Checks). The branch was
  force-pushed; the PR is unreviewed/unmerged so this is safe.
- Phase 2 (apply) intentionally made no file changes — it executed
  `scripts/branch-protection.sh apply` against the live `alanvardy/vardy`
  repo, which is the point of ticket VAR-860.
- Consequently the Completion Gate item "Commit contains both
  `scripts/branch-protection.sh` and `AGENTS.md` changes" is true across the
  branch but in two commits, not one — left unchecked for the user to sign
  off.

## Automated Checks

- [x] `./scripts/test.sh` passes — run in every phase (Phase 1: 112/112 in
      ~26s; Phase 2: 112/112; Phase 3: 112/112 in ~8s, warm)
- [x] `bash scripts/branch-protection.sh verify` exits **non-zero** with
      `branch protection NOT CONFIGURED on main (GET returned 404)` — Phase 1
      proved the verifier detects the pre-apply state
- [x] `bash scripts/branch-protection.sh verify` exits **0** with
      "All checks passed." after apply (Phase 2, re-verified by main agent
      post-hoc)
- [x] Live API `branches/main/protection`: `strict=true`, all 6 expected
      contexts, `required_linear_history.enabled=true`,
      `dismiss_stale_reviews=true`, `required_approving_review_count=0`,
      `enforce_admins.enabled=false`, `allow_force_pushes=false`,
      `allow_deletions=false`
- [x] Live API repo settings: `allow_merge_commit=false`,
      `allow_squash_merge=false`, `allow_rebase_merge=true`
- [x] `AGENTS.md` "Commits and PRs" section documents the rebase-only
      convention and points at `scripts/branch-protection.sh`

## Bug fixes folded into the Phase 1 commit (found/verified during Phase 2)

1. **`gh api --silent` + `--jq` incompatibility** — gh v2.100.0 rejects
   combining `--silent` with `--jq` on the same call. The 9 verify call sites
   used `gh api --silent <url> --jq <expr>`, which made the verify path print
   gh CLI help and exit 1 after a successful apply. Fix: dropped `--silent`
   from the 9 `--jq` verify calls (kept it on the 404 probe and the apply
   PUT/PATCH calls, where it is valid). Phase 1's automated check never
   exercised these calls because it only hit the 404 path.
2. **jq `sort` vs bash `sort` collation mismatch** — the script compares jq's
   byte-order `sort` against bash's locale-based sort (case-insensitive under
   `en_CA.UTF-8`), which makes two identical 6-item context sets never sort
   equal. Fix: `expected_ctxs=… | LC_ALL=C sort` so bash sorts in byte order
   like jq.

## Environment note (pre-existing, not introduced here)

On a genuinely clean checkout, `./scripts/test.sh` fails at the sqlx
"UPDATE MIGRATIONS" step until `test.db` exists and is migrated (SQLite
CANTOPEN; sqlx doesn't create/migrate it). Local fix used: `touch test.db &&
cargo sqlx migrate run`. `test.db` is gitignored, so nothing was committed.

## Manual Verification Items (from the plan)

- [ ] **Phase 1**: Read the script output and confirm it cleanly reports the
      404 (no `gh` auth errors, no bash syntax errors)
- [ ] **Phase 1**: Run `bash scripts/branch-protection.sh` (no args) —
      defaults to `verify`, same output
- [ ] **Phase 2**: `bash scripts/branch-protection.sh apply` — confirm
      interactive prompt, then confirm all verify steps pass
- [ ] **Phase 2**: Run `bash scripts/branch-protection.sh apply` a **second
      time** — must exit 0 with "All checks passed" (idempotent)
- [ ] **Phase 2**: Verify via `gh api .../branches/main/protection | jq` all
      listed fields (strict, 6 contexts, linear history, dismiss stale,
      review count 0, enforce_admins false)
- [ ] **Phase 2**: Verify via `gh api repos/alanvardy/vardy --jq '{...}'` the
      three merge-settings fields
- [ ] **Phase 2**: Sanity-check live merge flow — merge this PR with
      `gh pr merge <n> --rebase --delete-branch` — must succeed
- [ ] **Phase 3**: Re-read the "Commits and PRs" section in `AGENTS.md` —
      confirm the two new bullets are present and accurate
- [ ] **Phase 3**: Confirm no other doc file needs the same note (no
      `CONTRIBUTING.md`, no `docs/`; only `AGENTS.md`, `README.md`,
      `ROUTES.md`)
- [ ] **Completion Gate**: "Commit contains both script and AGENTS.md
      changes" — true across the two commits (`e06e685`, `11ad375`) rather
      than a single commit