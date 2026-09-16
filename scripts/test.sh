#!/usr/bin/env bash
# Load DATABASE_URL from .env.
set -a; source .env; set +a

# Reclaim age-expired build/test caches before building. No-op when the shared
# helper is not installed, so CI and other machines are unaffected.
command -v disk-clean >/dev/null 2>&1 && disk-clean || true

echo "🎨  FORMAT" &&
cargo fmt --all &&
echo "📦  UPDATE MIGRATIONS" &&
cargo sqlx prepare -- --tests &&
echo "🔍  CHECK" &&
cargo check --all-targets &&
echo "🎨  BUILD CSS" &&
./scripts/build-css.sh &&
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
