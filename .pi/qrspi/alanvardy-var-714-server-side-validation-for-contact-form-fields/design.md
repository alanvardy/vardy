# Design Discussion

Branch: `alanvardy-var-714-server-side-validation-for-contact-form-fields`

## Current State

The contact form POST handler (`src/interfaces/handlers/contact/web.rs:34-51`) accepts
any `name`, `email`, and `message` values as plain `String`s — no validation of any
kind beyond deserialization itself (`src/app/contact.rs:4-13`). The only check before
sending email is the honeypot `_website` field (`web.rs:40-41`). Empty fields and
multi-megabyte payloads pass straight through to the Resend API.

The error type (`src/app/error.rs:10-16`) has no 400-family variant — only
`TooManyRequests` (429), `External` (502), `Database`/`Template` (500), and dead
`NotFound` (404). A missing-field `Form` rejection bypasses `WebError` entirely and
returns axum's default plain-text 400/422 (Q2 research, vendored axum 0.8.9).

The form template (`templates/contact.html:19-41`) has exactly two states: show form
(`submitted=false`) or show thank-you (`submitted=true`). No error state, no preserved
input values, no `value=` attributes on any visible field.

Tests (`web.rs:55-157`) exercise the happy path, honeypot, Resend failure, and rate
limiting — but no test posts malformed or missing-field forms. The Resend stub
(`src/test/mod.rs:243-272`) records call count only; email body content is unobservable.

No explicit body-size limit exists (axum default: 2 MiB, Q4 research).

## Desired End State

After a POST to `/contact`:

1. **Missing fields** (key absent from POST body) → axum's built-in `Form` rejection
   returns default 400/422 (unchanged behavior, now tested).

2. **Empty fields** (key present, value `""` or whitespace-only) → handler validates
   before Resend call, returns 200 re-rendering the form with an error banner and
   preserved input values.

3. **Over-length fields** (name > 200, email > 254, message > 10,000 chars) → same
   re-render path with an error banner.

4. **Valid fields** → unchanged behavior: email sent via Resend, thank-you page
   rendered.

5. Validation runs **before** the Resend call — no empty or over-length payloads
   reach the external API.

Verification: existing tests continue to pass; new integration tests assert:
- Empty `name` → 200 + error banner visible + Resend never called
- Empty `email` → same
- Empty `message` → same
- Over-length fields → same
- Missing `name` key → 400/422 from axum (document current behavior)
- Valid form still sends email (regression)

## Patterns to Follow

- **Error chokepoint** (`src/app/error.rs:42-68`): every handler error flows through
  `IntoResponse for WebError`. New `BadRequest` variant follows the `External(String)`
  pattern (single string payload, maps to a status code, no Sentry for client faults
  per comment at `error.rs:60`).

- **Typed extractor** (`src/interfaces/handlers/contact/web.rs:36`): keep
  `Form<ContactForm>` — consistent with the handler's existing signature and the
  `Json<Value>` pattern in dump (`src/interfaces/handlers/dump/web.rs:21`). Do not
  switch to `HashMap` or manual `Bytes` extraction.

- **Handler-local render helper** (`src/interfaces/handlers/contact/web.rs:17-27`):
  extend the existing `render()` function rather than creating a second render path.
  The codebase has no shared engine-level render helper (Q3 research).

- **Test harness** (`src/test/mod.rs:42-45`): use `start_app_with_resend` for tests
  that need to assert "email not sent" (call count = 0). Follow the existing pattern
  of `.form(&[...])` + status + body assertions (`web.rs:92-105`).

- **`#[cfg(test)] mod tests`** at bottom of source file (`web.rs:55`): new tests go
  inline, not in separate files (project instructions).

- **Gate**: `./scripts/test.sh` runs `cargo nextest run` — all tests must pass
  including `test_architectural_rules` in `src/test/arkitect.rs`.

### Patterns to NOT follow

- **Do not add a `HandleErrorLayer` or custom rejection handler** — adds a new
  middleware pattern for marginal gain when axum's defaults are adequate for the
  missing-field case.

