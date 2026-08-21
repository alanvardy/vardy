# Implementation Plan

## Overview

Redesign the `/` homepage into an attractive, informative page about Alan Vardy
(greeting + wave, two bio paragraphs, "You are invited to" icon links to
blog/GitHub/LinkedIn, portrait photo) modeled on the reference site at
`/Users/vardy/dev/alan_vardy`, using template + static-asset changes only.
Handlers keep rendering empty contexts; no new routes, state, or dependencies.

**Verification commands for the whole task** (from `.github/workflows/ci.yml`):

```sh
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
./scripts/lint_string.sh "FIXME "   # also "FIXME:", "fixme ", "fixme:", "dbg!"
```

---

## Phase 1: Extract CSS to `static/site.css`

Move the inline `<style>` block out of `templates/layout.html` into a new
external stylesheet. Zero intended visual change.

### Changes

#### 1. New stylesheet with the existing rules, verbatim
**File**: `static/site.css`
**Action**: create

Copy the body of the `<style>` block (`layout.html:7-51`) unchanged:

```css
:root {
    --bg: #121212;
    --surface: #1e1e1e;
    --text: #e0e0e0;
    --muted: #9e9e9e;
    --accent: #7aa2f7;
}

* {
    box-sizing: border-box;
}

body {
    margin: 0;
    background: var(--bg);
    color: var(--text);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    line-height: 1.6;
}

.container {
    max-width: 48rem;
    margin: 0 auto;
    padding: 3rem 1.5rem;
}

.card {
    background: var(--surface);
    border: 1px solid #333;
    border-radius: 8px;
    padding: 1.5rem;
}

nav {
    display: flex;
    gap: 1.5rem;
    padding: 0.75rem 1.5rem;
    background: var(--surface);
    border-bottom: 1px solid #333;
}

nav a {
    color: var(--text);
    text-decoration: none;
}

nav a:hover {
    color: var(--accent);
}
```

#### 2. Link the stylesheet instead of inlining it
**File**: `templates/layout.html`
**Action**: modify

Replace the entire `<style>…</style>` block (lines 7–51) with a `<link>` tag
in `<head>`:

```html
    <title>{% block title %}Home{% endblock %}</title>
    <link rel="stylesheet" href="/static/site.css">
```

Nothing else in the file changes — nav markup and the three blocks stay as-is.

#### 3. Add a static-stylesheet route test
**File**: `src/interfaces/routes.rs`
**Action**: modify (add test inside existing `#[cfg(test)] mod tests`)

Add a sibling of `static_icon_is_served`, mirroring its shape:

```rust
#[tokio::test]
async fn static_stylesheet_is_served() {
    let addr = start_app().await;
    let client = test_client();
    let res = client
        .get(format!("http://{addr}/static/site.css"))
        .send()
        .await
        .expect("request failed");
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        res.headers()
            .get("content-type")
            .is_some_and(|v| v.to_str().unwrap().contains("text/css"))
    );
}
```

### Verification

#### Automated
- [x] `cargo test` passes — **all existing home/singlethread assertions unchanged and green**
- [x] New `static_stylesheet_is_served` test passes
- [x] `cargo fmt --all -- --check` passes
- [x] `cargo clippy --all-targets --all-features --locked -- -D warnings` passes

#### Manual
- [ ] `cargo run`, load `http://localhost:3000/` and `/singlethread` with a
      **hard refresh** (Cmd+Shift+R) — appearance identical to before
      (dark theme, card borders, nav styling). Note: once external, CSS is
      browser-cached; hard-refresh after any `site.css` edit.

---

## Phase 2: Copy assets from reference repo

Copy the five homepage images from the reference repo into `static/`, then
recolor the two monochrome SVGs so they are visible on the dark background.

### Changes

#### 1. Copy the assets
**Files**: `static/wave.svg`, `static/quill.png`, `static/github.svg`,
`static/linkedin.svg`, `static/alanvardy.jpg`
**Action**: create (copies)

```sh
cp /Users/vardy/dev/alan_vardy/priv/static/images/{wave.svg,quill.png,github.svg,linkedin.svg,alanvardy.jpg} static/
```

#### 2. Recolor `github.svg` and `linkedin.svg` for the dark theme
**Files**: `static/github.svg`, `static/linkedin.svg`
**Action**: modify

