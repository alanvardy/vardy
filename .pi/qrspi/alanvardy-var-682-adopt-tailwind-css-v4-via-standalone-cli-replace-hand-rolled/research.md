# Research Findings

## Q1: How does static asset serving and cache invalidation currently work?

### Findings
- Hash store: process-wide `static ASSET_HASHES: OnceLock<HashMap<String, String>>`, keyed by path
  relative to `static/` (`src/app/assets.rs:8`); lazily computed "on first use, i.e. during
  `templates::init()`" (`src/app/assets.rs:7`).
- `hash_all(dir)` (`src/app/assets.rs:12-15`) delegates to recursive `hash_dir`
  (`src/app/assets.rs:17-33`): walks the tree, computes `Sha256::digest` per file, stores a
  **12-hex-char prefix** (`src/app/assets.rs:28-30`). Unreadable dirs/files **panic** —
  "fail fast on broken deploys" (`src/app/assets.rs:18-20, 24-26`).
- `asset_url(file)` (`src/app/assets.rs:37-44`) returns `/static/{file}?v={hash}`; unknown files
  panic with `"unknown static asset {file}"` (`src/app/assets.rs:41-42`). ServeDir ignores `?v=`;
  invalidation relies purely on the URL changing when contents change.
- Registered as a minijinja global returning a safe string:
  `src/app/templates.rs:13-17`. Init call sites: `src/main.rs:26`, `src/test/mod.rs:32,61`.
- Serving: `.nest_service("/static", SetResponseHeader::overriding(ServeDir::new("static"),
  header::CACHE_CONTROL, HeaderValue::from_static("public, max-age=31536000, immutable")))`
  at `src/interfaces/routes.rs:30-36`. Every static response gets overriding one-year immutable
  Cache-Control.
- Template call sites of `asset_url`: `templates/layout.html:7` (site.css);
  `templates/singlethread.html:11,21,24,27,34,35` (screenshots). Hard-coded unversioned refs remain
  in `templates/home.html:4` (wave.svg), `:24` (github.svg), `:30` (linkedin.svg), `:37`
  (alanvardy.jpg) — immutable caching but no cache busting.

### Tests asserting on /static behavior
- `src/app/assets.rs:50-57` `known_file_yields_versioned_url` — `/static/singlethread-icon.png?v=` prefix + 12-char hex.
- `src/app/assets.rs:59-63` `hashes_are_deterministic`.
- `src/app/assets.rs:65-68` `unreadable_directory_panics`; `src/app/assets.rs:70-77` `unknown_file_panics`.
- `src/app/templates.rs:51-61` `asset_url_function_resolves_in_templates` — output starts with `/static/site.css?v=`.
- `src/interfaces/routes.rs:45-56` `static_icon_is_served` (200, image/png).
- `src/interfaces/routes.rs:96-109` `static_files_have_immutable_cache_control` (`max-age=31536000`).
- `src/interfaces/routes.rs:139-172` `singlethread_screenshots_are_served_with_immutable_caching` (5 assets × status/content-type/cache-control).
- `src/interfaces/routes.rs:174-186` `static_homepage_image_is_served` (alanvardy.jpg).
- `src/interfaces/routes.rs:188-211` `static_stylesheet_is_served` — GET `/static/site.css`: 200, `text/css`, `max-age=31536000`.
- Handler tests: `src/interfaces/handlers/singlethread/web.rs:46-50` asserts rendered HTML contains `?v=` URLs; `src/interfaces/handlers/home/web.rs:45-50` asserts home's *unversioned* URLs verbatim while site.css gets `?v=` — codifying the split.
- No test pins an exact hash value or verifies that editing site.css changes the fingerprint.

## Q2: What exactly does static/site.css contain today?

### Findings
198 lines total. Dark-only theme; no light mode, no `prefers-color-scheme`.

- `:root` palette (`static/site.css:1-7`): `--bg #121212`, `--surface #1e1e1e`, `--text #e0e0e0`,
  `--muted #9e9e9e`, `--accent #7aa2f7`.
