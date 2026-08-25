# Design Discussion

## Current State

The repo commits both the Tailwind v4 source (`css/site.css`) and the compiled,
minified artifact (`static/site.css`). The artifact is what gets served
(`src/interfaces/routes.rs:45-49`) with immutable cache headers. A content-hash
query parameter (`?v=<sha256>`) busts caches on rebuild
(`src/app/assets.rs:31-47`).

A **drift check** exists in exactly one place: `scripts/test.sh:12-13` rebuilds
CSS via `scripts/build-css.sh` then runs `git diff --exit-code --
static/site.css`. This catches stale committed CSS locally — the `&&` chain
stops before clippy/tests if the artifact is out of date.

**This check does NOT run in CI.** No GitHub workflow invokes `build-css.sh` or
diffs `static/site.css`. The `test`, `fmt`, `clippy`, and `todos` jobs in
`ci.yml` cover Rust lint/test/style only. The Dockerfile (`Dockerfile:27-37`)
rebuilds CSS unconditionally during the build but performs no comparison — the
committed artifact is simply overwritten. So a PR can merge with a stale
`static/site.css` and nobody catches it until someone runs `./scripts/test.sh`
locally.

## Desired End State

A new standalone `css-drift` job in `.github/workflows/ci.yml` that:

1. Checks out the repo
2. Runs `./scripts/build-css.sh` (which downloads the pinned Tailwind v4.3.3
   CLI, verifies its checksum, and compiles `css/site.css` → `static/site.css`)
