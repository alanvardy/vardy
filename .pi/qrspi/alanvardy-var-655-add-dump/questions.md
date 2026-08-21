# Research Questions

## Context
This is a small axum 0.8 web service backed by sqlx (SQLite) and minijinja,
with all code under `src/` split into `app/` (db, error, state, templates)
and `interfaces/` (routes, handlers). Focus on: route registration and
handler conventions, database setup and migrations, request
extraction/response/error patterns, and the test harness.

## Questions
1. Trace the full flow of an HTTP request through the service: where is the
   router constructed, how are routes registered, how are handler modules
   organized and re-exported, and what handler signatures and return types
   do existing handlers use?
2. How is the SQLite database initialized and used: where is the connection
   pool created, what does `AppState` hold, how is the pool threaded into
   handlers, and what sqlx query style is used anywhere in the codebase?
3. How do database migrations work in this project: what migration files
   exist, what do they contain, and where (if anywhere) are migrations
   applied at startup versus in tests — does production `main.rs` run
   migrations at all?
4. What patterns exist for request extraction and responses: are there any
   usages of path parameter extractors (`Path`), JSON extractors, or JSON
   responses anywhere in the codebase or dependencies, and how does the
   error type (`WebError`) map errors to HTTP responses?
5. How are integration and unit tests structured: how does the test harness
   start the app and build clients, how do existing tests make requests and
   assert on responses, and what reqwest capabilities are available for
   sending JSON request bodies and inspecting JSON responses?
6. What naming, file-layout, and module-wiring conventions would a new
   handler area follow (e.g. how `home` and `singlethread` handlers are
   structured under `interfaces/handlers/`, and how `main.rs` declares
   modules)?
