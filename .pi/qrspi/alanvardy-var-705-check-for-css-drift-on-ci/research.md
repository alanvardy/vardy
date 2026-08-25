# Research Findings

Scope: source→compiled CSS relationship, the CSS build script, any existing
source-vs-output "drift" check, the production (Docker) CSS rebuild, and the
CI workflows' environments/triggers. All paths are relative to the repo root.

## Q1: Relationship between `css/site.css` (source) and `static/site.css` (artifact)

### Findings
- `css/site.css` is the Tailwind v4 CSS-first **source**; `static/site.css` is the
  compiled, minified **artifact**. The source header states it is "Compiled to
  static/site.css by scripts/build-css.sh — never served directly" (`css/site.css:2`).
- **Both are committed to git.** `git ls-files` lists `css/site.css` and
  `static/site.css`. Prior commits show deliberate artifact rebuilds, e.g.
  "Rebuild static/site.css from css/site.css source" (commits `335f17c`, `9045191`,
  `a916633`, `621eeda`, `62b1b1f`). The build script banner also says "Compile
  css/site.css into the committed static/site.css" (`scripts/build-css.sh:2`).
- **Only `static/site.css` is served.** `src/interfaces/routes.rs:45-47` mounts
  `ServeDir::new("static")` at `"/static"` with an immutable cache header
  `public, max-age=31536000, immutable` (`routes.rs:49`). `css/site.css` is never
  routed.
- The compiled artifact is the minified one-line file `static/site.css` carrying a
  `/*! tailwindcss v4.3.3 */` banner (verified on disk).
- **Tailwind v4 transformation model:** `css/site.css:10-11` imports only Tailwind's
  `theme.css` and `utilities.css` under explicit layers; preflight is intentionally
  NOT imported (`css/site.css:3-5`). A custom `@theme` block (`css/site.css:13-22`)
  defines the palette (`--color-bg`, `--color-accent`, …) and `--radius-lg`; an
  `@layer base` block (`css/site.css:24-91`) holds global element rules that appear
  minified in the output.
- **Utility generation is Template-driven:** v4 scans class usage in templates; the
  `@layer utilities` in the output contains only classes actually present in
  `templates/*.html` (e.g. `.flex`, `.mt-8`, `md:flex-row`). Adding a class to a
  template changes the generated artifact.
- **Determinism guards:** `css/site.css:8-9` has `@source not "../.pi"` and
  `@source not "../static"` so source scanning ignores local planning notes and the
  prior build output (the comment at `css/site.css:5-7` explains they would otherwise
  make the utility set nondeterministic).
- **Serving + cache-busting:** `templates/layout.html:7` links the stylesheet via
  `asset_url('site.css')`. `asset_url` is registered as a MiniJinja function in
  `src/app/templates.rs:11-19` and produces `/static/<file>?v=<12-hex sha256>`
  (`src/app/assets.rs:31,37-47`). Hashes are lazily computed once into a
  `OnceLock<HashMap>` (`assets.rs:8,38`) by recursively hashing every file under
  `static/` (`assets.rs:12-31`). Because the URL embeds a content hash and the route
  sets `immutable` caching, a rebuilt `static/site.css` yields a new `?v=` and busts
  caches. Tests assert this (`src/app/templates.rs:51-61`, `src/app/assets.rs:49-76`).
- **Connection to serving contract:** the runtime test asserts `/static/site.css` is
  served (`src/interfaces/routes.rs:253-272`), confirming the artifact is the served
  object.

## Q2: Trace `scripts/build-css.sh`

### Findings
- **Pinned toolchain:** Tailwind standalone CLI `v4.3.3`, a Bun-based standalone
  binary (no Node/npm) (`scripts/build-css.sh:6`).
- **Pinned checksums:** macOS arm64 SHA-256 at `scripts/build-css.sh:7`
  (`cdf64670…d5ce9d`); Linux x86_64 at `:8` (`dc61b3ac…abc313a`). Both were verified
  against actual upstream `v4.3.3` release binaries.
- **Platform selection** (`scripts/build-css.sh:10-17`): `case "$(uname -s)/$(uname -m)"`
  supports only `Darwin/arm64` (`tailwindcss-macos-arm64`) and `Linux/x86_64`
  (`tailwindcss-linux-x64`); any other platform (`Darwin/x86_64`, `Linux/aarch64`,
  Windows) prints "Unsupported platform" and exits 1.
