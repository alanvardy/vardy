# Research Findings

App: Rust/Axum repo at repo root. Reference: Elixir/Phoenix site at `/Users/vardy/dev/alan_vardy`.
Line numbers for `templates/layout.html`, `templates/home.html`, `templates/singlethread.html`,
`src/interfaces/routes.rs`, and `src/interfaces/handlers/home/web.rs` verified directly.

## Q1: How does the minijinja templating layer work end-to-end?

### Findings
- Dependency: `minijinja = { version = "2", features = ["debug"] }` — `Cargo.toml:8`. Only the `debug` feature; no loader extras.
- Environment init is one function: `src/app/templates.rs:1-11`
  - `Environment::new()` — `src/app/templates.rs:3`
  - `set_loader(minijinja::path_loader("templates"))` — `src/app/templates.rs:4` (relative path, resolved against process CWD)
  - Autoescape callback: names ending `.html` → `AutoEscape::Html` (`templates.rs:5-7`); everything else `AutoEscape::None` (`templates.rs:8-10`)
  - No filters, globals, tests, or `add_template` calls; templates load lazily by name.
- Unit test documents the autoescape contract: `<b>` renders escaped in `.html`, raw in `.txt` — `src/app/templates.rs:14-41`.
- State wiring: `AppState { pub templates: minijinja::Environment<'static> }` (derive `Clone`) — `src/app/state.rs:1-3`; built once in `main` — `src/main.rs:6-8`; attached via `.with_state(state)` — `src/main.rs:15`.
- Layout inheritance: `templates/layout.html` is the base with three blocks:
  - `{% block title %}Home{% endblock %}` — `layout.html:6`
  - `{% block heading %}{% endblock %}` inside `<h1>` — `layout.html:59`
  - `{% block content %}{% endblock %}` — `layout.html:60`
- Children: both `templates/home.html:1` and `templates/singlethread.html:1` do `{% extends "layout.html" %}` and override exactly `title`, `heading`, `content`. Neither uses any `{{ ... }}` variables — all rendered text is static markup.
- Handler context: `src/interfaces/handlers/home/web.rs:6-12` — `state.templates.get_template("home.html")?.render(context! {})?` — the context is the **empty** `context!` macro; no data passed. Identical in `src/interfaces/handlers/singlethread/web.rs:6-12`.
- Return type: `Result<Html<String>, WebError>` (`home/web.rs:6`); `?` converts `minijinja::Error` → `WebError::Template` via `From` — `src/app/error.rs:14-16`.
- Autoescape in practice: both pages render `.html` names, so `AutoEscape::Html` applies (determined by the child template name); with empty contexts, escaping is currently a no-op on live paths.

## Q2: What does the reference site's homepage display and where does content live?

### Findings
- Route: `get "/", PageController, :index` — `lib/alan_vardy_web/router.ex:16`. Controller fetches newest post and renders with assigns: `render(conn, "index.html", page_title: "Welcome", latest_post: latest_post)` — `lib/alan_vardy_web/controllers/page_controller.ex:9-13`.
- Template: `lib/alan_vardy_web/templates/page/index.html.heex`, sections in order:
  1. **Greeting** — wave icon `priv/static/images/wave.svg` (`index.html.heex:5`) + hard-coded "Hi!" (`:8`).
  2. **Bio** — two hard-coded `<p>` blocks: "My name is Alan Vardy…" West Coast of Canada (`:11-13`); Elixir/Rust "high-output individual contributor" paragraph (`:14-16`).
  3. **"You are invited to" icon links** (`:18-41`) — three hard-coded links: blog → `Routes.post_path(:index)` with `quill.png` (`:21-24`); GitHub → `https://github.com/alanvardy` with `github.svg` (`:28-31`); LinkedIn → `https://www.linkedin.com/in/alanvardy/` with `linkedin.svg` (`:35-38`).
  4. **Portrait photo** — `priv/static/images/alanvardy.jpg` (`:44-46`).
  5. **Latest Post** — hard-coded heading (`:48`); renders partial `post/_post.html.heex` with `post: @latest_post` (`:51`).
- Post preview partial `lib/alan_vardy_web/templates/post/_post.html.heex:1-12`: `@post.title` (linked), `@post.date`, `@post.description` with "more" link, tags.
- Post data source: `AlanVardy.Blog` uses `Postex, prefix: "https://www.alanvardy.com/posts/"` — `lib/alan_vardy/blog.ex:1-4`; Postex reads markdown from repo-root `posts/` (year folders) with `==key==` front-matter (title/author/description/tags/body) — e.g. `posts/2023/05-21-compose-test-setup.md:1-13`. Post previews come from **external markdown files**.
- Layout chrome: `root.html.heex` → `app.html.heex` renders `_navbar.html.heex` + `{@inner_content}` + `_footer.html.heex`. Navbar brand "alanvardy" hard-coded (`_navbar.html.heex:2-6`); nav links from inline list `[{"Home",…},{"Blog",…},{"About Me",…},{"Contact",…}]` (`:15`); inline GitHub/LinkedIn SVGs (`:22-49`, `:51-78`); inline JS mobile-menu toggle (`:80-89`).
- Content provenance summary: bio/greeting/links/headings **hard-coded in the heex template**; aboutme card content in a view module attribute `@cards` — `lib/alan_vardy_web/views/page_view.ex:5-35`; posts from markdown files; **nothing from `config/*.exs`** (only a dev watcher for `posts/*`, `config/dev.exs:55`).

