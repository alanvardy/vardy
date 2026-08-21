# Task: Add SQLite (VAR-653)

Add SQLite persistence to the `vardy` web service. The repo currently has no
database layer at all — `AppState` holds only a minijinja template
environment — so this introduces the first persistence dependency, connection
handling, migrations, and configuration.

The sibling repository at `../api` previously used SQLite before migrating to
PostgreSQL (commit `4fb273f` "feat: switch from SQLite to PostgreSQL"); its
git history is intended as a reference for how SQLite was configured there.