- Global/element selectors: `*` box-sizing (9-11), `body` (13-19), `.container` max-width 48rem
  (21-25, used at layout.html:12), `.card` (**defined but unused**, 27-32), `nav`/`nav a`/
  `nav a:hover` (34-49, matching layout.html:11-14).
- Home-only classes: `.home .wave` (52-55 → home.html:4), `.home-columns` (57-61 → :8),
  `.home-text` (63-65 → :9), `.home-portrait` (67-69 → :26), `.portrait` (71-76 → :27),
  `.section-heading` (78-82 → home.html:17 **and** singlethread.html:17,38,46,56,68 — shared),
  `.invite-list` + link states (84-102 → home.html:18), `.invite-icon` (104-106 → home.html:20,25).
- SingleThread-only classes (`st-*`): `.st-hero` (109-113 → st.html:6), `.st-hero-text` (115-117 → :7),
  `.st-tagline` (119-122 → :8), `.st-hero-shot`+img (124-133 → :12), `.st-shots` (135-139 → :22),
  `.st-shot`+img (141-151 → :23,26,29), `.st-watch-pair`+img (153-164 → st.html:46),
  `.st-list li::marker`/`li strong` (166-172 → :56,66,78), `.st-closing` (174-179 → :90).
  Note: `.st-watch` is used in singlethread.html:45 but has **no CSS rule**.
- Hard-coded colors outside `:root`: literal `#333` appears 6× as border color only —
  site.css:29 (.card), :39 (nav), :75 (.portrait), :132 (.st-hero-shot img), :150 (.st-shot img),
  :163 (.st-watch-pair img). All other colors go through `var(--…)`.
- Media queries: exactly one — `@media (max-width: 48rem)` (site.css:181-198): stacks
  `.home-columns` (184-186), reorders portrait above text (188-190), stacks `.st-hero` (192-194),
  full-width hero shot (196-198). Breakpoint matches `.container` max-width (site.css:22).

## Q3: How are templates loaded and rendered?

### Findings
- `templates::init()` (`src/app/templates.rs:3-20`): `minijinja::path_loader("templates")`
  loader (line 6); auto-escape callback sets `AutoEscape::Html` for `.html` names,
  `None` otherwise (7-13); registers `asset_url` wrapping `assets::asset_url` via
  `Value::from_safe_string` (14-18). Test `html_names_are_escaped_and_others_are_not`
  (templates.rs:36-48).
- Handlers render with empty context: `state.templates.get_template("home.html")?.render(context! {})`
  (`src/interfaces/handlers/home/web.rs:12`); same for singlethread after a page-view metric
  (`src/interfaces/handlers/singlethread/web.rs:11-13`). Render errors flow through `WebError`,
  logged as "template render error" (`src/app/error.rs:45`).
- `templates/layout.html`: stylesheet link `{{ asset_url('site.css') }}` (layout.html:7); nav links
  `/` and `/singlethread` (11-13); `<h1>` heading block inside `.container` (15-17); content block
  (19). No favicon link anywhere.
- `home.html` classes: root `div.home` → `.home-columns` → `.home-text` / `.home-portrait`
  (:10-13, 27-29); wave `<img class="wave">` sits inside the `<h1>` block (:4);
  `h2.section-heading` (:22); `ul.invite-list` (:23); `img.invite-icon` for GitHub/LinkedIn (:26,31);
  `img.portrait src="/static/alanvardy.jpg"` (:28).
- `singlethread.html` classes: `.st-hero`/`.st-hero-text`/`.st-tagline`/`.st-hero-shot` (:6-13);
  `.st-shots` grid of three `figure.st-shot` (:24-34); `.st-watch` wrapper (no CSS rule) →
  `.st-watch-pair` with two images (:36-42); five `h2.section-heading` reuses (:17,38,46,56,68);
  three `ul.st-list` (:44,58,70); `p.st-closing` (:82).
