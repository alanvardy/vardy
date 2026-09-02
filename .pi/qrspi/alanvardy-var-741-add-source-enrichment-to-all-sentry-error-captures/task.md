# task.md — VAR-741: Add source enrichment to all Sentry error captures

Every Sentry error capture (currently the `Database`, `Template`, and
`External` arms of `WebError::IntoResponse`, plus any future arms) should
attach enrichment tags — specifically a `source` tag identifying which
error category triggered the capture.

Enrichment must be centralized: all capture sites call one public function
provided in `src/infra/sentry.rs`, rather than attaching tags per call
site. This is follow-up work identified during the design of VAR-715
("Capture WebError::External to Sentry").