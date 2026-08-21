# Task: Add a health endpoint (VAR-665)

Add a health check endpoint to the web application so that uptime/monitoring
systems can verify the service is responding. The sibling project `../api`
already implements an equivalent endpoint (`/health_check` returning HTTP 200)
and serves as the reference implementation. The work happens on branch
`alanvardy-var-665-add-a-health-endpoint`; a PR (#9) is already attached to
the Linear ticket.
