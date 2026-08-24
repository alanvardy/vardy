# Research Findings

Note on source availability: `css/site.css` (the Tailwind v4 source input) is **not
present on disk or in HEAD** — it was deleted in the "clean up artifacts" commit
`63464a7` (and a sibling commit `42904c5`). It is only recoverable from git history;
the last-known content lives at `ee48110:css/site.css` and was reconstructed from
`git show`. `scripts/build-css.sh:34` still references the missing file. The committed
`static/site.css` survives as the only build output.

---

## Q1: Anchor/colour rules in the Tailwind source

### Findings
- **No global `a` reset rule exists** in the source or the compiled CSS. The `@layer
  base` block in the recovered source (`ee48110:css/site.css`) defines only `*`,
  `body`, `.wallpaper`, `.page`, `.container`, `nav`, `nav a`, `nav a:hover`. There is
  **no bare `a { … }`, `a:link`, `a:visited`, or `:any-link`** rule.
- The only anchor rule is **nav-scoped** (recovered source):
  - `nav a { color: var(--color-text); text-decoration: none }`
  - `nav a:hover { color: var(--color-accent-strong) }`
- **Compiled `static/site.css` agrees**: the only `a` rule emitted in its `@layer
  base` is `nav a{color:var(--color-text);text-decoration:none}` plus
  `nav a:hover{color:var(--color-accent-strong)}` (minified, single-line file — line 1).
  `grep` for `:visited` / `:link` / `:any-link` / `color:inherit` across
  `static/site.css` returns **nothing**.
- **Preflight is intentionally skipped.** The source header
  (`ee48110:css/site.css`) says preflight is NOT imported because base rules rely on
  UA-default margins; only `@import "tailwindcss/theme.css" layer(theme)` and
  `@import "tailwindcss/utilities.css" layer(utilities)` are imported. Consequently
  Tailwind's normal `a { color: inherit; text-decoration: inherit }` preflight reset
  is **not emitted** into the compiled CSS.
- **Consequence:** any anchor that is NOT inside `<nav>` matches no author rule for
  colour/decoration, so it falls back to **browser UA defaults** — `:link` blue +
  underline, `:visited` purple + underline. (Nav anchors ARE styled by `nav a` on both
  visited and unvisited states.)
- Elements referenced: `src/interfaces/routes.rs:44-51` (static serving), and the
  recovered source blocks described above.

---

## Q2: Link styling across templates

### Findings — complete anchor inventory (4 anchors, plus zero in singlethread)
- **`templates/layout.html:12`** — `<a href="/">Home</a>` — **no class attribute**.
  Styled only by `nav a` (color `var(--color-text)`, no underline). Visited/unvisited
  identical.
- **`templates/layout.html:13`** — `<a href="/singlethread">SingleThread</a>` — **no
  class attribute**. Styled only by `nav a`. Identical for visited/unvisited.
- **`templates/home.html:23-24`** — GitHub link:
  `<a href="https://github.com/alanvardy" target="_blank"
     class="flex items-center gap-2 py-2 no-underline hover:text-accent">`.
  NOT inside `<nav>`, so `nav a` does not apply. Classes resolve to layout
  (`flex`, `items-center`, `gap-2`, `py-2`), `no-underline` (strips underline), and
  `hover:text-accent` (colour change on hover **only**). **No base colour class.**
- **`templates/home.html:31-32`** — LinkedIn link, identical class list
  (`flex items-center gap-2 py-2 no-underline hover:text-accent`). **No base colour
  class.** Also outside `<nav>`.
- **`templates/singlethread.html`** — **zero `<a>` elements.** All content uses
  `p`, `h2`, `ul`, `li`, `img`, `figure`; no anchors at all.

### Findings — utility definitions (compiled `static/site.css`, line 1)
- `.no-underline{text-decoration-line:none}` — used on GitHub/LinkedIn anchors.
- `.hover\:text-accent:hover{color:var(--color-accent)}` — wrapped in
  `@media (hover:hover)`, so colour applies only on pointer-hover.