Both copied SVGs rely on Tailwind classes (`class="fill-black ..."` /
`class="fill-black"`) that do nothing here — with no `fill` attribute they
default to black and will be invisible on `#121212`. Since they load via
`<img src>` (external CSS cannot reach inside), fix the files themselves:
replace the `class="fill-black ..."` attribute on the root `<svg>` element
with an explicit light fill:

```xml
<!-- before (github.svg root element) -->
<svg class="fill-black hover:fill-black" version="1.2" ... viewBox="0 0 2350 2314.8">
<!-- after -->
<svg fill="#e0e0e0" version="1.2" ... viewBox="0 0 2350 2314.8">
```

Same change in `linkedin.svg` (`class="fill-black"` → `fill="#e0e0e0"`).
`wave.svg` (multicolor, `#45413c` strokes / `#ffcebf` fills) and `quill.png`
are fine as-is.

#### 3. Extend the static-route test to one new image asset
**File**: `src/interfaces/routes.rs`
**Action**: modify (add test)

```rust
#[tokio::test]
async fn static_homepage_image_is_served() {
    let addr = start_app().await;
    let client = test_client();
    let res = client
        .get(format!("http://{addr}/static/alanvardy.jpg"))
        .send()
        .await
        .expect("request failed");
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        res.headers()
            .get("content-type")
            .is_some_and(|v| v.to_str().unwrap().contains("image/jpeg"))
    );
}
```

### Verification

#### Automated
- [x] `cargo test` passes, including new `static_homepage_image_is_served`

#### Manual
- [ ] With `cargo run` running:
      `curl -sI localhost:3000/static/wave.svg localhost:3000/static/quill.png localhost:3000/static/github.svg localhost:3000/static/linkedin.svg localhost:3000/static/alanvardy.jpg` —
      all return `HTTP/1.1 200 OK` with an image/css-appropriate content type **(verified via curl during Phase 2 implementation)**
- [ ] Open each SVG directly in the browser (`/static/github.svg`,
      `/static/linkedin.svg`) — glyphs render in light gray, not black-on-dark

---

## Phase 3: Rewrite homepage content

Replace the placeholder in `templates/home.html` with the full new page and
rewrite the home HTTP test assertions.

### Changes

#### 1. New homepage markup
**File**: `templates/home.html`
**Action**: modify (full rewrite of the file)

Content adapted from reference `index.html.heex:1-46`, using this repo's
classes (Phase 4 adds the CSS for them):

```html
{% extends "layout.html" %}
{% block title %}Home{% endblock %}
{% block heading %}
<img class="wave" src="/static/wave.svg" alt="Waving hand" width="48" height="48"> Hi!
{% endblock %}
{% block content %}
<div class="home">
  <div class="home-columns">
    <div class="home-text">
      <p>
        My name is Alan Vardy, I am a Senior Developer living on the beautiful
        West Coast of Canada.
      </p>
      <p>
        I love working remotely on backend Elixir services, and enjoy playing
        with Rust in my free time. I pride myself on being a high-output
        individual contributor who leaves code better than he finds it and
        actively improves the overall health of codebases.
      </p>
      <h2 class="section-heading">You are invited to</h2>
      <ul class="invite-list">
        <li>
          <a href="https://www.alanvardy.com" target="_blank">
            <img class="invite-icon" src="/static/quill.png" alt="" width="32" height="32">
            Read my blog
          </a>
        </li>
        <li>
          <a href="https://github.com/alanvardy" target="_blank">
            <img class="invite-icon" src="/static/github.svg" alt="" width="32" height="32">
            Take a look at my work on GitHub
          </a>
        </li>
        <li>
          <a href="https://www.linkedin.com/in/alanvardy/" target="_blank">
            <img class="invite-icon" src="/static/linkedin.svg" alt="" width="32" height="32">
            Check out my LinkedIn
          </a>
        </li>
      </ul>
    </div>
    <div class="home-portrait">
      <img class="portrait" src="/static/alanvardy.jpg" alt="Portrait of Alan Vardy">
    </div>
  </div>
</div>
{% endblock %}
```

