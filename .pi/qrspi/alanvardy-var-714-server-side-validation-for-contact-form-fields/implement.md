# Implementation Summary

Feature: server-side validation for contact form fields (VAR-714).
Plan: `.pi/qrspi/alanvardy-var-714-server-side-validation-for-contact-form-fields/plan.md`

## Commits

| Phase | Commit | Description |
|-------|--------|-------------|
| 1     | `12b9f4e` | WebError::BadRequest variant |
| 2     | `90e8795` | ContactForm::validate() |
| 3     | `2c9b548` | Handler - validate before Resend |
| 4     | `74b2d76` | Template - error banner and preserved input |

All four phases pushed to `origin/alanvardy-var-714-server-side-validation-for-contact-form-fields`.

## Implementation Notes

- `WebError::BadRequest(String)` added with `IntoResponse` arm producing `400` + message body; unit test `bad_request_is_400_with_body` added (`src/app/error.rs`).
- `ContactForm::validate()` added with empty/whitespace checks then length limits (name 200, email 254, message 10_000) and 10 unit tests (`src/app/contact.rs`).
- `render()` helper extended to carry `error`/`name`/`email`/`message`; `create()` validates before calling Resend and re-renders the form with an error banner + preserved input on failure; honeypot and success paths render with empty values. 5 integration tests added (4 re-render + 1 missing-key 422).
- `templates/contact.html`: `{% if error %}` banner block, `value=` attributes on name/email, message body inside textarea.
- `static/site.css` regenerated via `scripts/build-css.sh` for the new Tailwind classes (`bg-red-100`, `border-red-400`, `text-red-700`, `px-4`, `py-3`, `mb-4`) so the CSS-drift gate passes.

## Deviations from the plan (behavior-preserving compile fixes, Stage 3)

1. `form._website.is_some_and(|w| ...)` → `form._website.as_ref().is_some_and(...)`: the plan's version partially moved `_website`, then `form.validate()` borrows the whole struct (E0382). Honeypot semantics identical.
2. `("name", &long_name)` → `long_name.as_str()` for a uniform `&str` tuple array in `post_over_length_name_returns_200_and_skips_email` (E0308).

## Automated Checks

- [x] `cargo nextest run` passes (Stage 1; 93/93 green)
- [x] `cargo nextest run` passes (Stage 2; 103/103 green)
- [x] `./scripts/test.sh` passes — full gate: fmt, sqlx offline refresh, check, CSS drift, clippy, nextest, no TODOs (Stage 3; 108/108 green)
- [x] `post_valid_form_sends_email` continues to pass (regression — existing behavior unchanged)
- [x] `./scripts/test.sh` passes — full gate incl. CSS drift (Stage 4; 108/108 green)
- [x] CSS regenerated (`static/site.css` committed with the new Tailwind classes)

## Manual Verification Items (from the plan)

No manual verification needed — every stage's manual items are marked "N/A — (unit/integration) tests are self-verifying" in `plan.md`. Confirmed by the user before merge.