# Task — VAR-681: Move database

Relocate the local SQLite database file from `data/vardy.db` to `test.db` at
the repository root, matching the convention used by the sibling `../api`
repository, so that scripts can reference it at the same relative location.
Update every reference to the old location (env files, Dockerfile, docs,
gitignore) accordingly.

Linear ticket: https://linear.app/vardy/issue/VAR-681/move-database
