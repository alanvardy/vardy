# Implementation Plan

## Overview
Rebuild `/singlethread` as a rich static marketing page for SingleThread: five real
screenshots served with versioned `asset_url()` references, a responsive hero, a
three-phone screenshot row, an Apple Watch pair, and full marketing copy — all using
new page-scoped `st-*` classes on the existing dark-theme tokens. No DB, handler,
route, or metrics changes.

## Frozen copy (verbatim — do not reword)
The page text below is the user-provided copy, frozen as of this plan. Phases 2–4
place it verbatim; Phase 4 is the last phase allowed to touch wording.

- **Promotional tagline**: `Your brain does one thing at a time — your list should too.`
- **Short description**: `SingleThread shows one Apple Reminder at a time for calm, focused momentum.`
- **Description heading/lead**: `Your task list should match how your brain actually works — one thing at a time.`
- **Long description**: `SingleThread turns your Apple Reminders into a single, calm, focused list. Instead of an intimidating wall of everything you "should" do, you see one clear reminder at a time, in order, and you process it — complete, skip, or delete — before moving to the next. It's built for people who feel overwhelmed, who are neurodivergent, or who simply want their to-dos to stop feeling like noise.`
- **Section "Why it helps"** (h2), 3 bullets:
  - **One at a time.** Your human brain does one thing at a time — your task list should reflect that. Just one reminder fills the screen, so every task gets your full attention.
  - **Order that makes sense.** Sort by priority, by due date (soonest first), or alphabetically. Pick whichever removes the most thinking.
  - **No decision paralysis.** See the reminder, act, move on. Fast. Smooth. Calm.
- **Section "Everything you need, nothing you don't"** (h2), lead-in: `SingleThread stays out of your way, but it's as full-featured — or as bare — as you want. Every capability can be turned on or off in one simple Settings screen:` then 6 bullets:
  - Complete, Skip, or Delete each reminder with a tap, a swipe, or the action buttons.
  - Hide reminders from projects you don't want to see (with "Excluded Projects").
  - Hide undated (someday) reminders, or show them when you're ready.
  - Show or hide reminder dates — keep the screen as minimal as you like.
  - Fast add by voice: dictate a reminder, complete with due date and recurrence, right from the microphone button (or hide it with one switch).
  - Change how it looks: System, Light, or Dark appearance, plus larger text sizes across the whole interface.
- **Section "Thoughtful by design"** (h2), 3 bullets:
  - Works with your existing Apple Reminders — everything you've already captured stays right where it is. Two-way, in place.
  - Calm visual design that gets out of the way. Big, legible text. Nothing flashing at you.
  - Accessible — designed with clear hit targets, complete descriptions and labels, and full support for Apple's rendering commands.
- **Closing section "Built for quiet productivity"** (h2), paragraphs:
  - `Whether your to-do list is a source of anxiety or just a source of clutter, SingleThread gives you a smaller, softer way through it. One item at a time, in an order you choose, at the pace that feels right. Enable the features that help, hide the ones that don't.`
  - `SingleThread works with your existing Apple Reminders and syncs across your devices. Available on iPhone, iPad, and Mac, with a matching Apple Watch app and macOS widget; install on any one to get going. No account to create — your reminders stay in place.`
- **Tagline line**: `Your reminders. One at a time. In order. At your pace.`

---

## Phase 1: Assets land with serving tests

### Changes

#### 1. Copy and rename screenshots into `static/`
**Files**: `static/singlethread-shot-main.jpg`, `static/singlethread-shot-settings.jpg`,
`static/singlethread-shot-swipe.jpg`, `static/singlethread-watch-list.png`,
`static/singlethread-watch-detail.png`
**Action**: create (copy from `~/Downloads`)

> **Deviation from structure.md**: the three phone screenshots are JPEG data
> (`IMG_5426/5427/5429.jpeg`, verified via `file`), not PNGs. They land as `.jpg`
> so `ServeDir` emits the correct `content-type`; converting to PNG would balloon
> each file to multiple MB. The two watch screenshots really are PNGs and keep
> `.png`. All names stay lowercase kebab-case as designed.

```fish
cp ~/Downloads/IMG_5426.jpeg static/singlethread-shot-main.jpg
cp ~/Downloads/IMG_5427.jpeg static/singlethread-shot-settings.jpg
cp ~/Downloads/IMG_5429.jpeg static/singlethread-shot-swipe.jpg
cp ~/Downloads/incoming-4A555EAD-9C9E-4272-BD9E-A91CC5D94CF6.PNG static/singlethread-watch-list.png
cp ~/Downloads/incoming-67B29C24-7098-420D-9EE8-DD992511A13A.PNG static/singlethread-watch-detail.png
```

