# Implementation Plan

## Overview

Adopt Tailwind CSS v4 via the pinned standalone CLI: `css/site.css` (CSS-first source,
outside `static/`) compiles into the committed, fingerprint-served `static/site.css`,
gated by a rebuild-and-diff drift check in `scripts/test.sh` and mirrored in the Docker
builder stage. Once the pipeline proves invisible, redesign home and SingleThread pages
one at a time to pure utility classes, removing legacy CSS and fixing hard-coded asset
URLs along the way.

**Pinned CLI version**: `v4.3.3` (current latest v4 release). Platform binaries:
`tailwindcss-macos-arm64` and `tailwindcss-linux-x64` from the GitHub release.

**Preflight decision (deviation from structure.md shorthand)**: Phase 1 requires "zero
visual change", but a bare `@import "tailwindcss"` also injects Tailwind's *preflight*
reset, which strips UA-default margins on `p`, `h1`, `ul`, etc. that the current
hand-rolled rules silently rely on — that alone would visibly shift spacing. To keep
Phase 1 truly invisible, the source imports only the **theme + utilities layers**
(skipping preflight):

```css
@layer theme, base, components, utilities;
@import "tailwindcss/theme.css" layer(theme);
@import "tailwindcss/utilities.css" layer(utilities);
```

All default utilities and `@theme` tokens work exactly as with the full import. This
form is kept through Phase 4 (final file = these two imports + `@theme` + a small
global-base section for `body`/`nav`/`.container`, which `layout.html` still uses).
Everything else in the structure outline is followed as written.

---

## Phase 1: Tailwind pipeline with zero visual change

### Changes

#### 1. Obtain and record CLI checksums (implementation-time step)
**Action**: one-off, results pinned into files below

```fish
cd /tmp
curl -fsSLO https://github.com/tailwindlabs/tailwindcss/releases/download/v4.3.3/tailwindcss-macos-arm64
curl -fsSLO https://github.com/tailwindlabs/tailwindcss/releases/download/v4.3.3/tailwindcss-linux-x64
shasum -a 256 tailwindcss-macos-arm64 tailwindcss-linux-x64
```

Record the two digests as `MACOS_ARM64_SHA256` / `LINUX_X64_SHA256` in `build-css.sh`
and the linux digest as `TAILWIND_SHA256` in the Dockerfile. If GitHub is unreachable
from Fly remote builders later, vendoring the binary is a follow-up decision (design.md
Open Risks) — do not silently fall back to unpinned downloads.

#### 2. `css/site.css` (new, repo root — input, never served)
**Action**: create

Content = the two layer imports above, plus `@theme` tokens, plus the **entire current
content of `static/site.css` copied verbatim** (all rules: `*`, `body`, `.container`,
`.card`, `nav`, home-only, `st-*`, `.section-heading`, and the 48rem media query).
The `:root` block stays as-is (legacy rules consume `var(--…)`); the new `@theme`
mirrors the palette for future utility generation:

```css
@layer theme, base, components, utilities;
@import "tailwindcss/theme.css" layer(theme);
@import "tailwindcss/utilities.css" layer(utilities);

@theme {
    --color-bg: #121212;
    --color-surface: #1e1e1e;
    --color-text: #e0e0e0;
    --color-muted: #9e9e9e;
    --color-accent: #7aa2f7;
}

/* … current static/site.css rules pasted verbatim below this line … */
```

Note: Tailwind v4 auto-detects sources from the working directory (repo root),
respecting `.gitignore`. `templates/*.html` is not ignored, so class detection covers
our templates with no config file. Confirm the compiled output actually contains the
utility classes introduced in Phases 3–4 (spot-check in Verification).

#### 3. Compile to `static/site.css` (generated, committed)
**Action**: overwrite with CLI output

Run the build script (item 4) and commit the result. Minified output replaces the
198-line hand-rolled file; `assets::asset_url('site.css')` picks up the new sha256
hash automatically at startup — no Rust/template changes.

#### 4. `scripts/build-css.sh` (new)
**Action**: create, `chmod +x`

