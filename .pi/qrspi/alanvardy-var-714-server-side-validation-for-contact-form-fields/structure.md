# Structure Outline

## Approach

Add server-side validation to the contact form by extending `WebError` with a `BadRequest` variant, adding a `validate()` method on `ContactForm`, and re-rendering the form with an error banner + preserved input on failure — all before the Resend call. Built bottom-up in four horizontal layers, each tested green before the next begins.

---

## Stage 1: `WebError::BadRequest` variant

Add a 400-family variant to the shared error type so the handler has a chokepoint to return validation failures that produce HTML (via `IntoResponse`) rather than axum's default plain-text rejections.

**Files**: `src/app/error.rs`

**Key changes**:
- `WebError::BadRequest(String)` — new enum variant
- `IntoResponse for WebError` — new match arm: `BadRequest(msg) → 400 + msg` as plain text (no Sentry, consistent with `TooManyRequests` / client-fault pattern at `error.rs:60` comment)

**Tests** (inline, `#[cfg(test)] mod tests`):
- `bad_request_is_400_with_body` — construct `WebError::BadRequest("msg".into())`, assert status 400, body `"msg"`

**Verify**: `cargo nextest run` passes; `cargo clippy --all-targets --all-features --locked -- -D warnings` clean.

---

## Stage 2: `ContactForm::validate()`

Pure validation logic on the form struct — no I/O, no state, trivially unit-testable. This is the layer the handler will call before touhing Resend.

**Files**: `src/app/contact.rs`

**Key changes**:
- `impl ContactForm { pub fn validate(&self) -> Result<(), String>` — new method
- Validation rules (per design decision 3):
  - `name.trim().is_empty()` → `Err("Please enter your name.".into())`
  - `email.trim().is_empty()` → `Err("Please enter your email address.".into())`
  - `message.trim().is_empty()` → `Err("Please enter a message.".into())`
  - `name.len() > 200` → `Err("Name must be 200 characters or fewer.".into())`
  - `email.len() > 254` → `Err("Email must be 254 characters or fewer.".into())`
  - `message.len() > 10_000` → `Err("Message must be 10,000 characters or fewer.".into())`
  - Checks in order: empty first, then length. First failure wins.

**Tests** (inline, `#[cfg(test)] mod tests`):
- `valid_form_passes_validation` — all fields non-empty, within limits → `Ok(())`
- `empty_name_rejected` — `name = ""` → `Err(_)`
- `whitespace_only_name_rejected` — `name = "   "` → `Err(_)`
- `empty_email_rejected`
- `empty_message_rejected`
- `name_too_long_rejected` — 201-char name → `Err(_)`
- `name_at_boundary_accepted` — 200-char name → `Ok(())`
- `email_too_long_rejected` — 255-char email → `Err(_)`
- `message_too_long_rejected` — 10,001-char message → `Err(_)`
- `message_at_boundary_accepted` — 10,000-char message → `Ok(())`

**Verify**: `cargo nextest run` passes (only the new unit tests need green; existing tests unchanged).

---

## Stage 3: Handler — validate before Resend, re-render on failure

Wire validation into the `create` handler. On failure, call a modified `render()` that accepts an optional error message and preserved field values. The handler returns 200 with the form re-rendered — Resend is never called. The template hasn't been updated yet (Stage 4), so no error banner is visible, but the handler logic is fully testable: bad input → 200 (not 502), call_count ==0.

**Files**: `src/interfaces/handlers/contact/web.rs`

**Key changes**:
- `async fn render(state: &AppState, submitted: bool, error: Option<&str>, name: &str, email: &str, message: &str) → Result<Html<String>, WebError>` — extended signature
  - New template context vars: `error`, `name`, `email`, `message` (all passed to minijinja)
  - Backward-compatible: GET handler calls `render(&state, false, None, "", "", "")` — same behavior
  - Honeypot submit path: `render(&state, true, None, "", "", "")` — same behavior
  - Successful POST: `render(&state, true, None, "", "", "")` — same behavior
- `create()` handler:
  - After honeypot chek but **before** subject/text formatting and `contact::send()`:
    ```rust
    if let Err(msg) = form.validate() {
        return render(&state, false, Some(&msg), &form.name, &form.email, &form.message).await;
    }
    ```
  - This is a `let`-bind + early return, not `?` — `validate()` returns `Result<(), String>`, not `Result<_, WebError>`. (The `?`-to-`WebError::BadRequest` pattern from design decision 6 is re-evaluated here: `render()` already returns `Result<_, WebError>` and we want200, not 400, so we call `render()` directly rather than propagating a `BadRequest`.)