- **Binary cache & toolchain verification** (`scripts/build-css.sh:19-31`): cached at
  `target/tailwindcss-cli/tailwindcss` (under gitignored `/target`, `.gitignore:1`).
  It is re-downloaded only when the cached file is missing/not executable OR fails
  `shasum -a 256 -c -` (`:24-27`), fetched via `curl -fsSL` from the GitHub release
  URL (`:28-30`). The checksum is verified unconditionally again before running
  (`:32-33`) — a mismatch fails the script via `set -euo pipefail` (`:4`).
- **Compile invocation** (`scripts/build-css.sh:34-35`):
  `"$bin" -i css/site.css -o static/site.css --minify` — source in, minified artifact
  out. Run from repo root.
- **Call sites:** (a) local gate `scripts/test.sh:12` invokes it then diffs output
  (see Q3); (b) NOT called by any CI workflow; (c) the Docker builder inlines the
  equivalent steps because `scripts/` is dockerignored (see Q4).

## Q3: Where the source-vs-output consistency ("drift") check exists

### Findings
- **The drift check exists in exactly ONE non-CI place:** `scripts/test.sh:12-13`
  runs `./scripts/build-css.sh` then `git diff --exit-code -- static/site.css`. If the
  regenerated artifact differs from the committed `static/site.css`, `git diff
  --exit-code` fails and aborts the `&&` chain (before clippy/tests), enforcing the
  committed artifact matches build output.