## Q3: How is styling structured (app vs reference)?

### Findings
- Rust app: all CSS in one inline `<style>` in `templates/layout.html:7-51`; no external CSS, no asset pipeline.
- CSS custom properties on `:root` — `layout.html:8-14`: `--bg: #121212`, `--surface: #1e1e1e`, `--text: #e0e0e0`, `--muted: #9e9e9e`, `--accent: #7aa2f7` (dark theme). `--muted` is defined but unused.
- Rules: `*` box-sizing (`:16-18`); `body` dark bg/text + system font stack + line-height 1.6 (`:19-25`); `.container` max-width 48rem centered (`:26-30`); `.card` surface bg, hardcoded `#333` border, 8px radius (`:31-36`); `nav` flex row, surface bg, bottom border (`:37-43`); `nav a` (`:44-47`); `nav a:hover` accent (`:48-50`). No media queries or animations.
- Class usage: `home.html:5-7` and `singlethread.html:6-13` wrap content in `.card`; `singlethread.html:5` `<img>` is unstyled; nav markup at `layout.html:54-57`.
- Reference site: Tailwind CSS v3 via Mix `tailwind` tool + esbuild — `mix.exs:69`, `mix.exs:62`; pipeline config `config/config.exs:30-39` (JS) and `:47-54` (CSS → `priv/static/assets/app.css`); `assets/tailwind.config.js:3-15` (content globs, `@tailwindcss/forms` plugin).
- Stylesheet linked in `root.html.heex:11` (`/assets/app.css`); body classes `bg-slate-400` (`root.html.heex:20`), main wrapper `m-2 md:m-5 bg-gray-50 px-5 pt-5 pb-10 rounded shadow` (`:26`).
- `assets/css/app.css` (399 lines): Tailwind base/components/utilities imports (`:1-3`) plus hand-written custom CSS — `.blog-body` typography/lists, blockquote cards, code + syntax-highlight token classes, `.alert*`, LiveView classes, fade animations, `.copy-button` (`app.css:6-398`). **No CSS variables** — colors are hardcoded literals.
- Homepage uses Tailwind utilities inline: `flex`/`basis-3/4`/`basis-1/4` columns (`index.html.heex:1-2,44-46,49-53`), `text-3xl` (`:8`), `border-l-4 border-orange-700 pl-4` link list (`:19`), `rounded` avatar (`:45`). Navbar: `bg-slate-800 p-6`, hover transitions (`_navbar.html.heex:1,27-28`).

## Q4: How are static assets served and organized?

### Findings
- Rust app: `.nest_service("/static", ServeDir::new("static"))` — `src/interfaces/routes.rs:10` (tower-http `ServeDir`, imported `routes.rs:3`); relative `static/` dir at repo root.
- Assets on disk: exactly one file — `static/singlethread-icon.png`.
- Template reference: `templates/singlethread.html:5` — `<img src="/static/singlethread-icon.png" ...>`. `home.html` and `layout.html` reference no static assets (no favicon; CSS is inline).
- Reference site: `Plug.Static` at `/` from `priv/static`, whitelist `~w(assets fonts images favicon.ico robots.txt)` — `lib/alan_vardy_web/endpoint.ex:20-24`; templates use `Routes.static_path(@conn, "/images/...")`.
- `priv/static/images/` contents: `wave.svg`, `quill.png`, `github.svg`, `linkedin.svg`, `rss.svg`, `alanvardy.jpg`, `newzealand.jpg`, `family.jpg`, `combatengineer.jpg`, `refrigeration.jpg`, `bottle.jpg`, `bridge.png`, `startupedmonton.jpg`, `elixircode.png`, `phoenix.png`, plus blog images under `images/blog/{2020..2023}/`.
- Homepage image refs: wave `index.html.heex:5`, quill `:22`, github `:29`, linkedin `:36`, portrait `:45`; aboutme card paths in `page_view.ex:13,21,27,33`.

## Q5: How do HTTP tests assert page content?

