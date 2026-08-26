# Implementation Plan

## Overview

Bold template+CSS redesign of `/singlethread` and `/contact` plus upgraded shared chrome — no handler, route, or behavior changes. All work flows through `css/site.css` → Tailwind compile → `static/site.css` with zero JavaScript. Four vertical slices, each crossing CSS → template → tests.

---

## Phase 1: Design System + Chrome Upgrade

Establish the visual foundation and upgrade the nav bar / wallpaper treatment shared by all pages.

### Changes

#### 1. CSS component classes
**File**: `css/site.css`
**Action**: modify

Add a `@layer components` block after the existing `@layer base` closing brace. The new block defines all reusable component classes. All classes use plain CSS with theme custom properties (no `@apply`, matching the existing codebase convention).

```css
@layer components {
    .hero {
        max-width: 64rem;
    }

    .card {
        background: var(--color-surface);
        border: 1px solid var(--color-border);
        border-radius: var(--radius-lg);
        transition: border-color 200ms;
    }

    .card:hover {
        border-color: var(--color-accent);
    }

    .badge {
        display: inline-block;
        padding: 0.25rem 0.75rem;
        border-radius: 9999px;
        font-size: 0.875rem;
        font-weight: 500;
        background: color-mix(in srgb, var(--color-accent) 15%, transparent);
        color: var(--color-accent-strong);
    }

    .divider {
        width: 100%;
        height: 1px;
        margin: 2rem 0;
        background: linear-gradient(to right, var(--color-accent), var(--color-accent-strong), transparent);
    }

    .container-wide {
        max-width: 64rem;
    }

    .btn {
        border-radius: var(--radius-DEFAULT);
        background: var(--color-accent);
        color: var(--color-bg);
        font-weight: 600;
        padding: 0.625rem 1.25rem;
        transition: background-color 200ms;
    }

    .btn:hover {
        background: var(--color-accent-strong);
    }

    .bg-gradient-accent {
        background: linear-gradient(to right, var(--color-accent), var(--color-accent-strong));
    }

    .heading-hero {
        color: var(--color-accent);
        font-size: var(--text-3xl);
    }

    .heading-section {
        color: var(--color-muted);
        margin-top: 2.5rem;
        margin-bottom: 1rem;
        font-size: var(--text-2xl);
    }

    .heading-subsection {
        color: var(--color-muted);
        margin-top: 2rem;
        margin-bottom: 0.75rem;
        font-size: var(--text-xl);
    }

    .transition-interactive {
        transition: color 200ms, background-color 200ms, border-color 200ms;
    }

    .form-input {
        width: 100%;
        border-radius: var(--radius-DEFAULT);
        border: 1px solid var(--color-border);
        background: var(--color-surface);
        color: var(--color-text);
        padding: 0.75rem;
        transition: border-color 200ms;
    }

    .form-input:focus {
        border-color: var(--color-accent);
        outline: none;
    }

    .form-label {
        color: var(--color-muted);
        font-size: 0.875rem;
        font-weight: 500;
    }

    nav a.active {
        border-bottom: 2px solid var(--color-accent);
    }
}
```

#### 2. Nav bar and wallpaper base-layer upgrade
**File**: `css/site.css`
**Action**: modify

In the existing `@layer base` block, update the `nav` and `nav a` rules and add a `.wallpaper::after` rule:

**`nav` rule** — replace the existing block (currently `display: flex; gap: 1.5rem; padding: 0.75rem 1.5rem; background: var(--color-surface); border-bottom: 1px solid var(--color-border);`):

```css
nav {
    display: flex;
    gap: 1.5rem;
    padding: 1rem 2rem;
    background: rgba(38, 38, 38, 0.85);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border-bottom: 1px solid var(--color-border);
}
```

**`nav a` rule** — add `transition-interactive` for hover polish:

```css
nav a {
    padding: 0.5rem 0;
    transition: color 200ms, background-color 200ms, border-color 200ms;
}
```

