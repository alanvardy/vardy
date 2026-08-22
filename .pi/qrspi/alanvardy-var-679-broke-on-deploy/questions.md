# Research Questions

## Context
Focus on the deployment pipeline and infrastructure configuration: the `.github/workflows/` directory, `Dockerfile`, `fly.toml`, application startup code, and how the app serves health/metrics endpoints.

## Questions
1. Trace what happens end-to-end when a commit lands on the main branch: which GitHub Actions workflows trigger, in what order, and does the deploy workflow have any dependency on CI tests passing first?
2. What exactly happens during the Docker image build in the `Dockerfile`, particularly around database creation and migration execution — at what point do these run relative to the new container being started?
3. How is persistent storage configured for this app (`fly.toml` mounts/volumes, `DATABASE_URL`, anything under `data/`), and what happens to files written at runtime between deployments?
4. How does application startup behave in `src/main.rs` and `src/app/`: what gets initialized and in what order (database, migrations, templates, listeners), and how would an initialization failure surface?
5. What HTTP endpoints exist for health checks and metrics (e.g. `/health`, `/metrics` handlers), what do they actually verify, and how do they relate to the health check configuration in `fly.toml`?
6. What error handling, logging, and monitoring exist in the app (e.g. `src/app/error.rs`, Sentry integration) that would reveal whether and why a deployed release failed?