```bash
#!/usr/bin/env bash
# Compile css/site.css into the committed static/site.css with the pinned
# Tailwind standalone CLI. Binary is cached under target/ (already gitignored).
set -euo pipefail

TAILWIND_VERSION="v4.3.3"
MACOS_ARM64_SHA256="<recorded-in-step-1>"
LINUX_X64_SHA256="<recorded-in-step-1>"

case "$(uname -s)/$(uname -m)" in
    Darwin/arm64)
        asset="tailwindcss-macos-arm64"; expected="$MACOS_ARM64_SHA256" ;;
    Linux/x86_64)
        asset="tailwindcss-linux-x64"; expected="$LINUX_X64_SHA256" ;;
    *)
        echo "Unsupported platform: $(uname -s)/$(uname -m)" >&2; exit 1 ;;
esac

bin_dir="target/tailwindcss-cli"
mkdir -p "$bin_dir"
bin="$bin_dir/tailwindcss"

# Re-download if missing OR cached binary doesn't match the pinned checksum
# (e.g. after a version bump).
if [ ! -x "$bin" ] ||
    ! printf '%s  %s\n' "$expected" "$bin" | shasum -a 256 -c - >/dev/null 2>&1; then
    echo "⬇️  DOWNLOAD tailwindcss $TAILWIND_VERSION ($asset)"
    curl -fsSL -o "$bin" \
        "https://github.com/tailwindlabs/tailwindcss/releases/download/${TAILWIND_VERSION}/${asset}"
    chmod +x "$bin"
fi

printf '%s  %s\n' "$expected" "$bin" | shasum -a 256 -c -
"$bin" -i css/site.css -o static/site.css --minify
echo "✅  static/site.css rebuilt"
```

No `.gitignore` change needed: `target/` is already ignored. Verify before committing.

#### 5. `scripts/test.sh` (modify)
**Action**: insert drift gate before `cargo nextest run`

```bash
echo "🎨  BUILD CSS" &&
./scripts/build-css.sh &&
echo "🧭  CSS DRIFT CHECK" &&
# Committed artifact must match source; otherwise someone edited one side only
git diff --exit-code -- static/site.css &&
echo "📎  CLIPPY" &&
```

(i.e. the four new lines sit between the CLIPPY gate and the existing `cargo clippy`
invocation, so the rebuild definitely precedes `cargo nextest run`, which boots the app
and hashes `static/site.css`.)

#### 6. `Dockerfile` (modify)
**Action**: modify builder stage

`.dockerignore` excludes `scripts/`, so the Dockerfile cannot call `build-css.sh` —
the pinned download is inlined:

```dockerfile
FROM chef AS builder
ARG TAILWIND_VERSION=v4.3.3
ARG TAILWIND_SHA256=<linux-x64-digest-from-step-1>
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install sqlx-cli --no-default-features --features sqlite
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching Docker layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
# Rebuild CSS from source inside the image (overwrites the committed artifact)
RUN curl -fsSL -o /usr/local/bin/tailwindcss \
        "https://github.com/tailwindlabs/tailwindcss/releases/download/${TAILWIND_VERSION}/tailwindcss-linux-x64" \
    && echo "${TAILWIND_SHA256}  /usr/local/bin/tailwindcss" | sha256sum -c - \
    && chmod +x /usr/local/bin/tailwindcss \
    && tailwindcss -i css/site.css -o static/site.css --minify
ENV SQLX_OFFLINE=true
RUN cargo build --release --bin vardy
```

Runtime stage unchanged — it copies `/app/static` verbatim, now containing fresh CSS.

#### 7. `ROUTES.md`
**Action**: no change — confirm the `### GET /static/{file}` block is still accurate
(it is; nothing about serving changes).

### Verification
#### Automated
- [x] `./scripts/build-css.sh` exits 0 and prints the checksum OK line
- [x] Second consecutive run of `./scripts/build-css.sh` produces byte-identical
      `static/site.css` (`git diff --exit-code -- static/site.css` stays clean —
      determinism)
- [x] Tamper test: append a comment to `css/site.css` **without** rebuilding, run
      `git diff --exit-code -- static/site.css` after a forced rebuild → must fail;
      revert. Then rebuild and confirm gate green again
- [x] `./scripts/test.sh` passes end-to-end (drift gate active, all pre-existing tests
      unmodified and green)
- [x] `docker build .` succeeds locally
- [x] `grep -c 'tailwind' static/site.css` > 0 (output really is Tailwind-compiled)

#### Manual
- [ ] `cargo run` (or local server), then:
      `curl -sI localhost:3000/static/site.css` → 200, `text/css`,
      `cache-control: public, max-age=31536000, immutable`
- [ ] View `/` and `/singlethread` side-by-side against current deployed site —
      no visual difference (spacing, colors, breakpoints)

---

## Phase 2: Route all home-page images through `asset_url`

### Changes

#### 1. `templates/home.html` (modify)
**Action**: replace the four hard-coded URLs with the versioned global

