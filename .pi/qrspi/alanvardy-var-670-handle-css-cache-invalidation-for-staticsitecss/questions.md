# Research Questions

## Context
Focus on the Rust web server in `src/` (axum router, static file serving,
template rendering), the `templates/` directory, and the deployment
configuration (`Dockerfile`, `fly.toml`). Also consider how static assets
and routes are tested.

## Questions
1. How is the `/static` route configured and served (which service, which
   features of `tower-http` are enabled), and what HTTP caching-related
   headers (ETag, Last-Modified, Cache-Control) does that service emit by
   default for a static file response?
2. Where and how is CSS currently referenced or embedded in the HTML
   templates (e.g. inline `<style>` blocks, `<link>` tags), how are the
   minijinja templates organized and loaded, and how would an external
   stylesheet URL appear in the rendered pages today?
3. What middleware or router layers currently exist on the axum `Router`,
   and what patterns does the codebase already use for setting or
   overriding response headers on routes or responses?
4. How is the application built and deployed (Dockerfile contents, whether
   the `static/` directory is copied into the image, `fly.toml`
   configuration), and do static asset file paths or contents change
   identity between deploys?
5. What test patterns exist for the router and static file serving (test
   helpers in `src/test`, existing route tests), and how are HTTP response
   headers asserted in the existing test suite?
