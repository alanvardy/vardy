# Research Questions

## Context

This is a small axum web application that renders HTML through minijinja
templates. Relevant areas: route registration and handler organization under
`src/interfaces/`, the template layer (`templates/layout.html`,
`templates/home.html`, `src/app/templates.rs`), error handling in
`src/app/error.rs`, and the HTTP test helpers in `src/test/mod.rs`.

## Questions

1. How are HTTP routes registered and dispatched today? Trace the full flow
   from a request path to rendered HTML: where routes are declared, how
   handlers are organized into modules, and what state is passed to them.
   What pattern would a second path at a different URL follow?

2. How does the template system work end to end? Describe how templates are
   loaded (loader configuration, auto-escape rules), how layout inheritance
   and blocks are used to share page chrome across pages, and which blocks a
   new page template would need to define.

3. How are static assets such as images or icons handled by this codebase?
   Is there any static-file serving middleware, asset directory, or existing
   pattern for embedding images (inline SVG, data URIs, or otherwise) in the
   source tree or dependencies? What do current pages reference for styling
   and imagery?

4. What markup structure and CSS conventions does the shared layout use?
   Describe the existing CSS variables, container/card classes, and where a
   persistent element like a top-of-page navigation bar with links between
   pages would fit relative to the current block structure.

5. How are handlers tested, and how does error handling work for requests
   that don't match a route or fail template rendering? Describe the
   `start_app`/`test_client` helpers, the assertions made about rendered
   output, the `WebError` variants and their HTTP mappings, and what happens
   today when an unknown path is requested.