#### 2. Static serving tests
**File**: `src/interfaces/routes.rs` (tests module only)
**Action**: modify

Add one test covering all five files: status 200, correct `content-type` per format
(`image/jpeg` × 3, `image/png` × 2), and immutable cache-control. Mirrors
`static_homepage_image_is_served` + `static_files_have_immutable_cache_control`.

```rust
#[tokio::test]
async fn singlethread_screenshots_are_served_with_immutable_caching() {
    let addr = start_app().await;
    let client = test_client();
    let cases = [
        ("/static/singlethread-shot-main.jpg", "image/jpeg"),
        ("/static/singlethread-shot-settings.jpg", "image/jpeg"),
        ("/static/singlethread-shot-swipe.jpg", "image/jpeg"),
        ("/static/singlethread-watch-list.png", "image/png"),
        ("/static/singlethread-watch-detail.png", "image/png"),
    ];
    for (path, content_type) in cases {
        let res = client
            .get(format!("http://{addr}{path}"))
            .send()
            .await
            .unwrap_or_else(|_| panic!("request failed for {path}"));
        assert_eq!(res.status(), StatusCode::OK, "{path}");
        assert!(
            res.headers()
                .get("content-type")
                .is_some_and(|v| v.to_str().unwrap().contains(content_type)),
            "{path}"
        );
        assert!(
            res.headers()
                .get("cache-control")
                .is_some_and(|v| v.to_str().unwrap().contains("max-age=31536000")),
            "{path}"
        );
    }
}
```

Nothing references the assets yet, so `asset_url` is untouched and the page renders
exactly as before.

### Verification
#### Automated
- [x] `./scripts/test.sh` passes
- [x] New test `singlethread_screenshots_are_served_with_immutable_caching` passes

#### Manual
- [ ] `ls static/singlethread-*` shows exactly five new files; `file static/singlethread-*` reports JPEG ×3 and PNG ×2
- [ ] With server running: `curl -sI localhost:<port>/static/singlethread-shot-main.jpg` shows `content-type: image/jpeg` and `cache-control: ...immutable`; same shape for the other four

---

## Phase 2: Hero section

### Changes

#### 1. Template: replace icon+card body with hero
**File**: `templates/singlethread.html`
**Action**: modify — delete the `<img>` icon and `<div class="card">` block; insert hero

```html
{% extends "layout.html" %}
{% block title %}SingleThread{% endblock %}
{% block heading %}SingleThread{% endblock %}
{% block content %}
<div class="st-hero">
    <div class="st-hero-text">
        <p class="st-tagline">Your brain does one thing at a time — your list should too.</p>
        <p>SingleThread shows one Apple Reminder at a time for calm, focused momentum.</p>
    </div>
    <div class="st-hero-shot">
        <img src="{{ asset_url('singlethread-shot-main.jpg') }}"
             alt="SingleThread showing a single reminder with Complete and Skip buttons">
    </div>
</div>
{% endblock %}
```

The old `singlethread-icon.png` reference disappears here; the icon file itself stays
in `static/` (still used elsewhere conceptually — do not delete).

#### 2. CSS: hero classes + mobile stacking
**File**: `static/site.css`
**Action**: modify — append a SingleThread section after the homepage section, and
extend the existing media query

```css
/* SingleThread */
.st-hero {
    display: flex;
    gap: 2rem;
    align-items: center;
}

.st-hero-text {
    flex: 1;
}

.st-tagline {
    font-size: 1.25rem;
    color: var(--muted);
}

.st-hero-shot {
    flex: 0 0 16rem;
    max-width: 16rem;
}

.st-hero-shot img {
    width: 100%;
    border-radius: 8px;
    border: 1px solid #333;
}
```

Inside the existing `@media (max-width: 48rem)` block, add:

```css
    .st-hero {
        flex-direction: column;
    }

    .st-hero-shot {
        order: -1; /* screenshot above text on narrow screens */
        flex: none;
    }
```

(`#333` matches the existing `.card`/`.portrait` border literal; all other colors come
from tokens.)

#### 3. Handler test: swap old copy assertions for hero assertions
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify — in `index_serves_ok_html`, remove the `"single line of work"`
assertion AND the `singlethread-icon.png?v=` assertion (the icon markup is gone);
keep title/h1/nav assertions; add:

```rust
assert!(body.contains("one thing at a time"));
assert!(body.contains(r#"<img src="/static/singlethread-shot-main.jpg?v="#));
```

### Verification
#### Automated
- [x] `./scripts/test.sh` passes

#### Manual
- [ ] `cargo run`, open `/singlethread`: hero shows tagline text left, main screenshot right
- [ ] Narrow browser window below 48rem: hero stacks with screenshot above text

---

## Phase 3: Screenshot row + Watch pair

### Changes