- Image-reference inconsistency: singlethread.html uses `{{ asset_url(...) }}` for all images;
  home.html hard-codes 4 unversioned `/static/` paths (wave.svg :4, github.svg :26, linkedin.svg :31,
  alanvardy.jpg :28) — bypassing sha256 cache busting entirely.
- Unreferenced assets in `static/`: `quill.png`, `singlethread-icon.png` appear in no template
  (the icon is referenced only by tests: assets.rs:51, routes.rs:57,109).

## Q4: How does the build and deploy pipeline work end to end?

### Findings
- Dockerfile (4 stages, `Dockerfile:1-32`):
  1. `chef` — `lukemathwalker/cargo-chef:latest-rust-1-bookworm` (1-2).
  2. `planner` — `COPY . .`, `cargo chef prepare --recipe-path recipe.json` (4-6).
  3. `builder` — installs sqlx-cli sqlite (9), copies recipe.json, `cargo chef cook --release` (10-12),
     `COPY . .` again bringing `static/`, `templates/`, `migrations/`, `.sqlx/` (14),
     `ENV SQLX_OFFLINE=true` (15), `cargo build --release --bin vardy` (16). No asset/CSS step.
  4. `runtime` — `debian:bookworm-slim` + `libssl3 ca-certificates` (19-24); copies from builder:
     sqlx binary (26), `migrations/` (27), `templates/` (28), `static/` (29), release binary (30);
     `DATABASE_URL=sqlite:data/vardy.db`, entrypoint `/usr/local/bin/vardy` (31-32). The copied sqlx
     CLI is unused at runtime — migrations run in-process via `sqlx::migrate!("./migrations")`
     (`src/app/db.rs:32-33`, called from `src/main.rs:23`).
- `.dockerignore` excludes `.git/`, `.github/`, `target`, `.pi/`, `.env*`, `scripts/` — scripts never
  enter any image stage.
- `scripts/test.sh` (1-19): sources `.env` (fails if missing), then `&&`-chained gates:
  `cargo fmt --all` → `cargo sqlx prepare -- --tests` → `cargo check --all-targets` →
  `cargo clippy --all-targets --all-features --locked -- -D warnings` → `cargo nextest run` →
  inverted-rg TODO gate (no `FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO\s|todo\s` in src).
- `scripts/lint_string.sh` (1-9): greps `*.rs` for one arg, exits 1 if found.
- CI (`.github/workflows/ci.yml`): jobs test (nextest; llvm-cov+Codecov on main pushes), todos
  (lint_string.sh × 5 patterns), fmt, clippy. Does **not** run `scripts/test.sh` or
  `cargo sqlx prepare` — mirrors most gates except sqlx-prepare/check.
- CD (`fly-deploy.yml`): `workflow_run` after successful CI on main → `flyctl deploy --remote-only`;
  Fly builds the Dockerfile remotely. `fly.toml`: app `vardy`, region ord, port 3000,
  health check `GET /health` (fly.toml:19-23), metrics port 9090.
- Other workflows: weekly CodeQL (`ci-secure.yml`), daily rust-toolchain bump PRs
  (`rust-version-bump.yml`, pinned 1.97.1 in `rust-toolchain.toml:3`), Dependabot auto-merge +
  config, root `codecov.yml`.
- Asset/CSS build tooling: **none exists**. Repo-wide search for tailwind/postcss/package.json/
  sass/scss/npm/npx/node_modules finds no hits in buildable code — only mentions in `.pi/qrspi/`
  docs (VAR-664 design.md explicitly notes "No Tailwind, npm, esbuild, or any asset build step").
  CSS is served verbatim; fingerprinting happens at render time, not build time.

## Q5: What conventions and prior decisions constrain changes to static assets/styling?

### Findings
- Root `AGENTS.md`: routes only in `src/interfaces/routes.rs` (:14-17); route/param changes require
  ROUTES.md updates using `---` block cut points (:44-46); QRSPI pipeline mandatory (:49-53); errors
  through `WebError::IntoResponse` (:56-58); inline `#[cfg(test)]` unit tests + `start_app()`
  integration tests (:22-27). No AGENTS.md rule mentions CSS/assets directly.
