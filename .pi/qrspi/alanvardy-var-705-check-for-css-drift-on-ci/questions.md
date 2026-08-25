# Research Questions

## Context

Explore how the compiled CSS artifact relates to its source, how it is built
locally and in production, and how the repository's CI workflows are
organized. Focus on the CSS build pipeline, the scripts that invoke it, and
the GitHub Actions workflows — especially whether any source-vs-output
consistency check currently exists and whether it runs inside or outside CI.

## Questions

1. What is the relationship between the source CSS (`css/site.css`) and the
   compiled artifact (`static/site.css`)? Which is committed to git, which is
   served, and how does the Tailwind v4 build transform source into output?

2. Trace the build script (`scripts/build-css.sh`): what exact command, CLI
   version, and flags it runs, how it pins/verifies the toolchain, and what
   input/output paths it produces.

3. Where does the repository currently perform any source-vs-output
   consistency check (a "drift" check)? Look at `scripts/test.sh`, other
   scripts, and every GitHub Actions workflow under `.github/workflows/` —
   does the check exist, and in which execution contexts does it actually
   run vs not run?

4. How is CSS rebuilt in production (`Dockerfile`)? Does the production build
   regenerate the artifact from source, and how does that relate to the
   committed `static/site.css`?

5. What are the runner environments and tooling available in the existing CI
   workflows (operating system, pre-installed tools, cache/caching keyed on
   what)? How do workflows install or fetch external tools today?

6. What are the trigger conditions and job organization of the CI workflows?
   Which workflows run on pull requests versus pushes to `main`, how are
   jobs/concurrency configured, and how styling/format/fmt jobs interact?