**New `.wallpaper::after` rule** — add after the existing `.wallpaper` rule (line 61-68). Inserts a subtle dark gradient overlay at the bottom edge to ground the photographer credit bubble:

```css
.wallpaper::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(to top, var(--color-bg), transparent 30%);
    pointer-events: none;
}
```

#### 3. Extract nav into `{% block nav %}` in layout.html
**File**: `templates/layout.html`
**Action**: modify

Replace the `<nav>` element (lines 25-27) with a `{% block nav %}` containing the default nav (no active indicator):

```html
{% block nav %}
<nav>
    <a href="/">Home</a>
    <a href="/singlethread">SingleThread</a>
    <a href="/contact">Contact</a>
</nav>
{% endblock %}
```

#### 4. Override nav block in home.html
**File**: `templates/home.html`
**Action**: modify

Add a `{% block nav %}` override before `{% block heading %}` (line 3), marking Home as active:

```html
{% block nav %}
<nav>
    <a href="/" class="active">Home</a>
    <a href="/singlethread">SingleThread</a>
    <a href="/contact">Contact</a>
</nav>
{% endblock %}
```

#### 5. Override nav block in singlethread.html
**File**: `templates/singlethread.html`
**Action**: modify

Add a `{% block nav %}` override before `{% block heading %}` (line 3), marking SingleThread as active:

```html
{% block nav %}
<nav>
    <a href="/">Home</a>
    <a href="/singlethread" class="active">SingleThread</a>
    <a href="/contact">Contact</a>
</nav>
{% endblock %}
```

#### 6. Override nav block in contact.html
**File**: `templates/contact.html`
**Action**: modify

Add a `{% block nav %}` override before `{% block heading %}` (line 3), marking Contact as active:

```html
{% block nav %}
<nav>
    <a href="/">Home</a>
    <a href="/singlethread">SingleThread</a>
    <a href="/contact" class="active">Contact</a>
</nav>
{% endblock %}
```

#### 7. Update nav assertions — home/web.rs tests
**File**: `src/interfaces/handlers/home/web.rs`
**Action**: modify

In `index_serves_ok_html` (lines 50-52), update the two nav assertions. Home link now has `class="active"`. Home has NO Contact nav assertion (keep current behavior).

Replace:
```rust
assert!(body.contains(r#"<a href="/">Home</a>"#));
assert!(body.contains(r#"<a href="/singlethread">SingleThread</a>"#));
```

With:
```rust
assert!(body.contains(r#"<a href="/" class="active">Home</a>"#));
assert!(body.contains(r#"<a href="/singlethread">SingleThread</a>"#));
```

#### 8. Update nav assertions — singlethread/web.rs tests
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify

In `index_serves_ok_html` (lines 52-53), update the two nav assertions. SingleThread link now has `class="active"`.

Replace:
```rust
assert!(body.contains(r#"<a href="/">Home</a>"#));
assert!(body.contains(r#"<a href="/singlethread">SingleThread</a>"#));
```

With:
```rust
assert!(body.contains(r#"<a href="/">Home</a>"#));
assert!(body.contains(r#"<a href="/singlethread" class="active">SingleThread</a>"#));
```

#### 9. Update nav assertions — contact/web.rs tests
**File**: `src/interfaces/handlers/contact/web.rs`
**Action**: modify

In `get_contact_returns_200_with_form` (lines 83-85), update the three nav assertions. Contact link now has `class="active"`.

Replace:
```rust
assert!(body.contains(r#"<a href="/">Home</a>"#));
assert!(body.contains(r#"<a href="/singlethread">SingleThread</a>"#));
assert!(body.contains(r#"<a href="/contact">Contact</a>"#));
```

With:
```rust
assert!(body.contains(r#"<a href="/">Home</a>"#));
assert!(body.contains(r#"<a href="/singlethread">SingleThread</a>"#));
assert!(body.contains(r#"<a href="/contact" class="active">Contact</a>"#));
```

