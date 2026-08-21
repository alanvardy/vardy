# Task: Handle CSS cache invalidation for /static/site.css (VAR-670)

The homepage redesign (VAR-664) extracts all CSS from the inline `<style>`
block in `templates/layout.html` into an external `static/site.css`, served
via `ServeDir::new("static")` at `/static`. Once the stylesheet is external,
browsers may cache it, so styling changes might not appear for returning
visitors without a hard refresh.

This task decides on and implements a simple cache-invalidation strategy —
e.g. a version query param in templates (`/static/site.css?v=1`) bumped on
change, or explicit `Cache-Control` headers on static responses — keeping in
mind the ticket's guidance to keep it simple and only act if it becomes a
real problem.
