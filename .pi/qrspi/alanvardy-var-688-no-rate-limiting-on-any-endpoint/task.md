# Task: VAR-688 — No rate limiting on any endpoint

The `vardy` web app has no rate limiting on any route: the only middleware
layers in the stack are request tracing (`TraceLayer` in `src/main.rs`) and a
static-file cache header. Every endpoint is unauthenticated, so an attacker can
hammer any of them — notably `POST /dump/{key}` (unauthenticated SQLite writes,
unbounded DB growth) and `GET /unsplash` (burns the upstream Unsplash API
quota).

Goal: add per-client-IP rate limiting as a tower/axum middleware layer with
env-configurable limits, returning 429 through the project's `WebError`
convention, covering at minimum `/dump` and `/unsplash` while explicitly
deciding the fate of `/health` (Fly.io probes it every 30s) and `/metrics`.
Tests must cover happy and sad paths, and `ROUTES.md` must be updated.