### Verification

#### Automated
- [x] `./scripts/test.sh` passes (includes CSS drift check, format, type-check, clippy, tests)
- [x] All 3 page tests have updated nav assertions

#### Manual
- [ ] Nav bar is wider (1rem 2rem padding), has backdrop-filter blur, active page link has an orange bottom border
- [ ] Wallpaper has a subtle dark gradient at the bottom edge
- [ ] Home page renders with Home active in nav
- [ ] SingleThread page renders with SingleThread active in nav
- [ ] Contact page renders with Contact active in nav

---

## Phase 2: SingleThread Page Redesign

Rebuild `singlethread.html` using Phase 1 component classes.

### Changes

#### 1. Rewrite singlethread.html
**File**: `templates/singlethread.html`
**Action**: modify

Full structural rewrite. Replace the entire file content:

```html
{% extends "layout.html" %}
{% block nav %}
<nav>
    <a href="/">Home</a>
    <a href="/singlethread" class="active">SingleThread</a>
    <a href="/contact">Contact</a>
</nav>
{% endblock %}
{% block title %}SingleThread{% endblock %}
{% block heading %}SingleThread{% endblock %}
{% block content %}
<div class="hero">
    {# Icon hero: two-column asymmetrical flex — left has tagline + explanation, right has the app icon badge #}
    <div class="flex flex-col md:flex-row gap-8 items-center">
        <div class="space-y-4 md:flex-1">
            <p class="heading-hero">Your brain does one thing at a time — your list should too.</p>
            <p class="text-muted">SingleThread shows one Apple Reminder at a time for calm, focused momentum.</p>
        </div>
        <div class="order-first md:order-none md:flex-none">
            <img src="{{ asset_url('singlethread-icon.png') }}" alt="SingleThread app icon"
                 width="96" height="96" class="rounded-2xl">
        </div>
    </div>

    {# Platform badges #}
    <div class="flex flex-wrap gap-2 justify-center mt-6">
        <span class="badge">iPhone</span>
        <span class="badge">iPad</span>
        <span class="badge">Mac</span>
        <span class="badge">Watch</span>
    </div>
</div>

<div class="divider"></div>

<h2 class="heading-section">Your task list should match how your brain actually works — one thing at a time.</h2>
<p>SingleThread turns your Apple Reminders into a single, calm, focused list. Instead of an intimidating wall of everything you "should" do, you see one clear reminder at a time, in order, and you process it — complete, skip, or delete — before moving to the next. It's built for people who feel overwhelmed, who are neurodivergent, or who simply want their to-dos to stop feeling like noise.</p>

<div class="flex flex-wrap gap-6">
    <figure class="card m-0 flex-1 basis-[10rem] max-w-[14rem] p-3">
        <img src="{{ asset_url('singlethread-shot-main.jpg') }}" alt="One reminder at a time on iPhone"
             class="w-full rounded-lg border border-neutral-700">
    </figure>
    <figure class="card m-0 flex-1 basis-[10rem] max-w-[14rem] p-3">
        <img src="{{ asset_url('singlethread-shot-settings.jpg') }}" alt="The SingleThread settings sheet"
             class="w-full rounded-lg border border-neutral-700">
    </figure>
    <figure class="card m-0 flex-1 basis-[10rem] max-w-[14rem] p-3">
        <img src="{{ asset_url('singlethread-shot-swipe.jpg') }}" alt="Swiping to complete a reminder in light mode"
             class="w-full rounded-lg border border-neutral-700">
    </figure>
</div>

<h2 class="heading-subsection">On your wrist</h2>
<div class="flex justify-center gap-6">
    <div class="card max-w-[12rem] p-3">
        <img src="{{ asset_url('singlethread-watch-list.png') }}" alt="Apple Watch showing a reminder with Complete and Skip buttons"
             class="w-full rounded-lg border border-neutral-700">
    </div>
    <div class="card max-w-[12rem] p-3">
        <img src="{{ asset_url('singlethread-watch-detail.png') }}" alt="Apple Watch showing Refresh and Delete actions"
             class="w-full rounded-lg border border-neutral-700">
    </div>
</div>

<h2 class="heading-subsection">Why it helps</h2>
<ul class="list-disc pl-6 marker:text-accent space-y-2">
    <li><strong>One at a time.</strong> Your human brain does one thing at a time — your task list should reflect that. Just one reminder fills the screen, so every task gets your full attention.</li>
    <li><strong>Order that makes sense.</strong> Sort by priority, by due date (soonest first), or alphabetically. Pick whichever removes the most thinking.</li>
    <li><strong>No decision paralysis.</strong> See the reminder, act, move on. Fast. Smooth. Calm.</li>
</ul>

<h2 class="heading-subsection">Everything you need, nothing you don't</h2>
<p>SingleThread stays out of your way, but it's as full-featured — or as bare — as you want. Every capability can be turned on or off in one simple Settings screen:</p>
<ul class="list-disc pl-6 marker:text-accent space-y-2">
    <li>Complete, Skip, or Delete each reminder with a tap, a swipe, or the action buttons.</li>
    <li>Hide reminders from projects you don't want to see (with "Excluded Projects").</li>
    <li>Hide undated (someday) reminders, or show them when you're ready.</li>
    <li>Show or hide reminder dates — keep the screen as minimal as you like.</li>
    <li>Fast add by voice: dictate a reminder, complete with due date and recurrence, right from the microphone button (or hide it with one switch).</li>
    <li>Change how it looks: System, Light, or Dark appearance, plus larger text sizes across the whole interface.</li>
</ul>

<h2 class="heading-subsection">Thoughtful by design</h2>
<ul class="list-disc pl-6 marker:text-accent space-y-2">
    <li>Works with your existing Apple Reminders — everything you've already captured stays right where it is. Two-way, in place.</li>
    <li>Calm visual design that gets out of the way. Big, legible text. Nothing flashing at you.</li>
    <li>Accessible — designed with clear hit targets, complete descriptions and labels, and full support for Apple's rendering commands.</li>
</ul>

<h2 class="heading-subsection">Built for quiet productivity</h2>
<p>Whether your to-do list is a source of anxiety or just a source of clutter, SingleThread gives you a smaller, softer way through it. One item at a time, in an order you choose, at the pace that feels right. Enable the features that help, hide the ones that don't.</p>
<p>SingleThread works with your existing Apple Reminders and syncs across your devices. Available on iPhone, iPad, and Mac, with a matching Apple Watch app and macOS widget; install on any one to get going. No account to create — your reminders stay in place.</p>
<p class="text-2xl text-accent text-center mt-12">Your reminders. One at a time. In order. At your pace.</p>
{% endblock %}
```

