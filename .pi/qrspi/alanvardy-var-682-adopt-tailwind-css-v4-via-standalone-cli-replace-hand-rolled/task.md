# Task: Adopt Tailwind CSS v4 via standalone CLI (replace hand-rolled site.css)

Replace the hand-rolled `static/site.css` with Tailwind CSS v4, built via the
pinned standalone CLI binary (no Node/npm). Templates are migrated to Tailwind
utility classes while preserving the current visual design (dark theme,
responsive behavior). The build output must continue to be served as
`static/site.css` so asset fingerprinting (`asset_url`) and existing tests keep
working. Build step is added to the Dockerfile builder stage and local dev
docs; a project rule for AI assistants records Tailwind v4 CSS-first conventions.

Linear ticket: VAR-682. Supersedes the earlier "no asset build step" decision
recorded in `.pi/qrspi/alanvardy-var-664-make-homepage-more-attractive/design.md`.
