#!/usr/bin/env bash
# Load DATABASE_URL from .env.
set -a; source .env; set +a

echo "🎨  FORMAT" &&
cargo fmt --all &&
echo "📦  UPDATE MIGRATIONS" &&
cargo sqlx prepare -- --tests &&
echo "🔍  CHECK" &&
cargo check --all-targets &&
echo "🎨  BUILD CSS" &&
./scripts/build-css.sh &&
echo "🧭  CSS DRIFT CHECK" &&
# Committed artifact must match source; otherwise someone edited one side only
git diff --exit-code -- static/site.css &&
echo "📎  CLIPPY" &&
cargo clippy --all-targets --all-features --locked -- -D warnings &&
echo "🧪  TEST" &&
cargo nextest run &&
echo "🔎  FORGOTTEN TODOS" &&
# Requires ripgrep; invert so no-match (clean) continues the chain
! rg -i -s -g '*.rs' 'FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO\s|todo\s' src &&
echo "" &&
echo "🎉  SUCCESS"