#### 2. Update SingleThread test assertions
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify

In `index_serves_ok_html`, add two assertions. The icon is finally rendered in a template:

```rust
// After the existing image assertions (after line that checks singlethread-watch-detail.png):
assert!(body.contains(r#"<img src="/static/singlethread-icon.png?v="#));
```

Add the `home-columns` negative assertion alongside the existing negative checks (after the `section-heading` line):

```rust
assert!(!body.contains("home-columns"));
```

Full negative-assertion block after the change reads:
```rust
assert!(!body.contains("\"st-"));
assert!(!body.contains(" st-"));
assert!(!body.contains("section-heading"));
assert!(!body.contains("home-columns"));
```

No other test changes — all existing content assertions (`<h1>SingleThread</h1>`, `Your brain does one thing at a time`, `One at a time.`, `Why it helps`, etc.) remain valid because text content is unchanged.

### Verification

#### Automated
- [x] `./scripts/test.sh` passes
- [x] SingleThread tests pass with icon assert present
- [x] Negative assertions: no `"st-`, ` st-`, `section-heading`, `home-columns` in body

#### Manual
- [ ] SingleThread page renders with 96×96 app icon badge in hero
- [ ] Platform badges (iPhone, iPad, Mac, Watch) centered below hero
- [ ] Gradient divider between hero and content
- [ ] Screenshot figures are wrapped in `.card` with hover transition (border lights up accent on hover)
- [ ] Watch images are in `.card` wrappers
- [ ] Heading scale: hero is accent-colored text-3xl, main sections are text-2xl, subsections are text-xl
- [ ] Closing CTA is text-2xl accent

