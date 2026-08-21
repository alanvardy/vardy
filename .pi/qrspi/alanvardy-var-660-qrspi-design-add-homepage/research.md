# Research Findings

Reference repo: `/Users/vardy/dev/api` (live axum/minijinja HTML service).
Target repo (`vardy`): current working directory `/Users/vardy/dev/alanvardy-var-656-add-homepage`.

## Q1: How does the `api` repo bootstrap its HTTP server and wire template rendering into request handling?

### Findings
- `main()` in `src/main.rs:14-22` inits logging, `Env`, metrics (`Arc`), DB pool, then binds `TcpListener` to `0.0.0.0:{http_port}` (`main.rs:23-27`).
- **Template environment is created once**: `let templates = app::templates::init();` at `main.rs:40`. It is stored by value into `AppState.templates` and also cloned into `RealEmailer` (`main.rs:44-46`).
- `src/app/templates.rs:1-11` (entire file) is the single constructor:
  - `minijinja::Environment::new()` (line 2)
  - **Path loader**: `templates.set_loader(minijinja::path_loader("templates"))` (line 3) — CWD-relative filesystem loader; templates resolve relative to the repo root at runtime.
  - **Autoescape callback** (lines 4-8): closure over template `name`; `AutoEscape::Html` when name ends `".html"`, else `AutoEscape::None`. So `.html` templates are auto-escaped by default; `.txt`/emails are not.
- `app()` at `main.rs:60-90` receives `templates: minijinja::Environment<'static>` (param at `main.rs:69`), stored by value into `AppState { templates, ... }` (`main.rs:78`).
- Router built via `interfaces::routes::routes(env)` (`main.rs:83`), wrapped in global rate limiter, then `.with_state(state)` threads `AppState` in so handlers can extract it (`main.rs:84-86`).
- `AppState` in `src/app/state.rs:7-14` derives `Clone` and exposes `pub templates: minijinja::Environment<'static>` (line 13).
- Environment module exported via `src/app/mod.rs:14 pub mod templates;`. Handlers typed `Router<AppState>` receive it.

### How components connect
```
main.rs:40 init() -> Env<'static>
  -> stored in AppState.templates (main.rs:78, state.rs:13)
  -> .with_state(state) (main.rs:86)  ==> handlers read state.templates
  -> .clone() into RealEmailer (main.rs:45; infra/email.rs:34,41)
error.rs:358 LazyLock <templates::init> -> independent 3rd env for error pages
```
Handlers access via `State(state): State<AppState>` and call `state.templates.get_template(<file>)?.render(context! {...})` (e.g. `interfaces/handlers/encounters/web.rs:66,94-97`; same at `files/web.rs:35`, `locations/web.rs:45`, `users/web.rs:57-60`).

### Patterns observed
- Environment is **not a singleton** — 3 copies (AppState, emailer, error-env). All use the same `init()` config.
- Loader is CWD-dependent and read-only; no capability/read-only wrapper.
- Because autoescape is registered per-env in `init()`, `.html` escaping applies to every env including the error env.

## Q2: What is the exact handler pattern for an HTML page in `api`?

### Findings
Route registration (aggregator `src/interfaces/routes.rs:17-27`):
- `routes(env)` at `interfaces/routes.rs:17`; `Router::new().route(...)` composes; each resource `.nest("/<res>", f(env))`. `/users` subtree built in `users_web(env)`.
- `users_web` fn at `routes.rs:136`:
  - `.route("/web", get(handlers::users::web::list))` (`routes.rs:140`)
  - `.route("/web/users/{id}/delete", post(handlers::users::web::delete))` (`routes.rs:141`)
  - `.layer(from_fn_with_state(Arc::<str>::from(password), auth::require_web_password))` (`routes.rs:142-146`)
- `users()` at `routes.rs:165` merges web + json: `Router::new().merge(users_web(env)).merge(json)` (final line of fn).

Handler `src/interfaces/handlers/users/web.rs`:
- **Extractor**: `State(state): State<AppState>` for `list` (web.rs:35-36). `delete` uses `Path(id): Path<UserId>` + `State(state)` (web.rs:73-74).
- **Return types**: `Result<Html<String>, WebError>` (list, web.rs:35); `Result<Redirect, WebError>` (delete, web.rs:73). `WebError` = `app::error::WebError`; success wrapped in `axum::response::Html`.
- **Template fetch/render** (web.rs:57-62):
  ```rust
  let html = state.templates
      .get_template("users.html")?
      .render(context! { users => views })?;
  Ok(Html(html))
  ```
  `context!` binds template var `users` to `Vec<UserView>`; `?` propagates missing template / render failure to `WebError`. View structs `UserView`/`FileThumbView` are `serde::Serialize`-derived and passed into the minijinja namespace (web.rs:8-34).