- Line 4 (wave): `src="/static/wave.svg"` → `src="{{ asset_url('wave.svg') }}"`
- GitHub icon: `src="/static/github.svg"` → `src="{{ asset_url('github.svg') }}"`
- LinkedIn icon: `src="/static/linkedin.svg"` → `src="{{ asset_url('linkedin.svg') }}"`
- Portrait: `src="/static/alanvardy.jpg"` → `src="{{ asset_url('alanvardy.jpg') }}"`

All other markup/classes untouched in this phase.

#### 2. `src/interfaces/handlers/home/web.rs` (modify)
**Action**: update the two verbatim-image assertions (currently
`<img class="portrait" src="/static/alanvardy.jpg"` and
`<img class="wave" src="/static/wave.svg"`) to the `?v=` shape, mirroring
`singlethread/web.rs:46-50`:

```rust
assert!(body.contains(r#"<img class="portrait" src="/static/alanvardy.jpg?v="#));
assert!(body.contains(r#"<img class="wave" src="/static/wave.svg?v="#));
assert!(body.contains(r#"src="/static/github.svg?v="#));
assert!(body.contains(r#"src="/static/linkedin.svg?v="#));
```

### Verification
#### Automated
- [x] `cargo nextest run` passes

#### Manual
- [ ] View-source of `/`: all five image/CSS URLs carry `?v=<12 hex>`
      (wave.svg, alanvardy.jpg, github.svg, linkedin.svg, site.css)

---

## Phase 3: Redesign the home page with Tailwind utilities

First vertical slice: pure utility markup, dark identity via `@theme` tokens
(`bg-bg`, `text-text`, `text-muted`, `bg-accent`, `border-accent` resolve from the
Phase 1 `--color-*` definitions). Complete redesign freedom — the snippet below is the
reference direction, exact utility choices at implementer's discretion.

### Changes

#### 1. `templates/home.html` (rewrite)
**Action**: rewrite with Tailwind utilities only

Representative shape (responsive stacking via `md:` ≈ old 48rem breakpoint; portrait
above text on mobile via `order-first md:order-none`):

```html
{% extends "layout.html" %}
{% block title %}Home{% endblock %}
{% block heading %}
<img src="{{ asset_url('wave.svg') }}" alt="Waving hand" width="48" height="48"
     class="inline-block align-middle mr-2"> Hi!
{% endblock %}
{% block content %}
<div class="flex flex-col md:flex-row gap-8 items-start">
  <div class="space-y-4 md:flex-[3]">
    <p>My name is Alan Vardy, I am a Senior Developer living on the beautiful
       West Coast of Canada.</p>
    <p>I enjoy working with AI, backend Rust services and Swift applications …</p>
    <h2 class="text-muted text-xl mt-8 mb-3">You are invited to</h2>
    <ul class="list-none ml-0 py-0 pl-4 border-l-4 border-accent">
      <li>
        <a href="https://github.com/alanvardy" target="_blank"
           class="flex items-center gap-2 py-2 no-underline hover:text-accent">
          <img src="{{ asset_url('github.svg') }}" alt="" width="32" height="32"
               class="inline-block"> Take a look at my work on GitHub
        </a>
      </li>
      <!-- LinkedIn li, same pattern -->
    </ul>
  </div>
  <div class="w-full max-w-[200px] order-first md:order-none md:flex-1">
    <img src="{{ asset_url('alanvardy.jpg') }}" alt="Portrait of Alan Vardy"
         class="w-full rounded-lg border border-neutral-700">
  </div>
</div>
{% endblock %}
```

Constraints: **no** `.home`, `.home-columns`, `.home-portrait`, `.portrait`,
`.invite-list`, `.invite-icon`, `.section-heading`, `.wave` remnants; every image via
`asset_url()`.

#### 2. `css/site.css` (modify, then regenerate)
**Action**: delete the home-only legacy rules — `.home .wave`, `.home-columns`,
`.home-text`, `.home-portrait`, `.portrait`, `.invite-list` (+ link states),
`.invite-icon`. **Keep `.section-heading`** (singlethread.html still uses it until
Phase 4). Keep everything else (`body`, `.container`, `nav`, `st-*`, media query,
`.card`). Run `./scripts/build-css.sh` and commit the regenerated `static/site.css`.

#### 3. `src/interfaces/handlers/home/web.rs` (modify)
**Action**: rewrite body assertions for the new markup; keep asserting status +
content-type + body:

