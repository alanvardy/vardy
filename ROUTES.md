# Routes

Base URL: `http://localhost:3000`

### GET /

Renders the home page.

- Response: `200 OK` — `text/html` (minijinja `templates/home.html`)
- Errors: `500` via `WebError` (template render failure)

---

### GET /singlethread

Renders the SingleThread page.

- Response: `200 OK` — `text/html` (minijinja `templates/singlethread.html`)
- Errors: `500` via `WebError` (template render failure)

---

### GET /dump/{key}

Returns all stored entries for `key`, in insertion order.

- Response: `200 OK` — `application/json`, array of `{ "id": i64, "body": <any JSON> }`; empty array (`[]`) for unknown keys
- Errors: `500` via `WebError` (database failure)

---

### POST /dump/{key}

Stores a JSON payload under `key`. Entries accumulate per key.

- Request body: arbitrary JSON (`application/json`)
- Response: `201 Created`
- Errors: `400` for malformed JSON (axum's built-in `Json` extractor response)

---

### GET /health

Health check. Runs `SELECT 1` against the database pool; returns `200` when
the data layer responds, `500` (via the standard error path) otherwise.

---

### GET /static/{file}

Serves files from the `static/` directory (tower-http `ServeDir`).

- Response: `200 OK` — file contents with inferred content type
- Errors: `404` for missing files

---
