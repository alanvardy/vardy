#!/usr/bin/env bash
# Load .env if present; otherwise fall back to the .env_template defaults.
if [ -f .env ]; then
    set -a; source .env; set +a
else
    export DATABASE_URL="${DATABASE_URL:-sqlite:data/vardy.db}"
fi

echo "🎨  FORMAT" &&
cargo fmt --all &&
echo "📦  UPDATE MIGRATIONS" &&
cargo sqlx prepare -- --tests &&
echo "🔍  CHECK" &&
cargo check --all-targets &&
echo "📎  CLIPPY" &&
cargo clippy --all-targets --all-features --locked -- -D warnings &&
echo "🧪  TEST" &&
cargo nextest run &&
echo "🔎  FORGOTTEN TODOS" &&
# Requires ripgrep
if rg -i -s -g '*.rs' 'FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO\s|todo\s' src; then
    exit 1
fi
echo "" &&
echo "🎉  SUCCESS"