```rust
let body = res.text().await.unwrap();
assert!(body.contains("<title>Home</title>"));
assert!(body.contains("Hi!"));
assert!(body.contains("My name is Alan Vardy"));
assert!(body.contains("AI, backend Rust services and Swift applications"));
assert!(body.contains("high-output individual contributor"));
assert!(body.contains("You are invited to"));
assert!(body.contains(r#"href="https://github.com/alanvardy""#));
assert!(body.contains(r#"href="https://www.linkedin.com/in/alanvardy/""#));
// all images versioned
assert!(body.contains(r#"src="/static/wave.svg?v="#));
assert!(body.contains(r#"src="/static/alanvardy.jpg?v="#));
assert!(body.contains(r#"src="/static/github.svg?v="#));
assert!(body.contains(r#"src="/static/linkedin.svg?v="#));
// no legacy component classes remain on this page
assert!(!body.contains("home-columns"));
assert!(!body.contains("invite-list"));
// nav chrome unchanged
assert!(body.contains(r#"<a href="/">Home</a>"#));
assert!(body.contains(r#"<a href="/singlethread">SingleThread</a>"#));
assert!(body.contains("/static/site.css?v="));
assert!(!body.contains("<style>"));
```

### Verification
#### Automated
- [x] `cargo nextest run` passes
- [x] `rg 'class="[^"]*\b(home|portrait|invite-)' templates/` returns nothing
- [x] `rg 'section-heading' templates/home.html` returns nothing
- [x] Drift gate inside `./scripts/test.sh` confirms committed `static/site.css`
      matches source

#### Manual
- [ ] Review `/` at mobile (~375px) and desktop widths: columns stack, portrait above
      text on narrow screens, invite links legible with icons aligned
- [ ] `/singlethread` untouched and still rendering correctly
- [ ] Screenshots (mobile + desktop) captured for PR description

---

## Phase 4: Redesign the SingleThread page and finish the CSS cutover

### Changes

#### 1. `templates/singlethread.html` (rewrite)
**Action**: replace every `st-*` and `.section-heading` class with utilities; **drop
the unstyled `.st-watch` wrapper `<div>` entirely** (Decision 6) — promote its
`<h2>` and watch-pair contents up a level. Representative conversions:

| Old | New |
|---|---|
| `div.st-hero` | `div class="flex flex-col md:flex-row gap-8 items-center"` |
| `div.st-hero-text` | `div class="space-y-4 md:flex-1"` |
| `p.st-tagline` | `p class="text-xl text-muted"` |
| `div.st-hero-shot` | `div class="w-full max-w-[16rem] order-first md:order-none md:flex-none"` |
| `figure.st-shot` ×3 grid | `div class="flex flex-wrap gap-6"` / `figure class="m-0 flex-1 basis-[10rem] max-w-[14rem]"` |
| `.st-watch-pair` | `div class="flex justify-center gap-6"` (wrapper div removed) |
| `h2.section-heading` (×5) | `h2 class="text-muted text-xl mt-8 mb-3"` |
| `ul.st-list` (×3) | `ul class="list-disc pl-6 space-y-2 [&_li]::marker:text-accent"` or simpler: `ul class="list-disc pl-6 marker:text-accent space-y-2"` |
| `p.st-closing` | `p class="text-xl text-accent text-center mt-12"` |

Images keep `{{ asset_url(...) }}` calls — unchanged. All images get the shared
treatment `class="w-full rounded-lg border border-neutral-700"` (replaces six
`1px solid #333` border rules). Exact utilities at implementer's discretion;
`marker:text-accent` (Tailwind v4 supports `marker:` variant) replaces the
`li::marker` rule.

#### 2. `css/site.css` (reduce, then regenerate)
**Action**: delete all remaining hand-rolled rules except the global base:
- Remove: all `.st-*` rules, `.section-heading`, `.card` stays deleted-or-present per
  Phase 5 ordering (see Phase 5; safe either way since nothing references it), the 48rem
  `@media` block (superseded by Tailwind default breakpoints), `:root` palette block,
  and the `*` box-sizing reset (preflight-free setup keeps browser default
  `box-sizing: content-box` — **add `*, ::before, ::after { box-sizing: border-box; }`
  back into a `@layer base { … }` section**, see item 3)
- Final file shape:

