# Research Questions

## Context

This is an axum web server backed by SQLite (sqlx) that proxies the Unsplash
API. Focus on the `/unsplash` endpoint flow: the handler at
`src/interfaces/handlers/unsplash/`, the orchestrating `Picture` layer at
`src/app/picture.rs`, the domain type at `src/domain/picture.rs`, the upstream
API client at `src/infra/unsplash.rs`, the `unsplash_pictures` table
(`migrations/0003_unsplash_pictures.sql`), and the test harness in
`src/test/mod.rs` plus inline handler tests.

## Questions

1. Trace the full data flow for the `/unsplash` endpoint: how a `Picture` is
   fetched from the Unsplash API, orchestrated through `src/app/picture.rs`,
   returned by the handler in `src/interfaces/handlers/unsplash/json.rs`, and
   serialized as the JSON response. What fields does the `Picture` type carry,
   and where exactly is each populated?
2. How is the `Picture` domain type and its database persistence designed?
   Describe the struct definition in `src/domain/picture.rs` (its serde and
   `sqlx::FromRow` derives), the `unsplash_pictures` table schema in
   `migrations/0003_unsplash_pictures.sql`, and every query in
   `src/app/picture.rs` that reads or writes picture rows — including the
   exact column lists in each SQL statement.
3. How is the upstream Unsplash API response modeled and parsed in
   `src/infra/unsplash.rs`? Which sub-objects and fields of the
   `/photos/random` response are extracted into the three internal
   `Deserialize` structs, where does the parsing happen, and how does the code
   behave if a parsed field is missing or the response is malformed?
4. How does the `/unsplash` handler shape its JSON response and handle errors?
   Describe how `Json<Picture>` sets the response content-type and body, and
   how `WebError` (in `src/app/error.rs`) maps upstream-fetch and parse
   failures (via `From<UnsplashError>`) into both the HTTP status and the
   response body.
5. What test conventions cover the `/unsplash` endpoint? How do the inline
   tests in `src/interfaces/handlers/unsplash/json.rs` and the helpers in
   `src/test/mod.rs` stub the upstream Unsplash API, seed and clear the
   `unsplash_pictures` table, and assert on the returned JSON payload fields?