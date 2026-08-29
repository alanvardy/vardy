# Task — Server-side validation for contact form fields

The contact form POST handler currently accepts any `name`, `email`, and
`message` values (the browser's HTML `required` attribute is client-side
only) and forwards them straight to the Resend API — empty fields and
multi-megabyte payloads are sent unchecked. Add server-side validation to
the handler: reject empty fields and enforce reasonable max lengths, run
validation before the Resend call, and add an integration test asserting
rejection on empty/missing fields.