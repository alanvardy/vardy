# Research Questions

## Context
This is a binary-only Rust crate (`vardy`) whose source is organized into a
few top-level modules under `src/`: `app`, `domain`, `infra`,
`interfaces`, and a shared `test` helper module. A sibling project in
`../api` contains a test file `src/test/arkitect.rs` that uses the
`rust_arkitect` crate. The build/test tooling lives in `Cargo.toml` and
`scripts/test.sh`.

## Questions
1. How is the module tree of this crate declared and wired — which modules
   exist under `src/`, how are they registered in `main.rs` and `mod.rs`
   files, what logical path prefix does the crate compile under (crate
   name, presence/absence of `lib.rs`), and how does the shared test module
   fit into the tree?
2. What external crates does each top-level module (`src/app`, `src/infra`,
   `src/interfaces`, `src/main.rs`, `src/test`) import, and which of those
   imports occur in production code versus inside `#[cfg(test)]` modules or
   `#[cfg(test)]`-gated items? Cite file:line for each notable case,
   especially any usages that cross expected layer boundaries.
3. What intra-crate dependencies exist between the top-level modules —
   which `crate::…` paths does each module reference from the others (e.g.
   does `app` reference `interfaces`, does `interfaces` reference `app` or
   `infra`, does anything reference `domain`)? Map the current direction of
   dependencies between every pair of top-level modules with file:line
   references.
4. What are the established testing conventions in this crate — inline
   `#[cfg(test)]` modules versus integration-style tests through
   `src/test/mod.rs`, what `[dev-dependencies]` are declared, how new test
   files get registered so they actually run, and what steps
   `scripts/test.sh` executes?
5. How does the reference implementation in `../api/src/test/arkitect.rs`
   work — which `rust_arkitect` APIs it uses (`Arkitect`, `Project`,
   `ArchitecturalRules`, the `Rule` trait), how its rules are defined per
   module, how its custom rule excludes dependencies inside
   `#[cfg(test)]` code (AST walking, visitor, alias resolution), and what
   version/features of `rust_arkitect` would be required? Also note what
   the current stable published version of `rust_arkitect` is on
   crates.io and whether the APIs used in the reference file match it.
6. What existing enforcement mechanisms or quality gates does this repo
   already have — compiler lints in `Cargo.toml` ([lints] section),
   clippy configuration, CI workflows, or anything else beyond
   `scripts/test.sh` — and where would an additional automated check fit
   without duplicating them?

7. Where exactly does production code in this crate touch databases and
   HTTP clients — list every file using `sqlx` outside `#[cfg(test)]`
   code, every file performing outbound HTTP (e.g. `reqwest`) calls, and
   which module each belongs to, so layer ownership of I/O can be stated
   factually.