- `.text-accent{color:var(--color-accent)}` — **NOT used on any anchor**; used on
  `singlethread.html:71` (`<p>`) and `marker:text-accent` list markers
  (`singlethread.html:44,52,62`).
- `.text-muted{color:var(--color-muted)}` — **NOT used on any anchor**; used on
  headings/paragraphs (`home.html:20`; `singlethread.html:7,17,35,43,50,61,68`).
- Theme vars: `--color-text:#ece7e2`, `--color-accent:#fb923c`,
  `--color-accent-strong:#fdba74`, `--color-muted:#a8a29e` (from recovered source
  `@theme` and compiled `@layer theme` in `static/site.css`).

### Findings — fallback behaviour (the crux for "used links")
- **Nav anchors** (`layout.html:12,13`): `nav a` assigns author colour
  `var(--color-text)`, which cascades to visited and unvisited alike → no purple, no
  underline; visited/unvisited indistinguishable.
- **Body anchors** (`home.html:23-24, 31-32`): not matched by `nav a`, have **no base
  colour class** (`text-accent`/`text-muted` unused on them), and preflight is skipped.
  Underline is suppressed by `no-underline`, but the **colour** is browser-default:
  **unvisited `:link` = blue, visited `:visited` = purple.** This is the mechanism by
  which a used (visited) GitHub/LinkedIn link currently differs in colour from an
  unused one.

---

## Q3: Compiled CSS pipeline

### Findings — build script (`scripts/build-css.sh`)
- Pinned standalone Tailwind CLI: `TAILWIND_VERSION="v4.3.3"` (`build-css.sh:6`);
  platform asset per `uname` (`build-css.sh:10-16`); binary cached under
  `target/tailwindcss-cli` (`:19-21`); re-downloaded when missing or checksum mismatch
  via pinned `SHA256` constants (`:25-30`).
- **Invocation:** `"$bin" -i css/site.css -o static/site.css --minify`
  (`build-css.sh:34`). Source = `css/site.css`, output = committed `static/site.css`,
  minified.
- **Source is missing:** `css/site.css` was deleted and is absent at HEAD; the build
  script `:34` and the Dockerfile builder stage both reference it, so the pipeline
  cannot currently run until the source is restored.

### How `@layer base` is authored (recovered source `ee48110:css/site.css`)
- Source top: `@source not "../.pi"` and `@source not "../static"` — excludes planning
  notes and generated output from Tailwind's automatic source scanning.
- Layer order declared: `@layer theme, base, components, utilities`.
- Imports: `theme.css` `layer(theme)` + `utilities.css` `layer(utilities)` (no
  preflight).
- `@theme { --color-bg: …; --color-surface; … }` token palette.
- `@layer base { *, ::before, ::after box-sizing; body; .wallpaper; .page;
  .container; nav; nav a; nav a:hover }`.

### Custom element rules & pseudo-classes through minification
- Compiled `static/site.css` keeps the @layer structure emitted by Tailwind v4:
  `@layer properties`, `@layer theme`, `@layer base`, `@layer components`, `@layer
  utilities` (one of each). Custom base rules (`nav a`, `nav a:hover`, `body`, etc.)
  survive verbatim into `@layer base`.
- Minification (`--minify`) collapses the file to a **single line** but does not strip
  custom selectors; `nav a:hover` and the `@media (hover:hover)`-wrapped
  `.hover\:text-accent:hover` both remain.
- **No `:visited`** rule exists anywhere (`static/site.css`, `src/`, `templates/` —
  rg returns zero matches), so pseudo-class preservation is untested/unneeded today.

### Rebuild gating
- `scripts/test.sh` runs `./scripts/build-css.sh` and then a drift check
  `git diff --exit-code -- static/site.css` (the plan for VAR-682 describes this; the
  gate lives in `scripts/test.sh`).
- `Dockerfile` builder stage inlines a pinned `tailwindcss-linux-x64` download and runs
  `tailwindcss -i css/site.css -o static/site.css --minify` to regenerate CSS inside the
  image before `cargo build`. It also currently depends on the missing source file.

