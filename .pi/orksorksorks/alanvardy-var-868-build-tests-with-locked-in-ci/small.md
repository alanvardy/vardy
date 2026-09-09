# Task

Update the CI workflow to run tests with `--locked` to ensure Cargo.lock is respected during test execution.

## Why SMALL

Single file change (`.github/workflows/ci.yml`) with two localized edits; no schema/API/UI touches; no unknowns or design decisions needed.

## Key files

- `.github/workflows/ci.yml` (PR test step and main coverage step)
