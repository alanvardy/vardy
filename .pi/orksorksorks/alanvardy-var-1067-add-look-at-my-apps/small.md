# Task

Add a "Look at my apps" section to the homepage (`templates/home.html`), listing
the site's apps, positioned directly under the "Check out my LinkedIn" entry.

The homepage currently renders a `# You are invited to` heading followed by a
bordered list with two links: **Take a look at my work on GitHub** and
**Check out my LinkedIn**. Add a new section — with a heading like
"Look at my apps" (match the existing `h2`/list styling) — that lists the apps
so they appear after the LinkedIn item. The apps to list are the two the site
ships with and already routes to: **SingleThread** (`/singlethread`) and
**CheckStitch** (`/checkstitch`) — `src/interfaces/routes.rs` and the nav confirm
these are the app pages, and each has an icon in `static/`
(`singlethread-icon.png`, `checkstitch-icon.png`).

Concrete scope:
- Edit `templates/home.html` only — add the "Look at my apps" heading and
  list items linking to `/singlethread` and `/checkstitch`, mirroring the
  existing GitHub/LinkedIn list-item markup (`flex items-center gap-2 py-2`,
  icon `{{ asset_url(...) }}`, `target="_blank" rel="noopener noreferrer"`).
  Place it under the LinkedIn entry per the ticket ("Put them under the
  'Check out my linkedin'").
- Update the homepage tests in `src/interfaces/handlers/home/web.rs`
  (`index_serves_ok_html`): add assertions for the new heading and links
  (e.g. `Look at my apps`, `href="/singlethread"`, `href="/checkstitch"`),
  and assert the icons are versioned (`/static/singlethread-icon.png?v=`,
  `/static/checkstitch-icon.png?v=`).
- No route changes: `/singlethread` and `/checkstitch` already exist.
- If any new Tailwind classes are added to the template, regenerate and commit
  `static/site.css` in the same change (check whether `.py-2`, `gap-2` etc. are
  already generated; otherwise run the project's CSS pipeline).

## Why SMALL
Localized one-module UI addition (home template + its test file, ≤2 files),
follows the existing GitHub/LinkedIn list pattern, no schema/API/shared-code
changes, no design decision — the apps list is fixed by the existing routes.

## Key files
- `templates/home.html` — add the "Look at my apps" section.
- `src/interfaces/handlers/home/web.rs` — extend `index_serves_ok_html` assertions.
- Possibly `static/site.css` — regenerate if new Tailwind classes are used.