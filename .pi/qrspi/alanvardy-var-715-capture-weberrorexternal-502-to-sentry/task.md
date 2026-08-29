# Task: Capture WebError::External (502) to Sentry (VAR-715)

The WebError::External arm of the app's centralized error path currently
logs a 502 (e.g. a Resend outage stopping contact-form delivery) with only
`tracing::error!`, so it never alerts. Because `External` wraps a `String`
rather than an error type, `sentry::capture_error` doesn't apply directly.
Capture 502s to Sentry the way the `Database` and `Template` arms already
capture errors, without changing existing Sentry behavior for those arms.