# Design Discussion — CSS cache invalidation for `/static/site.css` (VAR-670)

## Current State

- All styling is a single inline `<style>` block in the base layout
  (`templates/layout.html:7-57`). There are no `<link rel="stylesheet">` tags
  and no external CSS files anywhere; the stylesheet ships inside every
  rendered HTML document, so browsers never cache it separately.
- The only external asset is an unversioned icon reference,
  `<img src="/static/singlethread-icon.png">` (`templates/singlethread.html:6`).
- `/static` is a bare `ServeDir::new("static")` nested with no layers
  (`src/interfaces/routes.rs:12`). tower-http has only the `fs` feature
  enabled (`Cargo.toml:11`) — no `set-header`.
- On a hit, ServeDir emits `Content-Type`, `Accept-Ranges`, and `Last-Modified`,
  and honors conditional requests (304s). It emits **no** `ETag` and **no**
  `Cache-Control` (tower-http 0.6.11 vendored source).
- With no `Cache-Control`, browsers fall back to heuristic caching — stale CSS
  can persist for an unbounded time after a deploy.
- No header-setting code exists anywhere in `src/`: no `.layer(...)`, no
  middleware, no `HeaderValue` construction. Error responses are plain
  `(StatusCode, &str)` tuples (`src/app/error.rs:22-33`).
- Templates load once via `minijinja` path loader into an `Environment` stored
  on `AppState.templates` (`src/app/templates.rs:1-13`, `src/app/state.rs:3`),
  initialized once in `src/main.rs` and tests (`src/test/mod.rs:7`). Page
  handlers render with empty contexts: `context! {}`
  (`src/interfaces/handlers/home/web.rs:8-12`,
  `src/interfaces/handlers/singlethread/web.rs:8-12`).
- No fingerprinting/versioning/build-step machinery exists anywhere in the repo.
  `static/` is copied verbatim into the Docker image (`Dockerfile:24`); fly.io
  builds remotely (`.github/workflows/fly-deploy.yml:17`).

## Desired End State

1. CSS lives in `static/site.css`, extracted verbatim from the inline
   `<style>` block in `layout.html` (work originally scoped to VAR-664,
   pulled into this branch), and is referenced from `layout.html` as a
   **self-versioned URL**: `/static/site.css?v=<hash>`.
2. Every `/static/*` response carries
   `Cache-Control: public, max-age=31536000, immutable`, so browsers cache
   aggressively but the URL itself changes whenever file contents change.
3. The icon reference is also self-versioned (`/static/singlethread-icon.png?v=<hash>`),
   so the pattern is uniform across all static assets from day one.
4. Verification:
   - Rendered pages contain `<link ... /static/site.css?v=` and
     `/static/singlethread-icon.png?v=`, and no longer contain a `<style>` block.
   - Existing page body-substring assertions keep passing (visual behavior is
     unchanged — the CSS moved byte-identical).
   - `GET /static/site.css` returns status OK with the expected `cache-control`
     header.

## Patterns to Follow

- **Router assembly stays declarative in `routes()`** — four entries today,
  built once and shared by prod and tests (`src/interfaces/routes.rs:7-13`);
  the new layer attaches there so `src/test/mod.rs:14-19` exercises the exact
  production wiring automatically.
- **Live-HTTP test pattern** — real listener + reqwest client
  (`src/test/mod.rs:5-25`), assertions via
  `res.headers().get(...).is_some_and(|v| v.to_str().unwrap().contains(...))`
  (`src/interfaces/routes.rs:30-33`). New cache-header tests copy this idiom.
- **One-time initialization in `templates::init()`** — the environment is
  created exactly once and stored on state (`src/app/templates.rs:1-13`);
  asset-hash computation belongs in that same startup path (or a sibling
  module called from it), not per-request.
- **Repo-relative paths assume CWD `/app`** — `ServeDir::new("static")`
  (`routes.rs:12`) and `path_loader("templates")` (`templates.rs:4`) both rely
  on the Dockerfile `WORKDIR /app` + copies (`Dockerfile:20,23-24`). Any code
  reading `static/` at startup must use the same relative convention.

## Patterns NOT to Follow

