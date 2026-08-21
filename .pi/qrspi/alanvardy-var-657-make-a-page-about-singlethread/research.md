# Research Findings

## Q1: How are HTTP routes registered and dispatched? Full request → HTML flow.

### Findings
- Routes are declared in one function returning `Router<AppState>`:
  `src/interfaces/routes.rs:6-9`. Currently a single route:
  `.route("/", get(handlers::home::web::index))`.
- Handlers live under `src/interfaces/handlers/<page>/web.rs`; module tree is
  re-exported via `src/interfaces/handlers/mod.rs:1` (`pub mod home;`) and
  `src/interfaces/handlers/home/mod.rs:1` (`pub mod web;`).
- Handler pattern (`src/interfaces/handlers/home/web.rs:7-13`): async fn taking
  `State(state): State<AppState>`, returns `Result<Html<String>, WebError>`;
  renders via `state.templates.get_template("home.html")?.render(context! {})?`.
- App state is `AppState { templates: minijinja::Environment<'static> }`
  (`src/app/state.rs:1-4`); built once in `main` (`src/main.rs:6-8`) and passed
  with `.with_state(state)` at `src/main.rs:14`.
- Server binds `0.0.0.0:3000`, uses
  `into_make_service_with_connect_info::<SocketAddr>()` (`src/main.rs:9-16`).
- A second path would follow the established shape: add a `.route("/x",
  get(handlers::<page>::web::<fn>))` line in `routes()`, create
  `src/interfaces/handlers/<page>/{mod.rs,web.rs}`, add `pub mod <page>;` to
  `handlers/mod.rs`, plus a template rendered through the shared `AppState`.

## Q2: How does the template system work end to end?

### Findings
- Environment built in `src/app/templates.rs:1-12`: `minijinja::path_loader("templates")`
  loads from filesystem dir `templates/` at runtime (not embedded);
  auto-escape callback sets `AutoEscape::Html` for names ending `.html`,
  `None` otherwise (`templates.rs:4-10`). Feature `debug` enabled in `Cargo.toml:9`.
- Layout inheritance: `templates/layout.html` is a full HTML document defining
  three blocks: `title` (line 6, default "Home"), `heading` (line 44),
  `content` (line 45).
- `templates/home.html:1-8` does `{% extends "layout.html" %}` and overrides
  all three blocks; body markup goes inside `<div class="container">`.
- All CSS lives inline in a `<style>` tag in layout's `<head>`
  (`layout.html:7-40`); there are no external stylesheets.
- A new page template needs: an `{% extends "layout.html" %}` line plus
  `title`, `heading`, and `content` block definitions (heading/content have
  empty defaults; title defaults to "Home").

## Q3: How are static assets handled?

### Findings
- There is no static-file serving of any kind: no `ServeDir`/`TowerHttp`
  dependency (`Cargo.toml:5-13` only axum, minijinja, tokio; dev reqwest), no
  asset directory, no route for `/static` or similar (`routes.rs:6-9`).
- Current pages reference no external imagery or stylesheet: `layout.html`
  uses system font stack and inline CSS only; `home.html` contains text only.
- No inline SVG, data URIs, favicons, or image tags exist anywhere in
  `templates/` or `src/`.
- Conclusion: nothing in the source tree answers "how images are served" —
  this capability does not exist today.

## Q4: Markup structure and CSS conventions of the shared layout.

### Findings
- Dark theme via CSS custom properties on `:root`
  (`templates/layout.html:8-14`): `--bg: #121212`, `--surface: #1e1e1e`,
  `--text: #e0e0e0`, `--muted: #9e9e9e`, `--accent: #7aa2f7`. Note `--muted`
  and `--accent` are defined but not yet used by any rule.
- Only two utility classes exist: `.container` (max-width 48rem, centered,
  3rem/1.5rem padding; `layout.html:28-32`) and `.card` (surface background,
  1px #333 border, 8px radius, 1.5rem padding; `layout.html:34-39`).
- Body structure (`layout.html:42-47`): `<body><div class="container">
  <h1>{% block heading %}</h1> {% block content %}</div></body>` — everything
  is inside the container div.
- A persistent top-of-page element would sit inside `<body>` immediately
  before `<div class="container">` (or as the first child inside it), i.e.
  outside both the `heading` and `content` blocks so all pages inherit it;
  nav links would be plain `<a href>` since routing is path-based (Q1).
- Responsive viewport meta present (`layout.html:5`).

## Q5: Testing helpers and error handling.

### Findings
- Test helpers in `src/test/mod.rs`: `start_app()` (lines 5-20) builds a fresh
  `AppState` with `templates::init()`, binds `127.0.0.1:0`, spawns
  `axum::serve` on the real router, returns the bound `SocketAddr`;
  `test_client()` (lines 22-24) returns a plain `reqwest::Client`. Included
  via `#[cfg(test)] mod test;` in `src/main.rs:21-22`.
- Example HTTP test (`src/interfaces/handlers/home/web.rs:20-39`): asserts
  status 200, `content-type` contains `text/html`, and body contains
  `<title>Home</title>` and specific copy strings from `home.html`.
- `WebError` has two variants (`src/app/error.rs:9-12`): `Template(minijinja::Error)`
  and `NotFound`. `From<minijinja::Error>` conversion enables `?` in handlers
  (`error.rs:14-18`).
- HTTP mapping (`error.rs:20-30`): `NotFound` → 404 with body "not found";
  `Template` → logs to stderr and returns 500 with body "internal server
  error".
- Unknown paths today: no fallback route exists in `routes.rs:6-9`, so axum's
  default 404 response is returned — the `WebError::NotFound` variant is never
  constructed by production code (kept alive only by unit tests, see comment
  `error.rs:6-8` and test `error.rs:36-40`). Template-not-found during a
  handler render surfaces as `WebError::Template` → 500.
- Unit tests also cover the auto-escape callback behavior
  (`src/app/templates.rs:22-41`) and error status mappings (`error.rs:32-48`).

## Cross-Cutting Observations
- Strict layered convention: `app/` (state, templates, errors) vs
  `interfaces/handlers/<feature>/web.rs`; each feature gets a directory pair
  of `mod.rs` + `web.rs` (`src/interfaces/handlers/home/` is the sole example).
- Rendering is uniform: state-carried `Environment`, `get_template(...)?.render(context! {})?`,
  `Result<Html<String>, WebError>` return type.
- Tests colocated in `#[cfg(test)] mod tests` inside the file they cover
  (`web.rs:15`, `error.rs:32`, `templates.rs:14`) plus shared HTTP harness in
  `src/test/mod.rs`. Tests run against a real bound socket (no tower mock).
- Templates are loaded from disk at runtime via `path_loader` — adding a new
  template requires no Rust registration beyond the handler/route.
- Coverage-hardening comments appear where variants/branches exist only for
  test completeness (`error.rs:6-8`, `templates.rs:17-21`).

## Open Areas
- No favicon/asset story exists at all; how imagery should reach the browser
  is unanswerable from this codebase.
- No examples of multi-page navigation, link elements, or additional routes —
  the homepage is the only page.
- `context! {}` is always empty; there is no existing pattern for passing
  dynamic data into templates.
