# Design Discussion

## Current State

The site has 4 anchors across 3 templates, with only nav links author-styled:
- **Nav anchors** (`templates/layout.html:12-13`, "Home" + "SingleThread"): styled by
  `nav a { color: var(--color-text); text-decoration: none }` and `nav a:hover {
  color: var(--color-accent-strong) }` in `@layer base` (recovered source
  `ee48110:css/site.css`). Visited/unvisited indistinguishable.
- **Content anchors** (`templates/home.html:23-24, 31-32`, GitHub + LinkedIn): no
  base colour class — only `no-underline` + `hover:text-accent`. Not inside `<nav>`,
  so `nav a` doesn't apply. Fall back to browser UA defaults: `:link` = blue,
  `:visited` = purple (research.md Q2). This is the bug.
- **SingleThread page** (`templates/singlethread.html`): zero `<a>` elements — no
  anchors to fix.
- **Preflight is off** (`ee48110:css/site.css` header). No `a { color: inherit }`
  reset exists. Only `theme.css` + `utilities.css` are imported.
- **`css/site.css` is missing from HEAD** — deleted in `63464a7`. Last known source
  at `ee48110`. `scripts/build-css.sh:34` references it; the pipeline can't run
  without it.
- **Cache-busting**: `asset_url()` in `src/app/assets.rs:31-42` computes a SHA-256
  12-hex-prefix `?v=` for `static/site.css`. Any byte change → new hash → browsers
  fetch updated CSS. Immutable `Cache-Control` on `/static` (`routes.rs:44-51`).
- **Gate**: `scripts/test.sh:12-15` runs `build-css.sh` then `git diff --exit-code --
  static/site.css` — any uncommitted CSS drift fails CI.

## Desired End State

1. **Every anchor on every page** — nav links, content links, any future links —
   renders with the **same accent colour** (`var(--color-accent)`, `#fb923c`) for
   both `:link` and `:visited` states. No browser-blue or purple anywhere.
2. **Hover** brightens to `var(--color-accent-strong)` (`#fdba74`), consistent across
   all anchors.
3. **No underline** on anchors by default.
4. **`css/site.css` restored** to disk so the build pipeline (`scripts/build-css.sh`)
   works again.
5. **`static/site.css` regenerated** with the new rules, committed, and its `?v=`
   hash rotated automatically.
6. **Templates cleaned up**: redundant utility classes (`hover:text-accent`,
   `no-underline`) removed from individual links.
7. **Tests pass** — no visual regression, all existing assertions hold.

### Verification
- Boot the app (`cargo run`), open the home page, click both GitHub and LinkedIn
  links. Verify both visited links show the same orange as unvisited.
- Inspect nav links — same behaviour.
- Run `./scripts/test.sh` — passes including the CSS drift gate.

## Patterns to Follow

| Pattern | Source | Notes |
|---------|--------|-------|
| Global element defaults in `@layer base` | `ee48110:css/site.css` — `body`, `nav`, `nav a`, `nav a:hover` are defined here | Same layer, same approach |
| Design tokens via `@theme` custom properties | `ee48110:css/site.css` — `--color-accent`, `--color-accent-strong`, etc. | Use `var()`, never hardcoded hex |
| Utility classes from Tailwind for layout, not colour | Existing `flex items-center gap-2 py-2` on content links (`home.html:23-24`) | Keep layout utilities; remove colour/decoration ones that the global rule now covers |
| `scripts/build-css.sh` → `static/site.css` → `git diff` gate | `scripts/build-css.sh:34`; `scripts/test.sh:12-15` | Source change → rebuild → commit both |
| Content-hash cache busting | `src/app/assets.rs:31-42` (`asset_url`), `templates/layout.html:7` | No manual version bumps — hash rotation is automatic |
| Template inheritance via `layout.html` base | `templates/layout.html` extended by `home.html` and `singlethread.html` | CSS changes in layout cascade to all pages |

### Patterns to Avoid
- **Hand-editing `static/site.css`** — it's minified single-line output. Always edit
  `css/site.css` and rebuild.
- **Adding per-element colour classes** for anchors — the global rule makes these
  redundant and they'd fight for specificity. Remove `hover:text-accent` and
  `no-underline` from individual links.
- **Importing preflight** — the site relies on UA-default margins. Don't add `base.css`.

## Design Decisions

1. **Global `a` rule in `@layer base`**: A single `a`, `a:visited`, and `a:hover`
   ruleset covers all current and future anchors — nav, content, footer, anything.
   The alternative (per-element classes) means every new anchor author has to
   remember to add `text-accent no-underline`. This is the root cause of the current
   bug, so we fix it application-wide.

2. **Accent orange (`var(--color-accent)`) for all links**: Matches the existing
   hover style and the site's brand colour. The muted/text-colour alternatives blend
   links into body copy, hurting discoverability. Accent makes links visible and
   cohesive.

3. **Remove `nav a` / `nav a:hover` rules**: Once a global `a` rule exists, the
   nav-scoped selectors are redundant. Removing them keeps `@layer base` clean and
   avoids confusion about which rule wins. The global hover colour
   (`--color-accent-strong`) replaces `nav a:hover`.

4. **Restore `css/site.css` from `ee48110` as-is, then edit**: The source was
   deleted. Restore the last-known-good version first in a separate commit (or as
   the first step), then apply the link-style changes on top. This keeps the diff
   reviewable: one commit says "restore source", the next says "add global anchor
   rules".

5. **Remove `no-underline` and `hover:text-accent` from home.html links**: Once the
   global `a` rule sets `text-decoration: none` and `a:hover` to
   `--color-accent-strong`, these utility classes are dead code. Removing them
   prevents confusion about where link styling lives.

## What We're NOT Doing

- **NOT touching `singlethread.html`** — it has zero anchors. No changes needed.
- **NOT changing the colour of non-anchor text**, headings, or list markers that
  already use `text-accent` / `text-muted`.
- **NOT importing Tailwind preflight** — the site is built around UA-default margins.
- **NOT adding a `:focus-visible` outline override** — out of scope. The global rule
  intentionally doesn't strip `outline`.
- **NOT adding a `transition`** for the hover colour — out of scope for this fix.
- **NOT changing the `@theme` token palette** — all values stay the same.
- **NOT altering `asset_url()`, the `?v=` mechanism, or `Cache-Control` headers** —
  they work correctly and require no changes.
- **NOT adding new pages/templates/routes** — pure styling fix.

## Open Risks

- **Tailwind class scan may drop `hover:text-accent` from utilities**: After removing
  `hover:text-accent` from templates, Tailwind v4's class scanner won't find it in
  `templates/*.html` anymore. If NO other element uses it (the research says
  `text-accent` is used on `<p>` in `singlethread.html:71`, but `hover:text-accent`
  variants may not be), the generated utility won't be emitted in `static/site.css`.
  This is actually desired — the global `a:hover` rule replaces it — but we must
  verify the rebuild doesn't drop a utility still needed elsewhere.
- **`css/site.css` rebuild drift**: The `ee48110` source may not produce the exact
  same `static/site.css` as currently on disk (Tailwind CLI version pinning should
  prevent this, but minor formatting or dependency changes in v4.3.3 could cause
  unexpected diff churn). Mitigation: rebuild and review the full diff before
  committing.
- **Dockerfile inline rebuild**: The Dockerfile builder stage recreates
  `static/site.css` from source at image build time — it will pick up the restored
  `css/site.css` and the new rules automatically. No Dockerfile changes needed, but
  worth a mental note that the deploy path relies on source being present.