**Tests** (inline, `#[cfg(test)] mod tests`):
- `post_empty_name_returns_200_and_skips_email` — POST `("name",""),("email","a@b.c"),("message","hi")` → 200, `call_count == 0`, body contains `<form` (not thank-you)
- `post_empty_email_returns_200_and_skips_email` — same pattern
- `post_empty_message_returns_200_and_skips_email` — same pattern
- `post_over_length_name_returns_200_and_skips_email` — 201-char name →200, call_count ==0
- `post_missing_name_key_returns_422` — POST only `("email","a@b.c"),("message","hi")` → 422 (axum default, document behavior)
- `post_valid_form_still_sends_email` — existing test must continue to pass (regression)

**Verify**: `./scripts/test.sh` passes (full gate). New integration tests green; existing tests unmodified and green.

---

## Stage 4: Template — error banner + preserved input

Add the error banner and `value=` attributes to `contact.html`. Now that the handler passes `error`, `name`, `email`, `message` context vars, the template renders them.

**Files**: `templates/contact.html`

**Key changes**:
- Error banner block inside the `{% else %}` branch, **above** the `<form>` tag:
  ```html
  {% if error %}
  <div class="bg-red-100 border border-red-400 text-red-700 px-4 py-3 rounded mb-4" role="alert">
    {{ error }}
  </div>
  {% endif %}
  ```
- `value=` attributes on visble fields:
  - `<input id="name" ... value="{{ name }}">`
  - `<input id="email" ... value="{{ email }}">`
  - `<textarea id="message" ...>{{ message }}</textarea>`
  - Honeypot `_website` keeps static `value=""` (unchanged)
- No `value=` on the get-request path: when `error` is `None` and name/email/message are empty strings, the fields render with empty `value=""` — identical to the current no-value behaior.

**Tests**: Extend Stage 3's validation-failure tests with additional assertions:
- `post_empty_name_returns_200_and_skips_email` — also assert body contins the error banner class (`bg-red-100`) and the specific error text
- `post_empty_name_returns_200_and_skips_email` — also assert `value="a@b.cc"` preserved on email field and `hi` in textarea
- Likewise for all validation-failure tests

**Verify**: `./scripts/test.sh` passes (inludes CSS drift check via `git diff --exit-code -- static/site.css`). If the new Tailwind classes (`bg-red-100`, `boder-red-400`, `text-red-700`, `px-4`, `py-3`, `rounded`, `mb-4`) aren't in the committed CSS, the check fails — re-run `./scripts/build-css.sh`.

---

## Testing Checkpoints

| Checkpoint | What must be green |
|---|---|
| After Stage 1 | `caro nextest run` (unit test for `BadRequest` variant) |
| After Stage 2 | `caro nextest run` (unit tests for `validate()`) |
| After Stage 3 | `./scripts/test.sh` (full gate: fmt, sqlx, check, CSS drift, clippy, nextest, no TODOs) |
| After Stage 4 | `./scripts/test.sh` (full gate, including CSS drift) |

---

## Layer Dependencies

```
Stage 1 (WebError::BadRequest) ── no deps, pure addition
    │
Stage 2 (ContactForm::validate()) ── no deps on Stage 1, pure logic
    │
Stage 3 (Handler) ─────────────── depends on Stage 1 + 2
    │
Stage 4 (Template) ──────────── depends on Stage 3 (handler passes context vars)
```

Stage2 could be built before Stage1 (they're independent), but the order above follows the codebase layering: `app/error.rs` → `app/contact.rs` → `interaces/handlers/contact/web.rs` → `templates/`.

---

## Notes

- **No `From` impl for `BadRequest`** — the handler calls `render()` directly on validation failure (producing200 HTML), so there's no `Result<_, WebError>` propagation path for validation errors. The `BadRequest` variant is available for futue handlers that need a plain 400 text response but isn't used in this feature's happy path.
- **Single error message** — `validate()` returns one `String`, not per-field errors. The template's `{% if error %}` block shows it as a banner. Consistent with design decision 7 (no per-field errors).
- **No new middleware or extractors** — the `Form<ContactForm>` extractor stays; missing-key rejection remaains axum's default 400/422 (tested in Stage 3).