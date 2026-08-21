# Design Discussion — Add a hello-world homepage to `vardy`

## Current State

`vardy` is a bare Rust console program with zero third-party dependencies:

- `Cargo.toml:1-4` — `[package]` with `edition="2024"`; `[dependencies]` is
  **empty**. `Cargo.lock:1-9` confirms no third-party crates.
- `src/main.rs:1-6` — `fn main() { println!("Hello, world! {}",
  greeting()); }` plus `greeting()` and a single `#[test] test_greeting`.
- **No HTTP surface**: no route handler, no template engine, no `templates/`
  dir. Grep confirms no `axum`/`minijinja` references anywhere.
- Toolchain: `rust-toolchain.toml:1-6` (`channel="1.97.1"`, clippy/rustfmt);
  runner `.config/nextest.toml:1-2` (JUnit for CI); CI at
  `.github/workflows/ci.yml` runs nextest + mold linker + codecov + clippy +
  fmt-check. `codecov.yml:1-12` **ignores `src/main.rs`** but has a **90%
  patch target** for new (non-main) code.

The reference repo is `api` at `/Users/vardy/dev/api`, a live axum/minijinja
HTML service. Its relevant mechanics (from research):

- Template env built once by `src/app/templates.rs:1-11` `init()`: a
  `minijinja::Environment::new()` with `path_loader("templates")` (line 3)
  and an autoescape callback that applies `AutoEscape::Html` to any name
  ending `.html`, else `AutoEscape::None` (lines 4-8). Stored by value into
  `AppState.templates` (`api src/main.rs:40,78`, `src/app/state.rs:13`).
- Handler shape: `Router<AppState>` + `.route(...)`; handlers extract
  `State(state): State<AppState>` and return `Result<Html<String>,
  WebError>`; render via `state.templates.get_template("<file>")?.render(
  context! { ... })?` (`api handlers/users/web.rs:57-62`).
- Templates: single `templates/layout.html` with named blocks
  (`title` :6, `active_*` nav :108-118, `heading`/`content` :117-118); every
  page `{% extends "layout.html" %}` overriding a subset.
- Tests: live HTTP bootstrap in `src/test/mod.rs:189-230` (`start_app` binds
  a real listener, spawns the axum app; `test_client` is a reqwest client);
  per-handler tests assert status, `content-type` contains `text/html`, and
  `body.contains(<markup>)` (`api/src/interfaces/handlers/users/web.rs:100+`).

## Desired End State

`vardy` serves an HTML homepage at `/` rendered from a minijinja template,
following `api`'s templating structure.

- New module skeleton mirroring `api` (trimmed to homepage needs):
  `src/app/templates.rs` (`init()`), `src/app/state.rs` (a minimal
  `AppState { templates }`), `src/interfaces/routes.rs`, and a handler
  `src/interfaces/handlers/home/web.rs`.
- `src/main.rs` becomes the server bootstrap: build the listener, call
  `app::templates::init()`, construct `AppState`, build the router with
  `.with_state(state)` (mirroring `api` `main.rs:60-90`), and serve. The
  console greeting is **removed** — the process becomes HTTP-only.
- `templates/layout.html` + a `templates/home.html` page that does
  `{% extends "layout.html" %}` and overrides `title`, `heading`, `content`.
  Nav-highlight blocks optional (single page).
- Tests: live HTTP test asserting `200`, `content-type` contains
  `text/html`, and body contains expected markup (mirrors api pattern).
  `src/test/mod.rs`-style bootstrap harness (`start_app` + `test_client`)
  in this repo, sized for a stateless app (no DB).

**Verification**: `cargo test` runs the live HTTP test green; `curl`/browser
GET `/` returns the rendered homepage; `cargo clippy` and
`cargo fmt --check` pass; new (non-main) `.rs` code is covered against the
CI 90% patch target.

## Patterns to Follow

Grounded in `api` + current `vardy`:

1. **Template init singleton fn** — `src/app/templates.rs:init()` shape
   (`Environment::new()`, `path_loader`, autoescape-by-`.html`). One knob,
   stored by value in a `#[derive(Clone)]` state struct (`api` state.rs:7-14).
2. **Handler wiring** — `Router<AppState>` + `State(state)` extractor +
   `Result<Html<String>, WebError>`; render via
   `get_template(...)?.render(context! {...})?` (`api` users/web.rs:55-62).
3. **Layout/extends composition** — shared `layout.html` with named blocks;
   pages `{% extends %}` and override `title`/`heading`/`content` (`api`
   templates/users.html:1-12). Autoescape is engine-level (`.html` suffix),
   never a per-template directive.
4. **Live HTTP tests** — `test_client` hitting a bound listener + status /
   `content-type` / `body.contains()` assertions (`api` src/test/mod.rs,
   users/web.rs).
5. **`main.rs` ignored by codecov** (`codecov.yml:1-6`) — keep server
   bootstrap (non-covered) in `main.rs`, put covered logic in the handler /
   templates sources.

**Patterns NOT to follow**:
- Do **not** replicate `api`'s DB/auth/email/metrics layers, `Env`,
  `require_web_password` auth, or rate limiter — `vardy` has no such needs.
- Do **not** clone the multiport metrics server (`api` main.rs:50-54) or the
  emailer's 3rd template env.
- Do **not** copy `api`'s large module tree wholesale; deliberately trim.

## Design Decisions

1. **Scope of architecture (Q1=B)**: mirror `api`'s structural shape
   (`app/` + `interfaces/` + `templates/`) but only the pieces a homepage
   needs. Keeps the codebase a faithful, extensible reference without the
   DB/auth/email subsystems.
2. **Composition (Q2=A)**: use `layout.html` + `{% extends %}` even for one
   page. Matches the reference templating pattern exactly and sets up
   multi-page growth.
3. **Testing (Q3=A)**: live HTTP test over a bound listener with reqwest,
   asserting status + `text/html` + markup. Mirrors api's harness; covers the
   handler/routes code (which counts toward the 90% patch target, unlike
   `main.rs`).
4. **Bootstrap role (Q4=A)**: `main.rs` is the server bootstrap; remove the
   console greeting. Stays coverage-ignored, matches api layout.
5. **Dependencies (Q5)**: add the same proven dependency set as `api`:
   `axum = "0.8.9"`, `minijinja = { version = "2", features = ["debug"] }`,
   and (dev) `reqwest` for tests. Grows `Cargo.lock` accordingly.

## What We're NOT Doing

- No databases, storage, auth/credentials, email, metrics, Sentry, or
  rate limiting.
- No multi-page nav/`active_*` highlighting, no JSON handlers, no 404/error
  page plumbing beyond what the chosen status path requires.
- No metrics/health endpoints or second listener port.
- No production HTTP/HTTPS TLS binding — localhost/plain axum serve only.
- No changes to CI workflows, toolchain, `.gitignore`, or lint script scope
  (it only checks `*.rs`).

## Open Risks

- **Coverage target**: new `.rs` files (templates init, routes, handler) hit
  the 90% patch target; `main.rs` stays ignored. If the live HTTP test
  doesn't cover all branches, clippy/codecov may flag it.
- **Cargo lock rewrite**: adding axum/minijinja/reqwest changes `Cargo.lock`
  substantially; mold `edition="2024"` API surface must line up with the
  pinned versions.
- **Axum bootstrapping without DB**: `api`'s `app()` relies on `PgPool`;
  trimming this for a homepage is a small new integration — exact
  `AppState`/router bootstrap may need adjustment not previewed in research.
- **Template path dependence**: `path_loader("templates")` is CWD-relative
  (`api` templates.rs:3); the test must run with the repo root as CWD or the
  loader setup must make template discovery robust across the test runner.