### Findings
- Helpers — `src/test/mod.rs`: `start_app()` (`:5-19`) builds real `AppState` + `routes()`, binds `127.0.0.1:0`, spawns `axum::serve` in a tokio task, returns `SocketAddr`; `test_client()` (`:22`) returns a plain `reqwest::Client`.
- Uniform test pattern: `#[tokio::test]` → `start_app()` → `client.get(format!("http://{addr}/…"))` → assert `StatusCode` → assert content-type header via `is_some_and(|v| v.to_str().unwrap().contains(...))` → `res.text()` → `assert!(body.contains("…"))` substring checks.
- Home page test `index_serves_ok_html` — `src/interfaces/handlers/home/web.rs:18-41`. Exact assertion strings (`:34-38`):
  - `"<title>Home</title>"`
  - `"Welcome to vardy"`
  - `"This is the vardy homepage, rendered with minijinja."`
  - `r#"<a href="/">Home</a>"#`
  - `r#"<a href="/singlethread">SingleThread</a>"#`
  - Sources: title block `layout.html:6` (default "Home"), heading `home.html:3`, body `home.html:5`, both nav links `layout.html:55-56`.
- SingleThread test — `src/interfaces/handlers/singlethread/web.rs:18-41`. Exact strings (`:34-39`): `"<title>SingleThread</title>"`, `"<h1>SingleThread</h1>"`, `"single line of work"`, `r#"<img src="/static/singlethread-icon.png""#` (open-tag prefix only), plus both nav anchors.
- Static route test `static_icon_is_served` — `src/interfaces/routes.rs:17-32`: GET `/static/singlethread-icon.png`, asserts 200 + content-type contains `"image/png"`; no body assertions.
- Error unit tests (no HTTP) — `src/app/error.rs:37-47`: `WebError::NotFound` → 404; `minijinja::Error` (TemplateNotFound) → 500. Bodies (`"not found"` / `"internal server error"`) unasserted.
- Sensitivity note: assertions are raw substring `contains` on full rendered HTML — sensitive to whitespace/attribute changes in `layout.html` nav anchors and the `<img` prefix.

## Q6: How are routes/handlers wired for a new page? Does the reference site have more pages?

### Findings
- Router composition — `src/interfaces/routes.rs:6-11`: `pub fn routes() -> Router<AppState>` with `.route("/", get(handlers::home::web::index))`, `.route("/singlethread", get(handlers::singlethread::web::index))`, `.nest_service("/static", …)`. Adding a page = one `.route(...)` line.
- Handler module structure: each page is a directory under `src/interfaces/handlers/` with `mod.rs` (one-line `pub mod web;`) and `web.rs` (handler + `#[cfg(test)]` HTTP tests) — `src/interfaces/handlers/home/mod.rs:1`, `home/web.rs:6-12`. Handler signature: `pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError>`.
- AppState — `src/app/state.rs:1-3`: only `templates: minijinja::Environment<'static>`; no DB/config.
- Error handling — `src/app/error.rs`: `WebError { Template(minijinja::Error), NotFound }` (`:8-11`); `From<minijinja::Error>` (`:14-16`); `IntoResponse` maps NotFound → 404 "not found" and Template → eprintln + 500 "internal server error" (`:19-28`). `NotFound` is `#[allow(dead_code)]`, only constructed in tests.
- Startup: `src/main.rs:5-18` — builds state, binds `0.0.0.0:3000`, serves `routes().with_state(state).into_make_service_with_connect_info::<SocketAddr>()`.
- Reference site routes — `lib/alan_vardy_web/router.ex:13-24` (all in `:browser` pipeline): `/` (home), `/aboutme`, `/blog` + `/blog/:page` (paginated), `/blog/rss.xml`, `resources /post` (show), `resources /tag` (show), `/contact` GET+POST (form + create with email via `lib/alan_vardy/mailer.ex`).
- Reference templates per page: `page/` (index, aboutme + partials), `post/` (index, show, rss, partials), `tag/show`, `contact/new`; shared shell `layout/root.html.heex`, `app.html.heex`, `_navbar.html.heex`, `_footer.html.heex`.

## Cross-Cutting Observations
- One-page = one route entry + one handler dir (`mod.rs`/`web.rs`) + one template extending `layout.html`; both existing pages follow this identically, and tests live inside the same `web.rs`/`routes.rs` files under `#[cfg(test)]`.
- Handlers pass **empty render contexts**; all page content is static template markup. Dynamic content would be a new pattern for this codebase (the autoescape machinery exists but is unexercised on live paths).
- The Rust app's dark theme (`--bg/--surface/--text/--accent`) vs the reference's Tailwind slate/orange palette; the reference has no CSS variables, the Rust app has no build step.
- Reference homepage content is hard-coded in the template (except post previews from markdown via Postex) — the closest analog in the Rust app is hard-coded text in `home.html`.
- Both apps serve assets from a repo-relative directory (`static/` vs `priv/static`); both depend on process CWD.
- Reference site is fully multi-page (home/about/blog/post/tag/contact/RSS) under a shared navbar/footer shell — structurally similar to `layout.html` + per-page child templates in the Rust app.

## Open Areas
- No favicon exists in the Rust app; the reference has `priv/static/favicon.ico` (serving behavior for it in the Rust app's `ServeDir` whitelist — none configured — was not tested).
- The reference's `_invitation.html.heex` uses classes (`biglist`, `borderleft`) not defined in `app.css` — legacy/dangling; unclear if intentionally unused.
- Dockerfile/deploy handling of the relative `static/` and `templates/` paths was not examined.