```css
@layer theme, base, components, utilities;
@import "tailwindcss/theme.css" layer(theme);
@import "tailwindcss/utilities.css" layer(utilities);

@theme {
    --color-bg: #121212;
    --color-surface: #1e1e1e;
    --color-text: #e0e0e0;
    --color-muted: #9e9e9e;
    --color-accent: #7aa2f7;
}

/* Global element defaults for layout.html chrome (shared across pages) */
@layer base {
    *, ::before, ::after {
        box-sizing: border-box;
    }

    body {
        margin: 0;
        background: var(--color-bg);
        color: var(--color-text);
        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
        line-height: 1.6;
    }

    .container {
        max-width: 48rem;
        margin: 0 auto;
        padding: 3rem 1.5rem;
    }

    nav {
        display: flex;
        gap: 1.5rem;
        padding: 0.75rem 1.5rem;
        background: var(--color-surface);
        border-bottom: 1px solid #333;
    }

    nav a {
        color: var(--color-text);
        text-decoration: none;
    }

    nav a:hover {
        color: var(--color-accent);
    }
}
```

(`.container`, `nav`, and `body` persist because `layout.html` renders them on every
page and `layout.html` is not part of the redesign scope — structure.md lists no
changes to it.)

Run `./scripts/build-css.sh`, commit regenerated `static/site.css`.

#### 3. `src/interfaces/handlers/singlethread/web.rs` (modify)
**Action**: text-content assertions largely survive (copy is unchanged); update/remove
anything tied to removed classes:

```rust
// existing text assertions stay: title, h1, tagline, bullets, section headings,
// closing line, five versioned screenshot URLs, nav chrome
// add:
assert!(!body.contains("st-"));           // no st-* classes remain
assert!(!body.contains("section-heading"));
```

### Verification
#### Automated
- [ ] `cargo nextest run` passes
- [ ] `rg 'st-|section-heading|@media|\.card|\.portrait|\.home|\.invite|\.wave' css/site.css`
      returns nothing
- [ ] `./scripts/test.sh` full gate green (fmt → sqlx prepare → check → clippy → CSS
      build + drift → nextest → TODO grep)

#### Manual
- [ ] Review `/singlethread` at mobile + desktop: hero stacks with screenshot above
      text, three-shot grid wraps, watch pair centered, accent list markers
- [ ] Both pages checked back-to-back for consistent typography/spacing
- [ ] Screenshots (mobile + desktop) captured for PR description

---

## Phase 5: Dead-code sweep and supersession bookkeeping

### Changes

#### 1. `static/quill.png` (delete)
**Action**: `git rm static/quill.png`. Grep first to confirm no references:

```fish
rg 'quill' --hidden -g '!.pi' -g '!target'
```

Must return nothing (note: `singlethread-icon.png` is referenced by tests — leave it).

#### 2. `.card` rule removal (if still present)
**Action**: if the `.card` block survived Phases 1–4 in `css/site.css`, delete it now
(Decision 6: defined but unused), regenerate `static/site.css`, commit both.

#### 3. Invariant checks (no edits expected unless a check fails)
**Action**: run and confirm empty outputs:

```fish
rg '/static/' templates/          # only {{ asset_url(...) }} forms allowed → expect NO raw hits
rg '@apply' css/ templates/       # expect nothing
rg 'quill'                        # expect nothing (post-delete)
```

If the `/static/` grep hits anything other than `asset_url` output in rendered HTML
(i.e. a literal in a template), fix that template to use `asset_url()`.

#### 4. Supersession bookkeeping
**Action**: none beyond what design.md records — VAR-682 supersedes the
VAR-657/VAR-664/VAR-670 no-build-step decisions; old design docs are not edited
(Decision 7). Confirm the PR description mentions the supersession.

#### 5. `ROUTES.md`
**Action**: confirm no changes needed (no route changes in any phase).

### Verification
#### Automated
- [ ] Fresh clone (or `git clean -xfd` in a scratch clone): `./scripts/test.sh` green
      end-to-end — proves no hidden dependency on stale `target/` artifacts
- [ ] `./scripts/test.sh` TODO/FIXME grep gate passes
- [ ] All three invariant greps above return nothing

#### Manual
- [ ] Local server (or deploy preview): click through `/` and `/singlethread`, follow
      the GitHub/LinkedIn links, confirm images load and caching headers hold
- [ ] PR description includes before/after screenshots (mobile + desktop) and the
      supersession note

---

## Testing Checkpoints (from structure.md)

- **After Phase 1**: all pre-existing tests pass unmodified; drift gate active in
  `test.sh`; Docker image builds; site visually unchanged.
- **After Phase 2**: home handler tests assert `?v=` shapes; all template images
  versioned.
- **After Phase 3**: home redesigned; home-only legacy CSS gone; singlethread
  untouched and rendering correctly.
- **After Phase 4**: both pages on pure utilities; `css/site.css` reduced to imports +
  `@theme` + minimal base; media query gone.
- **After Phase 5**: no dead assets/classes; full gate green from clean checkout —
  ready for PR review with screenshots.
