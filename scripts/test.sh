#!/usr/bin/env bash
# ── Test-gate lock ────────────────────────────────────────────────────────────────
# Serialize against other pi instances running the heavy test gate on this machine
# (they share one OS user). Two+ concurrent build+test gates can exhaust RAM (OOM).
# Acquires an exclusive flock(2) on a shared lock file via /usr/bin/lockf and waits
# (up to PI_TEST_LOCK_TIMEOUT seconds) for it; the lock is held for the whole run
# and released automatically on exit, even if this process is killed. The lock is
# never broken while held by another process. All projects should use the same
# default path so their gates serialize against each other. Bypass with
# PI_TEST_NO_LOCK=1, or automatically under CI (each CI job is its own runner).
if [[ -z "${PI_TEST_LOCK_HELD:-}" && -z "${PI_TEST_NO_LOCK:-}" && -z "${CI:-}" ]]; then
    PI_TEST_LOCK="${PI_TEST_LOCK:-$HOME/.cache/pi/test-gate.lock}"
    PI_TEST_LOCK_TIMEOUT="${PI_TEST_LOCK_TIMEOUT:-3600}"
    export PI_TEST_LOCK_HELD=1
    echo "==> Waiting for test-gate lock ($PI_TEST_LOCK)…"
    exec /usr/bin/lockf -k -t "$PI_TEST_LOCK_TIMEOUT" "$PI_TEST_LOCK" bash "$0" "$@"
fi
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
