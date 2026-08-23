# Routes

Base URL: `http://localhost:3000`

All `:3000` endpoints are subject to a global per-IP GCRA rate limiter. The
`/metrics` endpoint (`:9090`) is **not** rate-limited.

### GET /

Renders the home page.

- Response: `200 OK` — `text/html` (minijinja `templates/home.html`)
- Errors: `500` via `WebError` (template render failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.

---

### GET /singlethread

Renders the SingleThread page.

- Response: `200 OK` — `text/html` (minijinja `templates/singlethread.html`)
- Errors: `500` via `WebError` (template render failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.

---

### GET /dump/{key}

Returns all stored entries for `key`, in insertion order.

- Response: `200 OK` — `application/json`, array of `{ "id": i64, "body": <any JSON> }`; empty array (`[]`) for unknown keys
- Errors: `500` via `WebError` (database failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.

---

### POST /dump/{key}

Stores a JSON payload under `key`. Entries accumulate per key.

- Request body: arbitrary JSON (`application/json`)
- Response: `201 Created`
- Errors: `400` for malformed JSON (axum's built-in `Json` extractor response)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.
- Rate limit: also subject to a stricter dedicated tier (see
  `DUMP_TIER_*` in `src/app/rate_limit.rs`) nested inside the global budget.

---

### GET /health

Health check. Runs `SELECT 1` against the database pool; returns `200` when
the data layer responds, `500` (via the standard error path) otherwise.

- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.

---

### GET /static/{file}

Serves files from the `static/` directory (tower-http `ServeDir`).

- Response: `200 OK` — file contents with inferred content type
- Errors: `404` for missing files
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.

---

### GET /unsplash

Returns a random Unsplash photo (JSON), cached in the database for 6 hours.

- Response: `200 OK` — `application/json` `{ "url": ..., "photographer": ..., "created_at": ... }`
- Errors: `500` via `WebError` (database failure), `502` via `WebError` (upstream failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.
- Rate limit: also subject to a stricter dedicated tier (see
  `UNSPLASH_TIER_*` in `src/app/rate_limit.rs`) nested inside the global budget.

---
