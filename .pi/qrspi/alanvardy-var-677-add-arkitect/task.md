# Task — VAR-677: Add arkitect

Add [rust_arkitect](https://github.com/samuelcolvin/rust_arkitect)-based
architectural boundary enforcement to the `vardy` crate, so that layering
rules (e.g. app ↔ interfaces ↔ infra/domain dependency constraints) are
checked automatically as part of the test suite.

A working reference implementation exists in a sibling project at
`../api/src/test/arkitect.rs` (Linear ticket VAR-677 links PR #19 of
`alanvardy/vardy` as an attachment). The goal is to port/adapt that approach:
define rules for the crate's modules, allow-list the dependencies each layer
may use, skip dependencies inside `#[cfg(test)]` code, and make violations
fail the build via a test.
