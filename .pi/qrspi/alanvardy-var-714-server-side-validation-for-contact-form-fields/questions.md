# Research Questions

## Context

Explore how the contact form page works end-to-end: the POST request flow
from routing through to the external email API, how errors are represented
over HTTP, how templates are rendered and what state they receive, what
constraints apply to incoming request bodies, and how the existing handlers
are tested. Focus on the contact handler (`src/interfaces/handlers/contact/`),
the shared error and templates modules (`src/app/`), the routes file, and the
test harness (`src/test/`).

## Questions

1. Trace the complete request flow for a POST to `/contact`: which router and
   middleware layers it passes through, how the form body is extracted and
   deserialized, what checks the handler performs before sending, how the
   email is constructed and delivered (including the Resend API call), and
   what response each code path returns.

2. How are errors represented and mapped to HTTP responses? Examine
   `WebError` and its `IntoResponse` impl (`src/app/error.rs`), including how
   client-fault (4xx) cases are currently expressed — both through the shared
   error type and through built-in axum rejections (e.g. malformed JSON) —
   and how the rate limiter routes its errors into the same chokepoint.

3. How are pages rendered from templates? Look at the minijinja setup in
   `src/app/templates.rs`, the `render()` helper used by the contact handler,
   and the structure of `templates/contact.html` (what context variables it
   receives, how the form fields and `submitted` states are handled). Also
   check whether any handler in the codebase re-renders a form with preserved
   input values or error state, or otherwise responds to form submissions
   with anything other than a fresh page render.

4. What constraints apply to incoming request bodies and their content?
   Establish the effective body-size limit for `Form` extractors given the
   axum router setup (`src/interfaces/routes.rs`, `src/main.rs`) and whether
   any custom extractors or body-limit configuration exist. Also check what
   dependency constraints the architecture guard (`src/test/arkitect.rs`)
   places on the handler (`interfaces`) layer.

5. How are form-POST handlers tested? Survey the test harness in
   `src/test/mod.rs` (the `start_app*` variants, the Resend stub, and how
   request bodies are asserted), the existing tests for the contact handler
   and the closest POST precedent (`src/interfaces/handlers/dump/web.rs`),
   and which checks `scripts/test.sh` runs as the gate.