---

## Q4: Static asset cache-busting

### How `asset_url()` works (`src/app/assets.rs`)
- `static ASSET_HASHES: OnceLock<HashMap<String,String>>` (`assets.rs:8`) is
  populated **lazily** on first call via `ASSET_HASHES.get_or_init(|| hash_all("static"))`
  (`assets.rs:38`).
- `hash_all("static")` (`assets.rs:12`) recurses through the `static/` dir
  (`hash_dir`, `assets.rs:18`); for each file it SHA-256-hashes the bytes and stores a
  **12-hex-prefix** of the digest keyed by path relative to `static/`
  (`assets.rs:31`).
- `asset_url(file)` returns `/static/<file>?v=<12hex>` (`assets.rs:37-42`), panicking
  on unknown assets. This is a **content hash** of the file bytes, so **any change to
  `static/site.css` (e.g. a rebuild via `build-css.sh`) yields a new `?v=` value.**

### Wiring to templates
- `src/app/templates.rs:13-16` registers a minijinja function `asset_url` backed by
  `assets::asset_url`; template loader is `minijinja::path_loader("templates")`
  (`templates.rs:5`). Function returns a safe string (no escaping).
- `templates/layout.html:7` references the stylesheet as
  `<link rel="stylesheet" href="{{ asset_url('site.css') }}">`. Every render of a page
  that inherits `layout.html` resolves the current `?v=` and embeds it in the HTML.

### Serving & caching (`src/interfaces/routes.rs:44-51`)
- `/static` is served by `ServeDir::new("static")` wrapped in
  `SetResponseHeader::overriding(CACHE_CONTROL, "public, max-age=31536000,
  immutable")`. Because the `?v=` is a content hash, an immutable cache is safe —
  browsers fetch the new URL when the hash string changes; the file at the old hashed
  URL never changes.
- Tests assert immutable caching + `text/css` content-type for `site.css`
  (`routes.rs` `static_stylesheet_is_served`).

### Startup timing nuance
- Hashes are computed **on first `asset_url` call** (`OnceLock`), i.e. during template
  init/render, not at process boot strictly — the Q header's "computed at startup"
  is accurate in practice since every rendered page calls it.

---

## Cross-Cutting Observations
- **Preflight is off app-wide.** Because only `theme.css` and `utilities.css` are
  imported, browser UA defaults (margins, anchor colours, etc.) are preserved except
  where an author rule explicitly overrides. This is the root reason content-area
  links show UA blue/purple.
- **Only `<nav>` anchors are author-styled in colour.** Every other anchor relies on
  UA defaults; the home page GitHub/LinkedIn links are the only *visitable* anchors
  in content and are the only ones that show the visited-vs-unvisited colour split.
- **SingleThread page has no anchors at all** — the "used links" concern is confined to
  `home.html` content links and (implicitly) the nav.
- **Utility invention in templates is scan-based.** Tailwind detects classes by
  scanning `templates/*.html` (not ignored); new utilities must already exist in the
  source or be added, and `static/site.css` regenerated via `build-css.sh:34` (which
  currently cannot run because its input is missing).
- **Content-hashed `?v=` + immutable caching** is the intended mechanism to push CSS
  changes to browsers; a source→compiled change must actually alter `static/site.css`
  bytes for the hash to rotate.

## Open Areas
- `css/site.css` is absent from HEAD yet referenced by `build-css.sh:34` and the
  Dockerfile; the exact current-build provenance of `static/site.css` (which source
  revision produced it) is unverifiable — `ee48110` is the last known source commit.
- Whether the intended change should reset/stylize content-area anchors (a base-layer
  global `a` rule vs. per-link utilities vs. `:visited` styling) is not present in the
  codebase today; nothing marks visited links today.
- Exact CSS-recompute workflow in the deploy path: `static/site.css` is committed, so
  CI deploys its committed artifact; the Dockerfile inline rebuild is present but
  blocked on the missing source.