#### 1. Template: description, three-phone row, watch subsection
**File**: `templates/singlethread.html`
**Action**: modify — after `.st-hero`, add (long description paragraph is part of this
slice so the row has context; wording is already frozen above)

```html
<h2 class="section-heading">Your task list should match how your brain actually works — one thing at a time.</h2>
<p>SingleThread turns your Apple Reminders into a single, calm, focused list. Instead of an intimidating wall of everything you "should" do, you see one clear reminder at a time, in order, and you process it — complete, skip, or delete — before moving to the next. It's built for people who feel overwhelmed, who are neurodivergent, or who simply want their to-dos to stop feeling like noise.</p>

<div class="st-shots">
    <figure class="st-shot">
        <img src="{{ asset_url('singlethread-shot-main.jpg') }}" alt="One reminder at a time on iPhone">
    </figure>
    <figure class="st-shot">
        <img src="{{ asset_url('singlethread-shot-settings.jpg') }}" alt="The SingleThread settings sheet">
    </figure>
    <figure class="st-shot">
        <img src="{{ asset_url('singlethread-shot-swipe.jpg') }}" alt="Swiping to complete a reminder in light mode">
    </figure>
</div>

<div class="st-watch">
    <h2 class="section-heading">On your wrist</h2>
    <div class="st-watch-pair">
        <img src="{{ asset_url('singlethread-watch-list.png') }}" alt="Apple Watch showing a reminder with Complete and Skip buttons">
        <img src="{{ asset_url('singlethread-watch-detail.png') }}" alt="Apple Watch showing Refresh and Delete actions">
    </div>
</div>
```

Note: the hero also references `singlethread-shot-main.jpg` — intentional reuse of the
same asset (hero shows the one-reminder screen; the row repeats it alongside the other
two phones).

#### 2. CSS: shot row + watch pair
**File**: `static/site.css`
**Action**: modify — append after the hero rules

```css
.st-shots {
    display: flex;
    gap: 1.5rem;
    flex-wrap: wrap;
}

.st-shot {
    margin: 0;
    flex: 1 1 10rem;
    max-width: 14rem;
}

.st-shot img {
    width: 100%;
    border-radius: 8px;
    border: 1px solid #333;
}

.st-watch-pair {
    display: flex;
    gap: 1.5rem;
    justify-content: center;
}

.st-watch-pair img {
    width: 100%;
    max-width: 12rem;
    border-radius: 8px;
    border: 1px solid #333;
}
```

No extra media-query rules needed: `.st-shots` wraps via `flex-wrap`, and
`.st-watch-pair` images shrink under their `max-width`.

#### 3. Handler test: assert remaining versioned URLs
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify — add to `index_serves_ok_html`:

```rust
assert!(body.contains(r#"<img src="/static/singlethread-shot-settings.jpg?v="#));
assert!(body.contains(r#"<img src="/static/singlethread-shot-swipe.jpg?v="#));
assert!(body.contains(r#"<img src="/static/singlethread-watch-list.png?v="#));
assert!(body.contains(r#"<img src="/static/singlethread-watch-detail.png?v="#));
```

### Verification
#### Automated
- [ ] `./scripts/test.sh` passes

#### Manual
- [ ] `/singlethread` shows three rounded-corner bordered phone screenshots in a row
- [ ] The two watch images sit side by side beneath them
- [ ] Narrow viewport: the shot row wraps onto multiple lines; watch pair stays side-by-side but shrinks
- [ ] View source: every `img src` carries a distinct `?v=` hash suffix

---

## Phase 4: Feature prose sections + final copy

### Changes

#### 1. Template: feature sections + closing
**File**: `templates/singlethread.html`
**Action**: modify — after `.st-watch`, append the three prose sections and closing
(copy verbatim from "Frozen copy" above):