- **Do not add a validation library or `serde` in the interfaces layer** — `serde` is
  not in the interfaces whitelist (`src/test/arkitect.rs:28-41`), and the validation
  logic is simple enough for hand-rolled checks.

- **Do not add per-field error messages in the template** — no codebase precedent, and
  the task doesn't require it. A single error banner is sufficient.

## Design Decisions

1. **`BadRequest(String)` on `WebError`**: maps to 400, carries a plain message. No
   Sentry (client fault, consistent with `TooManyRequests` pattern at
   `error.rs:60-66`). A `From` impl is unnecessary since handlers construct it
   directly. No `From` impl for `ResendError` etc. needed — this is handler-only.

2. **Validation on `ContactForm`**: a `pub fn validate(&self) -> Result<(), String>`
   method on `ContactForm` in `src/app/contact.rs`. Returns `Ok(())` or
   `Err("message")` with a single human-readable error for the banner. The `app`
   layer is the right home — accessible to the handler, testable in isolation, no
   new dependencies (only `std`).

3. **Validation rules — non-empty + max lengths**: `name.trim().is_empty()` → reject;
   same for `email` and `message`. `name.len() > 200` → reject; `email.len() > 254`;
   `message.len() > 10_000`. Whitespace-only counts as empty. No email format
   validation (not requested, and `serde` isn't in the interfaces whitelist).

4. **Re-render form on validation failure**: handler calls `form.validate()`, and on
   `Err(msg)` calls a modified `render()` that passes `error: Some(msg)` plus the
   original field values. Template gains an error banner div and `value=` attributes
   on the three visible fields. `submitted` remains `false` (the form was not
   successfully submitted). The honeypot keeps its static `value=""`.

5. **Two-level rejection, no new middleware**: missing keys → axum's built-in `Form`
   rejection (400/422, unchanged). Empty/over-length values → handler validation →
   `WebError::BadRequest` → re-render. This keeps the clean extractor pattern and
   adds no new layers.

6. **`From` impl for `WebError::BadRequest` into the handler**: the `create` handler
   catches validation errors explicitly — `form.validate().map_err(WebError::BadRequest)?`
   is the pattern. This is consistent with how `External` is used via `?` on
   `ResendError` (`error.rs:36-39`).

7. **Error banner, not per-field errors**: a single `{% if error %}` block above the
   form in `contact.html`. The `error` template variable is `Option<&str>` (absent
   for GET and successful POST, `Some(msg)` on validation failure). No per-field
   `errors.name` / `errors.email` map — the codebase has zero precedent for
   structured error state in templates (Q3 research).

## What We're NOT Doing

- **Not adding a custom extractor or `HandleErrorLayer`** — axum defaults for missing
  fields are fine.
- **Not adding email format validation** — out of scope for this task.
- **Not adding a body-size limit layer** — the axum default (2 MiB) already prevents
  multi-megabyte payloads. The `message ≤ 10,000` char limit catches the rest.
- **Not changing the Resend stub to record body content** — out of scope.
- **Not adding per-field error rendering** — adds template complexity with no codebase
  precedent.
- **Not modifying the rate limiter or its constants** — out of scope.
- **Not adding a shared `render()` helper to `templates.rs`** — the contact handler's
  private `render()` is sufficient.

## Open Risks

- **axum Form rejection body format**: the exact status code (400 vs 422) and body
  text for missing fields depends on axum 0.8's vendored rejection impl. If axum's
  behavior changes across versions, the new test may need updating. Low risk — axum
  is locked in `Cargo.lock`.
- **Error banner styling**: the template has no existing error/alert CSS classes.
  May need a small Tailwind utility addition — check `static/site.css` drift via
  `scripts/test.sh` (it runs `build-css.sh && git diff`).
- **Whitespace-only honeypot interaction**: the honeypot check at
  `web.rs:40-41` uses `!w.trim().is_empty()`. The new `name`/`email`/`message`
  `trim().is_empty()` check is consistent with this precedent.
- **Re-render vs redirect-after-POST**: re-rendering on validation failure means the
  browser URL stays at `/contact` (POST). A browser refresh would re-POST with the
  same bad data and re-trigger the error — acceptable UX for a simple contact form.