---

## Phase 3: Contact Page Redesign

Rebuild `contact.html` as a two-column layout with introductory copy and a polished form.

### Changes

#### 1. Rewrite contact.html
**File**: `templates/contact.html`
**Action**: modify

Full structural rewrite. Replace the entire file content:

```html
{% extends "layout.html" %}
{% block nav %}
<nav>
    <a href="/">Home</a>
    <a href="/singlethread">SingleThread</a>
    <a href="/contact" class="active">Contact</a>
</nav>
{% endblock %}
{% block title %}Contact{% endblock %}
{% block heading %}Contact{% endblock %}
{% block content %}
<div class="flex flex-col md:flex-row gap-8 items-start">
    {# Left column (top on mobile): introductory copy #}
    <div class="space-y-4 md:flex-1">
        <p class="text-muted">
            I'm Alan, a Senior Developer on Canada's West Coast.
            I build AI tools, backend Rust services, and Swift apps for Apple platforms.
        </p>
        <p class="text-muted">
            Whether you have a question about one of my projects, want to collaborate,
            or just want to say hi — I'd love to hear from you.
        </p>
    </div>

    {# Right column (bottom on mobile): form or thank-you #}
    <div class="md:flex-1">
        {% if submitted %}
            <p class="text-2xl text-accent">Thank you — I'll get back to you soon.</p>
        {% else %}
            <form action="/contact" method="post" class="flex flex-col gap-4">
                <div class="flex flex-col gap-2">
                    <label for="name" class="form-label">Name</label>
                    <input id="name" name="name" type="text" required class="form-input">
                </div>
                <div class="flex flex-col gap-2">
                    <label for="email" class="form-label">Email</label>
                    <input id="email" name="email" type="email" required class="form-input">
                </div>
                <div class="flex flex-col gap-2">
                    <label for="message" class="form-label">Message</label>
                    <textarea id="message" name="message" rows="6" required class="form-input"></textarea>
                </div>
                {# Honeypot: bots fill this, humans never see it (CSS-hidden, not type="hidden") #}
                <input type="text" name="_website" value="" tabindex="-1" autocomplete="off"
                       aria-hidden="true"
                       style="position:absolute;left:-9999px;width:1px;height:1px;overflow:hidden">
                <button type="submit" class="btn self-start">Send message</button>
            </form>
        {% endif %}
    </div>
</div>
{% endblock %}
```

#### 2. Update Contact test assertions
**File**: `src/interfaces/handlers/contact/web.rs`
**Action**: modify

In `get_contact_returns_200_with_form`, add an assertion for the intro copy text. Add after the existing `action="/contact"` assertion:

```rust
assert!(body.contains("I'm Alan"));
```

In `post_valid_form_sends_email`, add a content assertion for the thank-you message after the existing status+call_count checks (the test currently only checks `StatusCode::OK` and `stub.call_count`):

```rust
// After `assert_eq!(stub.call_count.load(Ordering::SeqCst), 1);`
let body = res.text().await.unwrap();
assert!(body.contains("Thank you — I'll get back to you soon."));
```

Note: Adding `res.text()` means the `res` variable can't be used after. The existing test doesn't read the body, so this is fine as the last assertion in that test.

### Verification

#### Automated
- [x] `./scripts/test.sh` passes
- [x] Contact GET test has intro-copy assertion (`I'm Alan`)
- [x] Contact POST success test has thank-you assertion
- [x] All form field assertions still pass: `name="name"`, `name="email"`, `name="message"`, `name="_website"`, `action="/contact"`