```html
<h2 class="section-heading">Why it helps</h2>
<ul class="st-list">
    <li><strong>One at a time.</strong> Your human brain does one thing at a time — your task list should reflect that. Just one reminder fills the screen, so every task gets your full attention.</li>
    <li><strong>Order that makes sense.</strong> Sort by priority, by due date (soonest first), or alphabetically. Pick whichever removes the most thinking.</li>
    <li><strong>No decision paralysis.</strong> See the reminder, act, move on. Fast. Smooth. Calm.</li>
</ul>

<h2 class="section-heading">Everything you need, nothing you don't</h2>
<p>SingleThread stays out of your way, but it's as full-featured — or as bare — as you want. Every capability can be turned on or off in one simple Settings screen:</p>
<ul class="st-list">
    <li>Complete, Skip, or Delete each reminder with a tap, a swipe, or the action buttons.</li>
    <li>Hide reminders from projects you don't want to see (with "Excluded Projects").</li>
    <li>Hide undated (someday) reminders, or show them when you're ready.</li>
    <li>Show or hide reminder dates — keep the screen as minimal as you like.</li>
    <li>Fast add by voice: dictate a reminder, complete with due date and recurrence, right from the microphone button (or hide it with one switch).</li>
    <li>Change how it looks: System, Light, or Dark appearance, plus larger text sizes across the whole interface.</li>
</ul>

<h2 class="section-heading">Thoughtful by design</h2>
<ul class="st-list">
    <li>Works with your existing Apple Reminders — everything you've already captured stays right where it is. Two-way, in place.</li>
    <li>Calm visual design that gets out of the way. Big, legible text. Nothing flashing at you.</li>
    <li>Accessible — designed with clear hit targets, complete descriptions and labels, and full support for Apple's rendering commands.</li>
</ul>

<h2 class="section-heading">Built for quiet productivity</h2>
<p>Whether your to-do list is a source of anxiety or just a source of clutter, SingleThread gives you a smaller, softer way through it. One item at a time, in an order you choose, at the pace that feels right. Enable the features that help, hide the ones that don't.</p>
<p>SingleThread works with your existing Apple Reminders and syncs across your devices. Available on iPhone, iPad, and Mac, with a matching Apple Watch app and macOS widget; install on any one to get going. No account to create — your reminders stay in place.</p>
<p class="st-closing">Your reminders. One at a time. In order. At your pace.</p>
```

(Section headings reuse the existing `.section-heading` class — muted, 1.25rem — per
the design's "reuse or clone" instruction; no duplication needed.)

#### 2. CSS: list + closing styles
**File**: `static/site.css`
**Action**: modify — append after the watch rules

```css
.st-list li::marker {
    color: var(--accent); /* accent bullets, token-based */
}

.st-list li strong {
    color: var(--text);
}

.st-closing {
    font-size: 1.25rem;
    color: var(--accent);
    text-align: center;
    margin-top: 3rem;
}
```

#### 3. Handler test: rewrite around stable phrases
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify — final form of `index_serves_ok_html` body assertions:

```rust
let body = res.text().await.unwrap();
assert!(body.contains("<title>SingleThread</title>"));
assert!(body.contains("<h1>SingleThread</h1>"));
assert!(body.contains("Your brain does one thing at a time")); // hero tagline
assert!(body.contains("One at a time."));                      // first bullet lead-in
assert!(body.contains("Why it helps"));
assert!(body.contains("Everything you need, nothing you don't"));
assert!(body.contains("Thoughtful by design"));
assert!(body.contains("Built for quiet productivity"));
assert!(body.contains("Your reminders. One at a time. In order. At your pace."));
assert!(body.contains(r#"<img src="/static/singlethread-shot-main.jpg?v="#));
assert!(body.contains(r#"<img src="/static/singlethread-shot-settings.jpg?v="#));
assert!(body.contains(r#"<img src="/static/singlethread-shot-swipe.jpg?v="#));
assert!(body.contains(r#"<img src="/static/singlethread-watch-list.png?v="#));
assert!(body.contains(r#"<img src="/static/singlethread-watch-detail.png?v="#));
assert!(body.contains(r#"<a href="/">Home</a>"#));
assert!(body.contains(r#"<a href="/singlethread">SingleThread</a>"#));
```

Note `"nothing you don't"` contains a straight apostrophe — minijinja auto-escape
doesn't touch apostrophes, so `body.contains(...)` matches raw template text.

### Verification
#### Automated
- [ ] `./scripts/test.sh` passes (fmt, sqlx prepare check, clippy, tests, TODO grep)

#### Manual
- [ ] Read rendered `/singlethread` top-to-bottom against "Frozen copy" — every paragraph, bullet, and heading present verbatim
- [ ] Final visual review on the live server at desktop and phone widths; if page weight feels heavy, downscaling is an asset-only follow-up swap (hash-based cache busting makes it trivial)
- [ ] Confirm home page (`/`) still renders unchanged (no shared CSS classes were renamed)

---

## Testing Checkpoints (from structure.md)
- **After Phase 1**: five PNG/JPG assets served via `/static` with correct content-type and immutable caching; page renders exactly as before.
- **After Phase 2**: hero with versioned hero image; old copy assertions fully replaced; mobile stacking works.
- **After Phase 3**: all five assets referenced via `asset_url` and asserted in the handler test.
- **After Phase 4**: full copy present; `./scripts/test.sh` green end-to-end; live-server visual review.

## Notes
- ROUTES.md needs no edit (content-only change, confirmed in research — route, method,
  status codes, and error paths are unchanged).
- `home/web.rs:49` nav assertion is unaffected (layout nav untouched).
- Metrics label `"singlethread"` untouched.