- Auth is router-level (`.layer` + `require_web_password`), not a handler extractor.

### Patterns observed
The same web-handler wiring repeats for every resource:
`files_web` (routes.rs:99-108), `encounters_web` (:125-133), `locations_web` (:136-144), `feature_flags` (:31-...), `users_web` (:136-146). JSON handlers use `require_jwt` layer; HTML web handlers use `require_web_password`.
Delete follows the form-POST->303 pattern (`Redirect::to("/users/web")`, web.rs:79; `WebError::NotFound` web.rs:80).

## Q3: How are the `api` HTML templates composed?

### Findings
Base layout `templates/layout.html`:
- `<!DOCTYPE html>` / `<html lang="en">` (1-2); `<title>{% block title %}Admin{% endblock %}` (:6); inline `<style>` with CSS variables (7-74, single block, no override).
- Nav `active` blocks (`layout.html:108-113`): `{% block active_flags %}{% endblock %}`, `{% block active_users %}`, `{% block active_files %}`.
- Content blocks: `<h1>{% block heading %}</h1>` (line 117) and `{% block content %}{% endblock %}` (line 118).

`.html` page templates all begin `{% extends "layout.html" %}` and override a subset of `title`, one `active_*` block, `heading`, `content`:
- `users.html:1-12` — extends; overrides `title`, `active_users`, `heading`, `content`; renders `<table>` with `{% for user in users %}` (users.html:11) and `{% if user.files %}` thumbnail loop (:39-47).
- `encounters.html:1` — overrides `title`, `active_users`(sic), `heading`, `content`; uses `{{ "s" if count != 1 else "" }}` pluralization (:13-15), `{% for e in encounters %}` (:29).
- `locations.html` — bare twin of encounters (title/active_users/heading/content; pluralization; `{% for %}`).
- `error.html:1-9` — simplest: extends + overrides `title`, `heading`, `content`; renders ctx vars `code` and `message`; does NOT set any `active_*` block (so no nav highlight).
- `encounter_detail.html:47-62` — embeds minijinja `{{ }}` inside `<script>` JS (`var latA = {{ ... }}`), i.e. HTML-escaped values interpolated into JS literals.

**Autoescape**: no `{% autoescape %}` / `|safe` directive anywhere in templates. Escaping is engine-level only: `.html` => `AutoEscape::Html` (`templates.rs:5-8`). Encounter_detail interpolates into JS while still subject to HTML escaping.

### Patterns
One layout, many extending pages via `{% extends %}` + named block override. `active_*` nav highlighting is a per-page opt-in named block; error/detail pages skip it. Variables flow in via Rust `context! { name => value }`; the template namespaces match the Serialize field names owned on the handler `view` structs.

## Q4: How are HTML web routes tested in `api`?

**Bootstrap** (in-handler `#[cfg(test)] mod tests`, e.g. `users/web.rs:100+`):
- `#[sqlx::test] async fn ... (pool: PgPool)` — sqlx provisions DB/tx.
- `start_app(pool).await` returns a bound `SocketAddr` (`src/test/mod.rs:189-230`): builds `Env` with `WEB_USERNAME="admin"`, `WEB_PASSWORD="test-password"` (mod.rs:25,27), `tcp`-binds `127.0.0.1:0`, fills FakeStorage/FakeEmailer + `crate::app::templates::init()`, then `tokio::spawn`s the real axum app it so tests hit over HTTP (mod.rs:220-230).
- `test_client()` / `test_client_no_redirect()` (mod.rs:41-49): `reqwest::Client::new()` (+ a no-redirect variant for 303 form POSTs).
- Seed helpers: `seed_user(&pool)` (mod.rs:105); encounters use local `seed_encounter` (encounters/web.rs:149+).

**Requests**: `client.get(format!("http://{addr}/users/web"))` `.basic_auth(WEB_USERNAME, Some(WEB_PASSWORD))` `.send()` (`users/web.rs:105-116`); POST delete with no-redirect client (`users/web.rs:204-`); query params e.g. `/encounters/web/{id}?from_user=...` (encounters/web.rs:237-244).

**Assertions**:
- Status: `Status::OK` (users/web.rs:129-130; encounters), `SEE_OTHER` 303 + `location: /users/me` (users/web.rs:222-227), `NOT_FOUND` 404 (users/web.rs:247-253), `UNAUTHORIZED` for missing creds (users/web.rs:159-160).
- Content-type: header filtered, `is_some_and(|v| v.contains("text/html"))` (users/web.rs:131-133).
- Body: `assert!(body.contains("<table"))`, `contains("test-user")` (users/web.rs:138-139); thumbnail `class="thumbnail"` `/fake-presigned` (:193-195); 404 HTML-not-JSON via `contains("Error 404")` (users/web.rs:253-262); encounters table rows + `?from_user=` (:175-180).

