# Structure Outline

## Approach

Add branch protection to `main` for `alanvardy/vardy` via an idempotent, self-verifying bash script (`scripts/branch-protection.sh`) that uses `gh api`, then commit a one-line convention note in `AGENTS.md`. No Rust code changes; no CI workflow changes; no new secrets (`gh` token already has `repo` scope).

The work decomposes into three horizontal layers, bottom-up: **verify** (read-only, safe foundation) → **apply** (writes, reuses verify) → **docs** (points at the script and records the rebase-only convention).

---

## Layer 1: Read-only verification script

Delivers the script skeleton with a `verify` path only. It GETs the current protection config for `main` and the repo merge-method settings, compares each against the desired state, and exits non-zero on any mismatch. This is safe to run before any write happens, and it *is* the acceptance check the later apply layer reuses.

**Files**: `scripts/branch-protection.sh` (new)

**Key changes**:
- `set -euo pipefail` header (matches `scripts/build-css.sh:4`, `reset_db.sh:4`)
- Constants capturing desired state:
  - `REQUIRED_CONTEXTS=( "CI / Cargo CI Tests" "CI / TODO and FIXME" "CI / Rust-fmt (Cargo Format)" "CI / Clippy (Cargo Clippy Lint Check)" "CI / CSS Drift Check" "Dependabot Auto Merge / auto-merge" )`
  - `EXPECTED_REPO_SETTINGS` — `allow_merge_commit=false`, `allow_squash_merge=false`, `allow_rebase_merge=true`
- `gh_get(path: str): JSON` — helper wrapping `gh api --silent <path>`
- `verify_protection(): void` — asserts `required_status_checks.strict == true` and `contexts` sorted-equals `REQUIRED_CONTEXTS`; `required_linear_history == true`; `required_pull_request_reviews.dismiss_stale_reviews == true` and `required_approving_review_count == 0`; `enforce_admins == false`
- `verify_repo_settings(): void` — asserts the three `allow_*_merge` fields
- `main()` — dispatches on subcommand (`verify` default, `apply` stubbed to print "not implemented" in this layer)

**Tests**: no Rust unit tests (bash script). Manual gate: `bash scripts/branch-protection.sh` reports the *current* live state (protection GET returns 404 → script exits non-zero with a clear "protection not configured" message). Re-run after Layer 2 to confirm it flips to green.
**Verify**: `./scripts/test.sh` stays green (unchanged — this is a no-op guard); `bash scripts/branch-protection.sh` fails with the expected 404 diagnosis, confirming the verifier detects the un-configured state.

---

## Layer 2: Idempotent apply

Extends the script with write operations. Applies branch protection (`PUT`) and repo merge settings (`PATCH`), then re-runs Layer 1's verifiers to confirm the API now returns the expected config. Re-running the script produces the same end state (idempotent — no double-apply side effects).

**Files**: `scripts/branch-protection.sh` (modified)

**Key changes**:
- `apply_protection(): void` — `gh api --silent -X PUT repos/{owner}/{repo}/branches/{branch}/protection` with the JSON body from `design.md` (6 contexts, `required_linear_history`, review config, `enforce_admins: false`)
- `apply_repo_settings(): void` — `gh api --silent -X PATCH repos/{owner}/{repo}` with the three `allow_*_merge` fields
- `main()` — `apply` subcommand: apply → verify → print pass/fail summary; exit non-zero on any mismatch
- Interactive confirmation before writing (this is a human-run admin action; no `GITHUB_TOKEN`/CI integration)

**Tests**: manual gate: run `bash scripts/branch-protection.sh apply`; expect `gh api repos/alanvardy/vardy/branches/main/protection` to return the applied config. Re-run immediately — second run must make no changes and exit 0 (idempotency). Sanity-check live merge flow: `gh pr merge <n> --rebase --delete-branch` still works for a real PR.
**Verify**: `./scripts/test.sh` stays green; `bash scripts/branch-protection.sh` (verify) exits 0; a live `GET` of the protection endpoint matches the expected fields.

---

## Layer 3: Document the convention

Commits a doc note in `AGENTS.md` so the rebase-only/linear-history convention is recorded *in the repo* (today it lives only in `~/.pi/agent/AGENTS.md:86-88` and implicitly in `dependabot_auto_merge.yml:21`), and points maintainers at the script for future updates (e.g. when a CI `name:` drifts — the open risk from `design.md`).

**Files**: `AGENTS.md` (modified — "Commits and PRs" section, `AGENTS.md:46-49`)

**Key changes**:
- New bullet: rebase-only merges (`gh pr merge <n> --rebase --delete-branch`); merge-commit and squash merges are disabled on `main`
- New bullet: branch protection on `main` is managed by `scripts/branch-protection.sh` — run it after any CI job `name:` change (the script is the canonical list of required-check contexts)

**Tests**: no code tests. Manual gate: re-read the section; confirm it states the convention and points at the script, and that no other doc file needs the same note (research Q3 confirmed no CONTRIBUTING.md/docs/).
**Verify**: `./scripts/test.sh` stays green (no Rust/Tailwind changes).

---

## Testing Checkpoints

- **After Layer 1**: `./scripts/test.sh` green; `bash scripts/branch-protection.sh` correctly reports protection absent (non-zero).
- **After Layer 2**: `./scripts/test.sh` green; `bash scripts/branch-protection.sh` verify exits 0; live GET matches; second `apply` run is a no-op.
- **After Layer 3**: `./scripts/test.sh` green; doc section reviewed.

## Notes on non-horizontal aspects

- The "test gate" for this task is **manual/`gh api`**, not `cargo nextest` — the script is bash and has no unit-test harness. Each layer's verification is explicitly a live-API assertion (read-only in Layer 1, write-then-verify in Layer 2), which is why the layers are ordered verify-first.
- Cross-cutting risk (CI `name:` drift, noted in `design.md` Open Risks) is mitigated structurally: the required-context list lives in exactly one place (the script constants) and Layer 3 documents that the script is the canonical source to update.