Notes:
- The `{% block heading %}` override now wraps greeting markup; `layout.html`
  still renders it inside `<h1>` — acceptable (one `<h1>` containing an img +
  "Hi!", matching the reference's icon+greeting treatment).
- Bio text is verbatim from the reference; Alan signs off on wording at PR
  review (design risk, not a blocker for implementation).

#### 2. Rewrite home test assertions
**File**: `src/interfaces/handlers/home/web.rs`
**Action**: modify (only the assertion block in `index_serves_ok_html`)

Replace lines 34–38 (the five `body.contains` asserts) with:

```rust
        let body = res.text().await.unwrap();
        assert!(body.contains("<title>Home</title>"));
        assert!(body.contains("Hi!"));
        assert!(body.contains("My name is Alan Vardy"));
        assert!(body.contains("high-output individual contributor"));
        assert!(body.contains("You are invited to"));
        assert!(body.contains(r#"href="https://www.alanvardy.com""#));
        assert!(body.contains(r#"href="https://github.com/alanvardy""#));
        assert!(body.contains(r#"href="https://www.linkedin.com/in/alanvardy/""#));
        assert!(body.contains(r#"<img class="portrait" src="/static/alanvardy.jpg""#));
        assert!(body.contains(r#"<img class="wave" src="/static/wave.svg""#));
        // nav chrome unchanged
        assert!(body.contains(r#"<a href="/">Home</a>"#));
        assert!(body.contains(r#"<a href="/singlethread">SingleThread</a>"#));
```

The old assertions for `"Welcome to vardy"` and the minijinja sentence are
deleted. Handler function body is untouched (still `context! {}`).

### Verification

#### Automated
- [x] `cargo test` passes with the new assertions
- [x] `cargo fmt --all -- --check` and clippy command pass

#### Manual
- [ ] `cargo run`, load `/` — greeting, both bio paragraphs, three invite
      links (each opens the correct external URL in a new tab), and portrait
      all present; layout unstyled/plain is acceptable at this point
      (`.home*`/`.invite-*` classes have no CSS yet)

---

## Phase 4: Style the homepage

Extend `static/site.css` with homepage-specific rules. No template or Rust
changes (class names from Phase 3 are final).

### Changes

#### 1. Homepage styles
**File**: `static/site.css`
**Action**: modify (append)

```css
/* Homepage */
.home .wave {
    vertical-align: middle;
    margin-right: 0.5rem;
}

.home-columns {
    display: flex;
    gap: 2rem;
    align-items: flex-start;
}

.home-text {
    flex: 3;
}

.home-portrait {
    flex: 1;
}

.portrait {
    width: 100%;
    max-width: 200px;
    border-radius: 8px;
    border: 1px solid #333;
}

.section-heading {
    color: var(--muted);
    font-size: 1.25rem;
    margin: 2rem 0 0.75rem;
}

.invite-list {
    list-style: none;
    margin: 0;
    padding: 0 0 0 1rem;
    border-left: 4px solid var(--accent);
}

.invite-list a {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--text);
    text-decoration: none;
    padding: 0.5rem 0;
}

.invite-list a:hover {
    color: var(--accent);
}

.invite-icon {
    display: inline-block;
}

@media (max-width: 48rem) {
    .home-columns {
        flex-direction: column;
    }

    .home-portrait {
        order: -1; /* portrait above text on narrow screens */
    }
}
```

The `border-left` treatment mirrors the reference's
`border-l-4 border-orange-700 pl-4` invite list (`index.html.heex:19`), using
the existing `--accent` variable instead of orange.

### Verification

#### Automated
- [x] `cargo test` passes (no Rust/template changes in this phase)
- [x] `cargo fmt --all -- --check` and clippy command pass

#### Manual
- [ ] Hard refresh `/` at desktop width: bio text and portrait side by side,
      invite list has accent left border, icons legible on dark background
- [ ] Narrow the window below ~48rem (or device toolbar): columns stack with
      portrait on top, nothing overflows
- [ ] Portrait looks proportioned and rounded on the dark theme
- [ ] **Get Alan's sign-off on the bio wording** before marking the PR ready

---

## Testing Checkpoints

- After Phase 1: `cargo test` green with **unchanged** home assertions; new
  `/static/site.css` test passes; pages look identical (hard refresh).
- After Phase 2: all five assets serve 200; `github.svg`/`linkedin.svg`
  visible in light gray on the dark theme.
- After Phase 3: home tests assert new content (title, greeting, bio
  fragments, three hrefs, image srcs, nav anchors); page fully readable but
  plainly laid out.
- After Phase 4: everything green + responsive layout verified manually;
  bio wording signed off; PR ready for review.

Each phase boundary is a valid stopping point with a green build.
