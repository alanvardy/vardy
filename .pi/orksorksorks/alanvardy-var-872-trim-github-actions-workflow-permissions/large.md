# Task

Trim GitHub Actions workflow permissions by moving them to job level, auditing minimum required permissions across ci.yml, fly-deploy.yml, and dependabot_auto_merge.yml, and ensuring security best practices are followed.

## Why LARGE

**CONVENTION_RISK** + **MULTI_MODULE** — Touches shared CI/CD infrastructure across multiple workflow files with permission auditing that spans different deployment scenarios, requiring the full research → design → plan → implement pipeline.