3. Runs `git diff --exit-code -- static/site.css`
4. Fails the job if the diff is non-empty (meaning the committed artifact
   doesn't match a fresh build)

**Verification:** opening a PR that changes `css/site.css` without rebuilding
`static/site.css` must fail the `css-drift` job. Opening a PR that changes
neither CSS file (or rebuilds the artifact correctly) must pass.

## Patterns to Follow

### ✅ Good patterns to match

- **Job isolation (`ci.yml:89-127`):** `todos`, `fmt`, and `clippy` are
  standalone sibling jobs — no `needs:`, run in parallel. The new `css-drift`
  job follows this pattern: self-contained steps, no dependency on other jobs.

- **Script reuse (`ci.yml:93-97`):** `todos` calls `./scripts/lint_string.sh`
  rather than inlining. The new job calls `./scripts/build-css.sh` rather than
  duplicating the download/build logic.

- **`git diff --exit-code` pattern (`scripts/test.sh:13`):** The existing drift
  check uses `git diff --exit-code -- static/site.css`. The CI step uses the
  same command for parity.

- **`ubuntu-latest` runner (`ci.yml:40,90,101,111`):** All CI jobs run on
  `ubuntu-latest` (Linux/x86_64), which is supported by `build-css.sh`'s
  platform detection (`scripts/build-css.sh:13-14`).

- **`actions/checkout@v7` (`ci.yml:42,91,102,112`):** Consistent across all
  jobs. No need for Rust toolchain (no Cargo steps), so no `dtolnay/rust-toolchain`,
  `Swatinem/rust-cache`, or mold linker.

- **Trigger scope (`ci.yml:12-17`):** `push` to `main` + `pull_request`. The
  new job inherits these implicitly (it's in the same workflow).

### ❌ Patterns NOT to follow

- **Do NOT inline the build steps** as the Dockerfile does
  (`Dockerfile:27-37`). That would duplicate the Tailwind version, both
  checksums, and the download logic across three locations (`build-css.sh`,
  `Dockerfile`, `ci.yml`). The script is the single source of truth for local
  CSS builds.

- **Do NOT add the check to the `test` job** (`ci.yml:39-87`). `test` already
  has complex conditional logic (PR vs main paths, coverage uploads, JUnit
  artifact handling). Adding CSS drift checking there mixes unrelated concerns
  and adds latency to the critical Rust test path.

- **Do NOT add to the `fmt` job** (`ci.yml:100-108`). `fmt` currently needs
  only `rustfmt` — no external downloads. Adding Tailwind inflates its scope.

- **Do NOT key an explicit `actions/cache` on the Tailwind binary.**
  `scripts/build-css.sh` already caches to `target/tailwindcss-cli/`
  (`scripts/build-css.sh:20`), and `target/` is gitignored. The download is
  fast (~50MB, a few seconds). Simpler to redownload than to manage cache
  invalidation for a pinned, infrequently-changed binary.

- **Do NOT modify `scripts/test.sh`.** It remains the local gate, unchanged.
  CI runs the check independently through the workflow job.

## Design Decisions

1. **Standalone `css-drift` job**: New job in `ci.yml`, sibling to `test` /
   `todos` / `fmt` / `clippy`, with no `needs:` — runs in parallel. Failing
   CSS drift shows up as a distinct job failure, making it immediately obvious
   what went wrong.

2. **Call `scripts/build-css.sh` directly**: The script is already CI-capable:
   it runs on `Linux/x86_64` (the `ubuntu-latest` runner matches
   `scripts/build-css.sh:13-14`), has no `.env` dependency, and handles its
   own toolchain pinning + checksum verification. No duplication.

3. **`git diff --exit-code` for the check**: Identical to the local gate
   (`scripts/test.sh:13`). If `build-css.sh` produces `static/site.css` that
   differs from the committed version, `git diff --exit-code` exits non-zero
   and the job fails.

4. **No caching beyond the script's built-in `target/` cache**: The Tailwind
   binary is cached at `target/tailwindcss-cli/tailwindcss` by the script.
   Since CI doesn't cache `target/` (it's not keyed by `rust-cache`), the
   binary is re-downloaded each run. This is fine — the download is bounded
   and fast, and the checksum verification (`scripts/build-css.sh:32-33`)
   guarantees integrity regardless.

5. **No Rust toolchain needed**: The job doesn't run `cargo`, so it doesn't
   need `dtolnay/rust-toolchain`, `Swatinem/rust-cache`, mold, or any Rust
   CI tooling. The only prerequisites are `git`, `bash`, `curl`, and
   `shasum` — all available on `ubuntu-latest`.

6. **`scripts/test.sh` left unchanged**: The local gate continues to run the
   same check. No refactoring, no coupling between the local script and CI.
   They're independent implementations of the same logic.

## What We're NOT Doing

- **NOT modifying the Dockerfile.** The Docker build unconditionally rebuilds
  CSS from source (`Dockerfile:27-37`) and ships the fresh artifact. No
  change needed there — the drift check ensures the committed artifact is
  correct before it reaches Docker.

- **NOT modifying `scripts/build-css.sh`.** It already works on
  `Linux/x86_64`, handles its own toolchain verification, and produces
  deterministic output.

- **NOT modifying `scripts/test.sh`.** Local gate stays as-is.

- **NOT adding a `css-drift` job to `ci-secure.yml`, `fly-deploy.yml`,
  `rust-version-bump.yml`, or `dependabot_auto_merge.yml`.** The check goes
  in `ci.yml` only, which covers PRs and main pushes — the events where CSS
  changes happen.

- **NOT comparing across platforms.** The check only runs on `ubuntu-latest`
  (Linux/x86_64). The Tailwind v4 CLI's output is deterministic across
  platforms when given the same source and `--minify` flag, so a Linux-produced
  artifact should byte-match the committed one regardless of where it was built.

- **NOT adding a `--check` flag or dry-run mode to `build-css.sh`.** The
  existing script compiles to `static/site.css`, then `git diff --exit-code`
  detects drift. No script changes needed.

## Open Risks

- **Unexpected non-determinism**: If Tailwind v4 produces different output on
  Linux/x86_64 than the committer's platform (macOS/arm64), the check would
  fail spuriously. The same source + same CLI version + `--minify` should be
  deterministic, but this hasn't been verified across `Darwin/arm64` and
  `Linux/x86_64`. If it surfaces, the fix is to commit a Linux-built artifact.

- **Network dependency**: The job downloads the Tailwind CLI from GitHub
  releases on every run. If GitHub releases are down, the job fails. This is
  a low-probability risk and matches the existing behavior of `build-css.sh`
  (which also fetches on first use or checksum mismatch).

- **Future Tailwind version bumps**: When the Tailwind version is bumped, both
  `build-css.sh` and `Dockerfile` must be updated (they're already coupled on
  version/checksums). The CI job picks up the new version automatically since
  it calls the script. No additional maintenance.