- **`scripts/test.sh` is a local/repo gate** (documented at `AGENTS.md:38`: "Run
  `./scripts/test.sh`…"). It also sources `.env` (`test.sh:3`), runs `cargo fmt`,
  `cargo sqlx prepare`, `cargo check`, then build-css+diff (`test.sh:6-13`), then
  clippy, nextest, and a `rg` TODO scan (`test.sh:15-20`).
- **The check does NOT run in CI.** `.github/workflows/ci.yml` never invokes
  `build-css.sh` nor any diff; its `test` job runs `cargo nextest run --profile ci`
  (PR) / `cargo llvm-cov nextest` (main) directly (`ci.yml:62,67`), not
  `scripts/test.sh`. Its `todos`/`fmt`/`clippy` jobs (`ci.yml:89-127`) cover Rust
  lint/style only.
- **The Dockerfile rebuilds CSS but performs NO comparison** (see Q4). It just
  regenerates `static/site.css` to ship it.
- **Context matrix:** local `test.sh` = rebuild ✅ + compare ✅; CI workflows = rebuild
  ❌ + compare ❌; Dockerfile build = rebuild ✅ + compare ❌.

## Q4: How CSS is rebuilt in production (Dockerfile)

### Findings
- The **builder stage** (`FROM chef AS builder`, `Dockerfile:8`) regenerates
  `static/site.css` from `css/site.css` after `COPY . .` (`Dockerfile:21`).
- **Versioned/checksum-pinned:** `ARG TAILWIND_VERSION=v4.3.3` (`Dockerfile:9`);
  per-arch checksums `TAILWIND_SHA256_AMD64` (`:11`, same value as the script's
  Linux x64) and `TAILWIND_SHA256_ARM64` (`:12`, `55fd0b…8395` — an arm64 asset does
  NOT exist in `build-css.sh`).
- **Arch selection in the RUN block** (`Dockerfile:27-37`): `case "$TARGETARCH"` maps
  `amd64`→`tailwindcss-linux-x64` and `arm64`→`tailwindcss-linux-arm64`, else fails
  the build (`Dockerfile:27-31`). The comment at `Dockerfile:24-26` explains arm64 can
  not run the x64 (Bun) binary under Rosetta. Download via `curl -fsSL`
  (`Dockerfile:33-34`), verify with `sha256sum -c -` (fails build on mismatch,
  `Dockerfile:35`), `chmod +x` (`:36`), then
  `tailwindcss -i css/site.css -o static/site.css --minify` (`Dockerfile:37`) —
  overwriting the committed artifact.
- **Why inlined:** `.dockerignore:11` excludes `scripts/` from `COPY . .`, so
  `build-css.sh` is not available in the image; the comment at `Dockerfile:23` states
  it explicitly. The inlined command is functionally identical to
  `scripts/build-css.sh:34`.
- **Shipping:** the runtime stage copies the regenerated static tree via
  `COPY --from=builder /app/static ./static` (`Dockerfile:52`, `WORKDIR /app:48`).
  The committed `static/site.css` in the repo is overwritten by the build, so the
  shipped artifact is the freshly-compiled one (identical bytes when the committed
  file is up to date; otherwise the image uses the freshly built output).
- **Serving relationship:** the runtime serves the shipped `/app/static` tree
  (`routes.rs:45-47`), fingerprinted via `asset_url` (`src/app/assets.rs:37-47`).

## Q5: CI runner environments & tooling

- **All workflows default to `ubuntu-latest`**, except CodeQL's `analyze` matrix which
  picks `macos-latest` only for a hypothetical `swift` language entry
  (`ci-secure.yml:19`).
- **`ci.yml`** (`runs-on: ubuntu-latest` at each job `ci.yml:40,90,101,111`):
  - `test` job: `actions/checkout@v2`…`actions/checkout@v7` (`ci.yml:42`); mold v2.37.1
    installed via `wget`+`tar xz` (`ci.yml:45-47`); `dtolnay/rust-toolchain@stable`
    with `components: llvm-tools-preview` (`:49-51`); `Swatinem/rust-cache@v2`
    (`:53`); `taiki-e/install-action@v2` with `tool: cargo-llvm-cov,nextest`
    (`:55-57`).
  - `todos` job: `./scripts/lint_string.sh` on `FIXME `, `FIXME:`, `fixme `, `fixme:`,
    `dbg!` (`ci.yml:93-97`).
  - `fmt` job: `cargo fmt --all -- --check` with `rustfmt` component (`ci.yml:104-107`).
  - `clippy` job: mold again, `clippy` component, rust-cache, then
    `cargo clippy --all-targets --all-features --locked -- -D warnings` (`ci.yml:113-126`).
- **`Swatinem/rust-cache@v2` keying:** used only in `ci.yml` (`test` at `:53`, `clippy`
  at `:124`) with **no custom `key`/`shared-key`**. It uses the action's built-in
  defaults (based on `Cargo.toml`/`Cargo.lock`, the toolchain in
  `rust-toolchain.toml`, and OS/target-triple) — it is NOT keyed on any CSS or
  `static/site.css` contents.
- **`ci-secure.yml`:** `analyze` uses `github/codeql-action/init@v4.37.7` +
  `analyze@v4.37.7` with config `codeql/codeql.yml` (matrix entries: `actions` and `rust`, both
  build-mode none) (`ci-secure.yml:43,50,58`); `clippy-analyze` installs tooling via
  `cargo install clippy-sarif sarif-fmt` (`ci-secure.yml:70`), streams clippy SARIF with
  `continue-on-error: true`, uploads via `github/codeql-action/upload-sarif@v4.37.7`
  (`ci-secure.yml:74-84`). `actions/checkout@v7` everywhere.
- **`dependabot_auto_merge.yml`:** single step, `fastify/github-action-merge-dependabot@v3`
  (`target: minor`, rebase, auto-merge). No checkout/Rust tooling.
- **`fly-deploy.yml`:** `actions/checkout@v4` (the only @v4 across workflows) +
  `superfly/flyctl-actions/setup-flyctl@master`, then `flyctl deploy --remote-only` with
  `FLY_API_TOKEN` secret (`fly-deploy.yml:15-17`).
- **`rust-version-bump.yml`:** raw bash resolves latest `rustc` via `rustup`, parses
  `rust-toolchain.toml`, and `peter-evans/create-pull-request@v8` with
  `token: secrets.MYTOKEN` (`rust-version-bump.yml:27-60`).
- **External-tool install strategies observed:** `actions/checkout`, `dtolnay/rust-toolchain`,
  `Swatinem/rust-cache`, `taiki-e/install-action`, raw `wget`+`tar` (mold)/`curl`+pipe
  (build-css, `scripts/build-css.sh:28-30`), `cargo install`, flyctl setup action,
  CodeQL actions, GitHub-marketplace 3p actions.
- **Local Tailwind fetch** (non-CI) downloads the standalone CLI to `target/` only as
  needed (`scripts/build-css.sh:19-33`); CI never does this — CI builds no CSS.

## Q6: Workflow triggers & job organization

- **`ci.yml` triggers** (`ci.yml:12-17`): `push` filtered to `branches: [main]`; every
  `pull_request`; and `workflow_dispatch`. The header comment (`ci.yml:7-11`)
  explains PR branch pushes are covered by `pull_request` and only `main` is covered by
  the push trigger to avoid duplicate runs.
- **Concurrency** (`ci.yml:33-35`): `group = <workflow-name>-<ref>`, `cancel-in-progress:
  true`, so superseded runs on the same ref are cancelled.
- **Job organization in `ci.yml`:** `test` (`:39-87`), `todos` (`:89-98`), `fmt`
  (`:100-108`), `clippy` (`:110-127`) are sibling jobs with **no `needs:`**, so they run
  concurrently and independently.
- **PR vs main split inside `test`:** PR path (`if: github.event_name == 'pull_request'`)
  runs fast `cargo nextest run --profile ci` (`ci.yml:60-62`); main path
  (`if: push && ref == refs/heads/main`) runs full `cargo llvm-cov nextest` coverage
  (`ci.yml:65-67`). Coverage upload to codecov is likewise gated to pushes to main
  (`ci.yml:69-71`); nextest JUnit upload is gated on `!cancelled()` and non-fork PRs
  (`ci.yml:77-86`).
- **`fly-deploy.yml` triggers** on `workflow_run` of the **`CI`** workflow completing,
  restricted to `branches: [main]` (`fly-deploy.yml:3-7`); the deploy job proceeds only
  `if: github.event.workflow_run.conclusion == 'success'` and uses concurrency group
  `deploy-group` (`fly-deploy.yml:12-13`).
- **`ci-secure.yml`** is triggered only by `schedule` cron `26 17 * * 4` (weekly
  Thursday) (`ci-secure.yml:3-4`) plus concurrency `cancel-in-progress`.
- **`rust-version-bump.yml`** is triggered by daily `schedule` cron `0 6 * * *` plus
  `workflow_dispatch` (`rust-version-bump.yml:9-12`); opens a PR (not a direct main commit)
  so full CI validates a new compiler.
- **`dependabot_auto_merge.yml`** triggers on `pull_request` and gates on
  `github.event_name == 'pull_request'` (`dependabot_auto_merge.yml:2,7`).
- **How styling/format/fmt jobs interact:** `fmt` and `todos` are standalone style/lint
  jobs; nothing (`needs:`) makes `test` depend on them, and they do not force any CSS
  build or drift check.
- **Net CI behavior relation to CSS:** no workflow component performs the CSS drift
  check described in Q. Deploys to `main` (`fly-deploy`) trigger off CI success, and the
  Docker build (which itself recompiles CSS) is what produces production CSS.

## Cross-Cutting Observations

- **Single source of truth for Tailwind version:** `scripts/build-css.sh:6` and
  `Dockerfile:9` both pin `v4.3.3`; the AMD64 checksum is duplicated verbatim
  (`scripts/build-css.sh:8` == `Dockerfile:11`). The macOS/arm64 checksum is local-only;
  the Docker arm64 checksum/asset (`Dockerfile:12`) is not in `build-css.sh`.
- **Committed artifact + rebuild-and-diff drill:** the repo intentionally commits the
  generated `static/site.css` and relies on a rebuild-then-`git diff --exit-code` guard
  in `scripts/test.sh` to keep it consistent. That consistency guarantee is enforced
  ONLY locally; production rebuilds from source unconditionally (no comparison), and CI
  neither rebuilds nor checks.
- **Cache-busting is byte-driven from the served artifact:** `asset_url`'s `?v=` hash
  (`assets.rs:31`) is computed from the bytes of the committed `static/site.css`, so a
  drift (or any artifact change) changes the served URL automatically.
- **Centralized error/percolation:** per AGENTS.md, all handler errors route through
  `WebError::IntoResponse`; the CSS pipeline is entirely outside Rust (scripts + Docker)
  and communicates only via exit codes.

## Open Areas

- Whether the drift check SHOULD also run in CI is a design decision — the research
  only establishes that today it runs locally and is absent from CI. No answer is
  implied here.
- The Docker `arm64` path pins a checksum/asset that the local build script cannot
  produce (no `Linux/aarch64` support in `build-css.sh`); whether the produced bytes
  would match a commit-on-arm64 is unverified.
- Cross-matrix platform coverage (`build-css.sh` Darwin/x86_64, Linux/aarch64, Windows)
  is currently hard-fail; a future drift check on a non-macOS/Linux-x64 runner would
  need to account for this.