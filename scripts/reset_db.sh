#!/bin/bash
# Reset the local SQLite test database.
# The database file is gitignored — this removes it so the app re-creates on next boot.
set -euo pipefail

DB_FILE="${1:-test.db}"

if [ -f "${DB_FILE}" ]; then
    echo "Removing ${DB_FILE}..."
    rm "${DB_FILE}"
else
    echo "${DB_FILE} does not exist — nothing to remove."
fi

echo "Done. The database will be re-created on next app boot."