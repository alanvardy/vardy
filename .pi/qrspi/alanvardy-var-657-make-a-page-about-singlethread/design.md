# Design Discussion — VAR-657: SingleThread Page + Site Nav

## Current State

- Single-page axum app. One route: `.route("/", get(handlers::home::web::index))`
  (`src/interfaces/routes.rs:6-9`).
- Handlers follow a strict feature-directory pattern:
  `src/interfaces/handlers/<page>/{mod.rs,web.rs}`, re-exported from
  `src/interfaces/handlers/mod.rs:1`. Handler signature is
  `async fn(State(state): State<AppState>) -> Result<Html<String>, WebError>`
  rendering via `state.templates.get_template(...)?.render(context! {})?`
  (`src/interfaces/handlers/home/web.rs:7-13`).
- Templates loaded at runtime by `minijinja::path_loader("templates")` with HTML
  auto-escape for `.html` names (`src/app/templates.rs:1-12`). Adding a template
  needs no Rust registration.
- `templates/layout.html` is the shared chrome: dark theme CSS custom props on
  `:root` (`--bg #121212`, `--surface #1e1e1e`, `--text #e0e0e0`, `--muted`,
  `--accent #7aa2f7`; `layout.html:8-14`), all CSS inline in `<head><style>`
  (`layout.html:7-40`), body is `<div class="container"><h1>{% block heading %}
  </h1>{% block content %}</div>` (`layout.html:42-47`). Only utilities are
  `.container` and `.card`. `--muted`/`--accent` defined but unused so far.
- **No static-asset capability exists**: no ServeDir/tower-http dep
  (`Cargo.toml:5-13`), no asset directory, no images anywhere.
- No fallback route — unknown paths get axum's default 404;
  `WebError::NotFound` exists but is never constructed by production code
  (`src/app/error.rs:20-30`, comment at `error.rs:6-8`).
- Tests: real-socket harness `start_app()`/`test_client()`
  (`src/test/mod.rs:5-24`); colocated `#[cfg(test)] mod tests` asserting status,
  content-type, and body substrings (`home/web.rs:20-39`).

## Desired End State

1. A persistent nav bar at the top of every page with links **Home** and
   **SingleThread**, inherited via `layout.html`.
2. A page at `/singlethread` describing the SingleThread app (original copy)
   with the SingleThread icon displayed.
3. Icon served from a new `static/` directory backed by `tower-http`'s
   `ServeDir`.
4. Verification: existing home test still passes; new tests assert
   `/singlethread` returns 200 text/html with expected title/copy/icon tag,
   and that both pages' HTML contains nav links to the other page.

## Patterns to Follow

- **Feature handler directories** (`handlers/home/{mod.rs,web.rs}`): create
  `src/interfaces/handlers/singlethread/{mod.rs,web.rs}`, add
  `pub mod singlethread;` to `handlers/mod.rs:1`. Follows Q1 finding.
- **Route registration**: one `.route("/singlethread", get(handler))` line in
  `routes()` (`routes.rs:6-9`). Keep `routes()` the single source of routes.
- **Handler shape**: copy `home/web.rs:7-13` exactly — `Result<Html<String>,
  WebError>`, empty `context! {}` render.
- **Template inheritance**: new `templates/singlethread.html` extends
  `layout.html` and overrides `title`, `heading`, `content`
  (`templates/home.html:1-8` pattern). Markup inside `<div class="container">`.
- **CSS conventions**: reuse `:root` variables and `.card`; add any new rules
  to layout's inline `<style>` block — no external stylesheets.
- **Testing style**: colocated `#[cfg(test)] mod tests` using `start_app()` +
  `test_client()` with substring assertions on rendered HTML.
- **NOT to follow**: leaving `--muted`/`--accent` unused — the nav should
  actually consume these tokens. Also do not replicate the "variant kept only
  for tests" situation (`error.rs:6-8`) — don't add unused error paths.

## Design Decisions

1. **Asset delivery: `tower-http` ServeDir + `static/` dir** (user choice).
   Add `tower-http` with `fs` feature; nest `.nest_service("/static",
   ServeDir::new("static"))` in `routes()`. Establishes reusable asset
   infrastructure; one new dependency.
2. **Icon source**: `~/Downloads/AppIcon2.png` (1024×1024 RGBA PNG, 1.4 MB) —
   too heavy to serve raw. Downscale to ~256px (macOS `sips`) into
   `static/singlethread-icon.png` before commit; reference at
   `/static/singlethread-icon.png` in the template. Source file stays out of
   the repo.
3. **Route path: `/singlethread` lowercase** (user choice), deviating from the
   ticket's literal `/SingleThread`. URLs are case-sensitive in axum; lowercase
   is conventional. Note this on PR #8 against VAR-657. No redirect from the
   capitalized variant.
4. **Page copy: original marketing-style text** written for this task (one
   intro paragraph + short feature list), since no canonical description was
   supplied. Kept in the template directly — no dynamic context needed.
5. **Nav lives in `layout.html`**, immediately inside `<body>` before
   `<div class="container">` — outside the `heading`/`content` blocks so both
   pages inherit it (Q4 finding). Contents: links Home → `/` and SingleThread
   → `/singlethread` only. Styled with flexbox using `--surface` background,
   plain `<a>` elements colored `--text`, hover/current accent via `--accent`.
6. **Icon placement**: within the page's `content` block near the heading,
   sized explicitly (e.g., width 96px) so layout doesn't jump while loading;
   meaningful `alt="SingleThread icon"`.

## What We're NOT Doing

- No favicon work (separate concern; browser will 404 `/favicon.ico` quietly).
- No redirect or alias for `/SingleThread` capitalized spelling.
- No dynamic data in templates — copy is static; `context! {}` stays empty.
- No fallback/404 route changes; `WebError::NotFound` remains untouched.
- No external stylesheets, JS, fonts, or build tooling for assets.
- No other pages or nav entries beyond Home and SingleThread.
- No caching headers / asset fingerprinting for static files.

## Open Risks

- `tower-http` version must align with current `axum` version in
  `Cargo.toml`/`Cargo.lock`; check compatibility when adding.
- `ServeDir` reads from filesystem relative to process cwd — matches the
  existing runtime `path_loader("templates")` behavior, but verify Dockerfile
  (`Dockerfile`, `fly.toml`) copies/includes `static/` like it does templates.
- Icon rendering quality at 256px downscale unverified until eyeballed in a
  browser; if muddy, try 512px or export SVG later.
- Nav styling is new visual territory (first cross-page chrome); expect minor
  iteration during implementation review.
