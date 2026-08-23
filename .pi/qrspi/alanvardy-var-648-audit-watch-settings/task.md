# Task — Audit watch settings (VAR-648)

Go through every setting exposed in the SingleThread phone app's settings screen
and verify that each one propagates to the watch app when it is changed. Where a
setting currently fails to propagate (or only takes effect on the watch after a
relaunch), fix it so changes are reflected on the watch promptly and reliably,
with unit and UI test coverage following the repo's testing requirements.

Linear ticket: https://linear.app/vardy/issue/VAR-648/audit-watch-settings
App codebase: /Users/vardy/dev/SingleThread (design artifacts for this ticket
live in this vardy-repo branch / PR #28).