- `ROUTES.md`: "### GET /static/{file}" section documents ServeDir serving from `static/`, 200 with
  inferred content type, 404 for missing files.
- Prior design-doc decisions (`.pi/qrspi/*/design.md`, 15 docs exist; var-682 has none yet):
  - **VAR-657** (singlethread page): established ServeDir + `nest_service("/static")` pattern
    (design.md:65-67); CSS convention of its era was inline `<style>` blocks, "no external
    stylesheets" (:55-56), "No external stylesheets, JS, fonts, or build tooling" (:96-98),
    "No caching headers / asset fingerprinting" (:98) — since superseded.
  - **VAR-664** (homepage redesign): "Patterns NOT to Follow" rejects "Tailwind + esbuild build
    pipeline … adds a build step to a zero-build repo"; "What We're NOT Doing": "No Tailwind, npm,
    esbuild, or any asset build step", no JS, no theme toggle; Decision 4 extracted CSS into
    `static/site.css` linked from layout.html keeping the dark variable palette; flagged CSS cache
    invalidation as open risk (later solved by VAR-670).
  - **VAR-670** (CSS cache invalidation): Decision 1 = minijinja `asset_url` global → sha256 12-hex
    query param hashed once at startup; Decision 2 = explicit `Cache-Control:
    public, max-age=31536000, immutable` confined to the static mount; Decision 5 = missing/unhashable
    assets panic at startup (fail-fast, fly health check gates release). "Patterns NOT to Follow":
    manual version bumps, **build-time fingerprinting / CI asset pipeline ("gross overkill")**,
    per-request hashing, graceful degradation.
  - **VAR-668** (improve singlethread page): documents live state incl. panic-on-unknown-filename
    (design.md:11-17); versioned `asset_url()` references are the pattern to follow — hardcoded
    `/static/...` URLs are a pattern NOT to follow (:48-55); design tokens reused with "no new color
    literals"; mirror existing media-query breakpoint pattern; image treatment = radius + 1px border.
  - **VAR-679** (deploy breakage): fly.io remote builds; Docker copies `static/` verbatim;
    repo-relative paths assume CWD `/app`.

## Cross-Cutting Observations

- **Zero-build repo**: VAR-657, VAR-664, and VAR-670 each recorded decisions against build steps,
  npm tooling, and CI asset pipelines. No JS tooling exists anywhere; the Dockerfile has no asset
  stage. Any new build step touches Dockerfile, `scripts/test.sh`, ci.yml, and `.dockerignore`
  simultaneously.
- **Caching contract**: `/static/*` serves with year-long immutable Cache-Control
  (`routes.rs:30-36`); correctness depends entirely on hashed URLs via `asset_url`. Home page's four
  hard-coded image URLs are a known inconsistency that handler tests currently assert verbatim
  (`home/web.rs:45-50`).
- **Fail-fast asset discipline**: `asset_url` panics on unknown filenames and `hash_dir` panics on
  unreadable files — new static files must exist under `static/` before first render.
- **Token system**: dark-only `:root` custom properties are the color source of truth; prior designs
  mandate "no new color literals". One breakpoint (48rem) matches `.container`.
- **Test surface**: every asset-related behavior (versioned URL shape, determinism, panics, headers,
  content types, per-page rendering) has dedicated tests across assets.rs, templates.rs, routes.rs,
  and both handlers' web.rs tests.
- **Gate asymmetry**: `scripts/test.sh` runs `cargo sqlx prepare` + `cargo check`; ci.yml omits
  those two but adds coverage/Codecov.

## Open Areas

- No test pins the exact `?v=` fingerprint value or asserts that changing site.css changes the URL —
  only prefix-shape assertions exist.
- `.card` class is defined but unused; `.st-watch` class is used but unstyled — current intent unclear.
- `static/quill.png` ships but is referenced by no template.
- Whether the standalone Tailwind CLI approach would run locally only, in CI, in Docker, or all
  three cannot be determined from the repo (no precedent exists; prior docs rejected build steps).
