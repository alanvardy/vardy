# Task

Fix a production Sentry error where the contact form's email send fails with
Resend HTTP 422:

```
ExternalError: resend returned status 422 Unprocessable Entity:
{"message":"The `\\n` is not allowed in the `subject` field.", ...}
```

The contact form's email subject is built directly from user-supplied `name`
and `email` fields:

```rust
format!("New contact message from {} <{}>", self.name, self.email)
```

If the submitted name (or email) contains a newline/CR or other control
character, that character ends up in the `subject` header and Resend rejects
the whole send. Make the subject line safe so a message with newlines in the
name/email can never produce a 422 — e.g. strip or collapse CR/LF (and other
control characters) from the interpolated values before building the subject,
so the subject is a single clean line. Add unit tests covering a name and an
email containing `\n` (and `\r`) that assert the resulting subject contains no
newline/control characters. Keep the existing subject format for clean input
unchanged. Run `./scripts/test.sh` before finishing.

## Why SMALL

Localized one-module fix (`src/app/contact.rs`) following the existing
form-validation pattern: single file plus its inline tests, no schema/API/UI
change, no new subsystem, approach already known.

## Key files

- `src/app/contact.rs` — `ContactForm::subject()` (builds the subject),
  `body()`, `validate()`, and the inline `#[cfg(test)] mod tests`.
- `src/interfaces/handlers/contact/web.rs` — calls `form.subject()` and
  `contact::send(...)`.
- `src/infra/resend.rs` — Resend client that surfaces the 422 as
  `ExternalError`.