- **Manual version bumps** (`?v=1` edited by hand): humans forget; rejected in
  favor of content-derived hashes.
- **Build-time fingerprinting / CI asset pipeline**: nothing similar exists;
  gross overkill for two files.
- **Per-request hashing**: recompute cost on every render for files that only
  change at deploy time; hash once at startup instead.
- **Graceful degradation on missing assets**: silently serving unversioned URLs
  hides broken deploys; rejected in favor of fail-fast panics.

## Design Decisions

1. **Versioning strategy — startup content hash injected via template context**
   (Q1 = B). At startup, hash each file under `static/` (short SHA-256 prefix,
   e.g. 12 hex chars, via the `sha2` crate — deterministic across rebuilds so
   unchanged assets keep their cached URLs). Expose the map to templates as a
   minijinja **global function**, e.g. `{{ asset_url("site.css") }}` →
   `/static/site.css?v=a1b2c3d4e5f6`, registered on the `Environment` in
   `templates::init()` before it is stored on `AppState`. This keeps handler
   contexts empty (`context! {}` untouched) and makes staleness impossible
   without manual steps.
2. **Explicit `Cache-Control` on all of `/static`** (Q2 = A). Enable the
   tower-http `set-header` feature and wrap the nested service with
   `SetResponseHeaderLayer::overriding(CACHE_CONTROL,
   HeaderValue::from_static("public, max-age=31536000, immutable"))` around
   `ServeDir::new("static")` in `routes()`. Long max-age is safe *because*
   URLs are content-hashed (decision 1). This introduces the codebase's first
   `.layer(...)`, but it is confined to the static mount.
3. **Scope covers all static assets, not just CSS** (Q3 = B). Both the CSS link
   and the existing icon reference go through `asset_url(...)`. One pattern,
   no special cases.
4. **CSS extraction happens on this branch** (user decision). Move the full
   contents of the `<style>` block (`templates/layout.html:7-57`) verbatim
   into `static/site.css` and replace it with
   `<link rel="stylesheet" href="{{ asset_url('site.css') }}">`, leaving
   VAR-664 free to build on top. No CSS content changes in this step —
   extraction is byte-identical so page appearance is unchanged.
5. **Missing files panic at startup — fail fast** (user decision). If any file
   under `static/` cannot be read or hashed during initialization, init panics
   naming the offending path, e.g.
   `panic!("failed to hash static asset {}: {err}", path.display())`.
   A broken deploy should crash at boot rather than serve unversioned URLs;
   fly.io's health check (`GET /health`) will then fail and the release will
   not go live.
6. **Tests assert both the versioned URLs and the header** (Q4 = A):
   - Extend page tests (`handlers/*/web.rs`) to assert body contains
     `/static/site.css?v=` (home) and `/static/singlethread-icon.png?v=`
     (singlethread), plus absence of `<style>` in rendered layout output.
   - Extend the static test in `routes.rs` (or add a case) asserting
     `cache-control` contains `max-age=31536000` on `/static/site.css`.

## What We're NOT Doing

- No ETags (ServeDir doesn't generate them; `Last-Modified` + immutable
  Cache-Control make them redundant here).
- No fingerprinted filenames / rename-on-build step, no CI asset pipeline.
- No service worker, no HTML-level `no-cache` directives, no CDN config
  changes in `fly.toml`.
- No changes to error responses, other routes, or `WebError::into_response`.
- No runtime hot-reloading of hashes — hashes are computed once at process
  start; a redeploy restarts the process by definition.
- No CSS redesign or content tweaks here — extraction only; VAR-664 owns the
  visual redesign.

## Open Risks

Resolved during design review:
- ~~Ordering vs VAR-664~~ → extraction folded into this branch (decision 4).
- ~~Startup failure mode~~ → panic with a clear message (decision 5).

Remaining:
- **fly.io edge behavior**: unobservable from the repo; fly's proxy may add its
  own caching headers. Low risk given `immutable` + hashed URLs, but worth a
  post-deploy curl check.
- **First-deploy cache flush**: clients that heuristically cached the old
  unversioned URLs will re-fetch once; after that, hashed URLs keep everything
  correct. Accepted as a non-issue.
