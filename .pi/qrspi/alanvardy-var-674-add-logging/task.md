# Task: Add logging (VAR-674)

Add structured logging to the vardy web app so that application events and
HTTP requests are logged in a leveled, configurable way — matching the
approach already used in the sibling `../api` project (`tracing` +
`tracing-subscriber`, request tracing via tower-http `TraceLayer`, JSON
output for Fly.io log capture).

Scope includes: subscriber initialization at startup, per-request HTTP
logging, replacing ad-hoc `println!`/`eprintln!` diagnostics in error
handling, and making sure the test harness still works.