### Patterns observed
Every web test follows: `seed` -> `start_app(pool)` -> `test_client[_no_redirect]()` -> `client.get/post(...)`.basic_auth -> assert status + `content-type` + `body.contains(<markup>)`. Shared harness in `src/test/mod.rs` reduces per-handler test boilerplate.

## Q5: What is the present state of the `vardy` repo?

**Bare, zero-dependent console program.**
- `Cargo.toml:1-4`: `[package] name="vardy" version="0.1.0" edition="2024"`; `[dependencies]` is **empty** — no axum, no minijinja, no template engine, no dev-dependencies.
- `Cargo.lock:1-9`: `version=4` + single `[[package]] name="vardy"` — no third-party crates at all.
- `src/main.rs:1-12`: `fn main(){ println!("Hello, world! {}", greeting()); }`, plus `fn greeting()` and one `#[test] test_greeting` (`assert_eq!`).
- **No HTTP surface**: no route handler, no template rendering, no `templates/` dir, no `axum`/`minijinja` references anywhere in the source tree (grep-confirmed).
- Toolchain `rust-toolchain.toml:1-6`: `channel = "1.97.1"`, `components=["clippy","rustfmt"]`.
- Test runner `.config/nextest.toml:1-2`: single `[profile.ci.junit] path="junit.xml"` (JUnit output for CI).
- `.gitignore`: only `/target`.
- `codecov.yml:1-12`: **ignores `src/main.rs`**; project-avg target 70%, patch target 90% (new code must be covered).
- `scripts/lint_string.sh:1-18`: greps all `*.rs` files for forbidden strings (`FIXME `, `FIXME:`, `fixme `, `fixme:`, `dbg!`). Only `*.rs` — no HTML-template lint scope.
- `.github/workflows/ci.yml`: runs nextest (JUnit) + mold linker (v2.37.1) + `cargo-llvm-cov`/`codecov` + `cargo clippy` + `cargo fmt --check` on every push/PR. Uses `taiki-e/install-action` (cargo-llvm-cov, nextest). No path gating.
- `.github/workflows/ci-secure.yml`: CodeQL weekly (Thursdays 17:26 UTC) for actions+rust, plus `clippy-analyze` SARIF.
- `.github/dependabot.yml`: daily cargo + github-actions updates, auto-merge for minor/patch.
- **No `AGENTS.md`** anywhere in `vardy` — `find **/AGENTS.md` returns nothing. `.pi/qrspi/*/questions.md` + `task.md` describe the intended homepage design only, not module conventions.

### The gap (bare console -> template service)
- add dependencies (axum, minijinja) + transitive graph to `Cargo.toml`/`Cargo.lock`;
- add `src/server bootstrap (matches api's `templates::init` + `Router<AppState>`)`, routes, handlers, `templates/*.html`;
- note in CI: `lint_string.sh` only checks `*.rs`; code in new `.html`/non-main.rs files counts toward the 90% patch target.

## Cross-Cutting Observations
- **Single global template bootstrap function**: `api`'s `src/app/templates.rs:init()` is the central knob shared by app state, emailer, and error-page paths; clone-by-value in a `#[derive(Clone)]` struct is the pattern.
- **HTML handlers are structurally uniform**: `Router<AppState>` + `.route("/<web>", get(handlers::<res>::web::list))` pending auth layer; `State<AppState>` extractor; `Html<String>`/`Redirect` return; template render `get_template(...).render(context! {...})`.
- **Templates base** is a single `layout.html` extensible via named blocks; autoescape is engine-level (`.html` suffix) — never per-template.
- **Testing** relies on `src/test/mod.rs` bootstrap (`start_app`, `test_client`, seeded `WEB_*` creds) and `#[sqlx::test]` DB provisioning.
- **vardy is the inverse baseline**: zero deps, single console `main`, no HTTP/template surface — no `.app/` layer, no `templates/`, no `test/mod.rs`.
- `api`'s design references back to the same `src/app/templates.rs::init` for both the emailer and error env — this is the cross-component (email + web + error) integration point.

## Open Areas
- `env`-level metrics port `9090` (`main.rs:50-54`) serves `/metricsRouter` on a separate port; exact usage not required here but distinct from `AppState` templates.
- Exact `WebError` to-HTML status mapping (404 page) in error.rs:360-380 `render_error_page`/`fallback_error_html` is a secondary path; the root 404-HTML assertion lives in users/web.rs:253.
- `encounter_detail.html`'s JS interpolation interacts with HTML autoescape in a way not covered by any template test.
- No evidence in `vardy` of an `AGENTS.md`/module convention beyond `rust-toolchain.toml` + `ci.yml`; CI has no path-gating to protect non-`.rs` files.