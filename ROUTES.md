# Routes

Base URL: `http://localhost:3000`

All `:3000` endpoints are subject to a global per-IP GCRA rate limiter. The
`/metrics` endpoint (`:9090`) is **not** rate-limited.

### GET /

Renders the home page with a random Unsplash wallpaper and photographer credit
(linked name when a profile URL is available, plain text otherwise). The
wallpaper and credit gracefully degrade to hidden when the Unsplash fetch
fails.

- Response: `200 OK` — `text/html` (minijinja `templates/home.html`)
- Errors: `500` via `WebError` (template render failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.

---

### GET /singlethread

Renders the SingleThread page with an app-icon hero (icon badge + tagline),
platform badges (iPhone, iPad, Mac, Watch), a gradient decorative divider,
screenshot and watch-image cards with hover transitions, feature lists, an
FAQ section with collapsible Q&A pairs (native <details>/<summary> widgets), and a
closing CTA line. Includes a random Unsplash wallpaper and photographer credit
(linked name when a profile URL is available, plain text otherwise). The
wallpaper and credit gracefully degrade to hidden when the Unsplash fetch
fails.

- Response: `200 OK` — `text/html` (minijinja `templates/singlethread.html`)
- Errors: `500` via `WebError` (template render failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.

---

### GET /contact

Renders a two-column contact page: introductory copy about Alan in the left
column, and the contact form (name, email, message, CSS-hidden honeypot) in the
right column. Both columns stack vertically on mobile. Includes a random
Unsplash wallpaper and photographer credit that degrade to hidden when the
Unsplash fetch fails.

- Response: `200 OK` — `text/html` (minijinja `templates/contact.html`)
- Errors: `500` via `WebError` (template render failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.

---

### POST /contact

Accepts the contact form, skips email when the honeypot is filled (returns the
two-column thank-you page silently), otherwise sends the message to the
configured inbox via the Resend API and returns the two-column thank-you page
(introductory copy in the left column, confirmation message in the right).

- Request body: `application/x-www-form-urlencoded` (`name`, `email`, `message`,
  `_website` honeypot)
- Response: `200 OK` — `text/html` thank-you page
- Errors: `502` via `WebError` (Resend API failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.
- Rate limit: also a stricter dedicated tier (see
  `CONTACT_TIER_*` in `src/app/rate_limit.rs`) nested inside the global budget.

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

- Response: `200 OK` — `application/json` `{ "url": ..., "photographer": ..., "photographer_url": ..., "created_at": ... }`
- Errors: `500` via `WebError` (database failure), `502` via `WebError` (upstream failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.
- Rate limit: also subject to a stricter dedicated tier (see
  `UNSPLASH_TIER_*` in `src/app/rate_limit.rs`) nested inside the global budget.

---

### GET /unsplash/random

Returns a random Unsplash photo (JSON). If fewer than 5 pictures are cached,
fetches from Unsplash and inserts a new row; otherwise picks a random cached
row. No staleness timeout — the 5-row threshold controls when the cache is
refilled.

- Response: `200 OK` — `application/json` `{ "url": ..., "photographer": ..., "photographer_url": ..., "created_at": ... }`
- Errors: `500` via `WebError` (database failure), `502` via `WebError` (upstream failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.
- Rate limit: also subject to the same stricter dedicated tier as `/unsplash` (see
  `UNSPLASH_TIER_*` in `src/app/rate_limit.rs`) nested inside the global budget.

---