#### Manual
- [ ] Contact page renders as two columns on desktop (left: intro copy, right: form)
- [ ] Form inputs have the `.form-input` styling with focus ring (accent border on focus)
- [ ] Submit button uses `.btn` styling (accent background, rounded, hover darkens)
- [ ] Thank-you page renders as two columns (left: intro copy, right: confirmation message)
- [ ] Honeypot is still CSS-hidden

---

## Phase 4: ROUTES.md Sync

Update route descriptions to reflect the new page structure.

### Changes

#### 1. Update `GET /singlethread` description
**File**: `ROUTES.md`
**Action**: modify

Replace the `### GET /singlethread` block (from the `###` heading through the closing `---`). The new block:

```markdown
### GET /singlethread

Renders the SingleThread page with an app-icon hero (icon badge + tagline),
platform badges (iPhone, iPad, Mac, Watch), a gradient decorative divider,
screenshot and watch-image cards with hover transitions, feature lists, and a
closing CTA line. Includes a random Unsplash wallpaper and photographer credit
(linked name when a profile URL is available, plain text otherwise). The
wallpaper and credit gracefully degrade to hidden when the Unsplash fetch
fails.

- Response: `200 OK` — `text/html` (minijinja `templates/singlethread.html`)
- Errors: `500` via `WebError` (template render failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.

---
```

#### 2. Update `GET /contact` description
**File**: `ROUTES.md`
**Action**: modify

Replace the `### GET /contact` block:

```markdown
### GET /contact

Renders a two-column contact page: introductory copy about Alan in the left
column, and the contact form (name, email, message, CSS-hidden honeypot) in the
right column. Both columns stack vertically on mobile. Includes a random
Unsplash wallpaper and photographer credit that degrade to hidden when the
Unsplash fetch fails.

- Response: `200 OK` — `text/html` (minijinja `templates/contact.html`)
- Errors: `500` via `WebError` (template render failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.

---
```

#### 3. Update `POST /contact` description
**File**: `ROUTES.md`
**Action**: modify

Replace the `### POST /contact` block:

```markdown
### POST /contact

Accepts the contact form, skips email when the honeypot is filled (returns the
two-column thank-you page silently), otherwise sends the message to the
configured inbox via the Resend API and returns the two-column thank-you page
(introductory copy in the left column, confirmation message in the right).

- Request body: `application/x-www-form-urlencoded` (`name`, `email`, `message`,
  `_website` honeypot)
- Response: `200 OK` — `text/html` thank-you page
- Errors: `502` via `WebError` (Resend API failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.
- Rate limit: also a stricter dedicated tier (see
  `CONTACT_TIER_*` in `src/app/rate_limit.rs`) nested inside the global budget.

---
```

### Verification

#### Automated
- [x] `./scripts/test.sh` passes (no code changes to break — ROUTES.md is not tested)

#### Manual
- [ ] `### GET /singlethread` describes icon hero, platform badges, cards, dividers
- [ ] `### GET /contact` describes two-column layout with intro copy
- [ ] `### POST /contact` describes two-column thank-you page
- [ ] All three blocks end with `---` (correct cut point)

---

## Testing Checkpoints

| After Phase | What must be true |
|---|---|
| **1** | `./scripts/test.sh` passes. All 3 page tests have updated nav assertions. CSS compiles without drift. Home, SingleThread, and Contact pages all render with upgraded nav chrome. |
| **2** | `./scripts/test.sh` passes. SingleThread tests assert `singlethread-icon.png` in body. Cards, badges, dividers render. Other two pages unaffected. |
| **3** | `./scripts/test.sh` passes. Contact tests have intro-copy and thank-you assertions. Two-column layout renders for both form and thank-you states. Home and SingleThread unaffected. |
| **4** | ROUTES.md sections accurate. No code changes — no test gate impact. |