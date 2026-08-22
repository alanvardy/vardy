# Task: Add Sentry (VAR-675)

Integrate Sentry error monitoring into the vardy web app, mirroring how the
sibling project `../api` already does it: a DSN/enable flag from environment
variables, client initialization at startup with hardened panic hooks, and
error reporting wired into the app's error path. Deployment (Docker/Fly.io)
must be able to supply the new configuration.
