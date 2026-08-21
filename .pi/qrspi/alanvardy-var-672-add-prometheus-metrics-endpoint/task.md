# Task — VAR-672: Add Prometheus metrics endpoint

Add a `/metrics` endpoint that exposes application and runtime metrics in
Prometheus text exposition format so the deployed service (currently on Fly.io)
can be scraped by a Prometheus-compatible monitoring setup. This should follow
the existing patterns for routing, handlers, and testing in this axum-based
Rust project.
