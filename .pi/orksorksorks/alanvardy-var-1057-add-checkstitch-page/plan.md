# Implementation Plan

## Overview

Add a `/checkstitch` marketing page to the axum web app for the CheckStitch iOS
app, mirroring `templates/singlethread.html` and
`src/interfaces/handlers/singlethread/web.rs` end-to-end — same layout, hero,
platform badges, screenshot gallery, feature sections, FAQ widget and shared
Tailwind classes — but with CheckStitch copy, CheckStitch screenshots from
Linear ticket VAR-1057 (3 iPhone + 2 iPad + 2 Watch), and **no App Store badge
block at either the hero or the closing CTA** (CheckStitch has no App Store
presence yet). Additive only: one new handler module, one new template, one
route, one nav link, eight static assets, one `ROUTES.md` block, and updated
nav assertions.

Approved decisions (from the planning Q&A):
- **Q1 → A**: use all 7 ticket screenshots. 3-phone row, then **two** iPad
  figures side-by-side in the "Everything you need" section, then the 2-up
  Apple Watch row.
- **Q2 → A**: author CheckStitch-specific FAQ copy from the app's real
  behaviour (verbatim copy is given below; the implementer invents nothing).
- Reuse SingleThread's exact Tailwind class strings so `static/site.css` is
  **not** regenerated. Any class string not already present in
  `templates/singlethread.html` risks failing the gate's CSS-drift check —
  do not invent arbitrary-value variants.
- Phone/iPad screenshots convert to `.jpg`; watch shots and the icon stay
  `.png`; sizes match SingleThread's (phone max edge 1306, iPad max edge 1024,
  watch 410x502 as supplied, icon 256x256).

### Preconditions (fresh worktree)

`.env` already exists in this worktree; `test.db` does **not**. Before the
first gate run:

```bash
cd /Users/vardy/dev/alanvardy-var-1057-add-checkstitch-page
cargo sqlx database create
cargo sqlx migrate run
```

Never let a `cargo sqlx prepare` run against a blank DB — it can delete the
committed `.sqlx/` metadata. If that happens, `git checkout -- .sqlx`.

### Boundary confirmation

No schema change, no migration, no API contract change, no new subsystem. The
only shared-code edit is the nav in `templates/layout.html`. MEDIUM holds.

---

## Phase 1: Walking skeleton — `/checkstitch` renders with a nav entry

Thinnest end-to-end path: the icon asset exists, the handler renders a real
template, the route answers 200 with HTML, and the shared nav shows the new
link as active.

### Changes

#### 1. Static icon asset

**File**: `static/checkstitch-icon.png`
**Action**: create

```bash
cp /Users/vardy/dev/CheckStitch/CheckStitch/Assets.xcassets/AppIcon.appiconset/icon_256x256.png \
   static/checkstitch-icon.png     # verified 256x256, matches singlethread-icon.png
```

#### 2. New handler module

**File**: `src/interfaces/handlers/checkstitch/mod.rs`
**Action**: create

```rust
pub mod web;
```

#### 3. Handler

**File**: `src/interfaces/handlers/checkstitch/web.rs`
**Action**: create

Copy the module skeleton from `src/interfaces/handlers/singlethread/web.rs`
(lines 1-7 imports, 9-17 structs, 142-166 handler). Phase 1 carries **no**
`FAQS` const and **no** `faq_categories` context entry — those arrive in
Phase 3.

```rust
use axum::{extract::State, response::Html};
use minijinja::context;

use crate::app::error::WebError;
use crate::app::picture;
use crate::app::state::AppState;

pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    state.metrics.inc_page_view("checkstitch");
    let (wallpaper_url, photographer, photographer_url) = picture::wallpaper_context(&state).await;
    let html = state.templates.get_template("checkstitch.html")?.render(
        context! { wallpaper_url, photographer, photographer_url, active_page => "checkstitch" })?;
    Ok(Html(html))
}
```

Plus the Phase-1 test module (see Tests below). The `html_escape` helper is
**not** added in Phase 1 — it is deferred to Phase 3 together with the FAQ
tests that use it (an unused helper in Phase 1 trips rustc `dead_code`, which
fails the clippy `-D warnings` gate). Phase 3 adds it.

#### 4. Register the module

**File**: `src/interfaces/handlers/mod.rs`
**Action**: modify

```rust
pub mod checkstitch;
pub mod contact;
pub mod dump;
pub mod home;
pub mod metrics;
pub mod singlethread;
pub mod unsplash;
```

#### 5. Route

**File**: `src/interfaces/routes.rs`
**Action**: modify

Directly after `.route("/singlethread", get(handlers::singlethread::web::index))`
(currently line 49):

```rust
        .route("/singlethread", get(handlers::singlethread::web::index))
        .route("/checkstitch", get(handlers::checkstitch::web::index))
        .route("/contact", get(handlers::contact::web::index))
```

#### 6. New template

**File**: `templates/checkstitch.html`
**Action**: create

Phase 1 renders the hero/divider/intro/closing CTA only; the gallery and
feature sections are added in Phase 2, the FAQ block in Phase 3. There is
**no** App Store badge block anywhere — do not copy those two blocks from
`singlethread.html`.

```html
{% extends "layout.html" %}
{% block title %}CheckStitch{% endblock %}
{% block heading %}CheckStitch{% endblock %}
{% block content %}
<div class="hero">
    {# Icon hero: two-column asymmetrical flex — left has tagline + explanation, right has the app icon badge #}
    <div class="flex flex-col md:flex-row gap-8 items-center">
        <div class="space-y-4 md:flex-1">
            <p class="heading-hero">Your list is how you think. Reminders is where you act.</p>
            <p class="text-muted">CheckStitch turns a checklist into Apple Reminders — one reminder per item, in one tap.</p>
        </div>
        <div class="order-first md:order-none md:flex-none">
            <img src="{{ asset_url('checkstitch-icon.png') }}" alt="CheckStitch app icon"
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
    {# No App Store badge: CheckStitch has no App Store presence yet. #}
</div>

<div class="divider"></div>

<h2 class="heading-section">Write the list once. Run it as reminders.</h2>
<p>CheckStitch is the missing bridge between the list you wrote and the reminders you actually need. Name a checklist, add your items, and run it: each item becomes its own Apple Reminder, in the list you choose. No more typing the same thing ten times, and no more half-finished lists lost in Notes. Stitch the two together and get on with your day.</p>

<p class="text-2xl text-accent text-center mt-12">One checklist in. A list of reminders out.</p>
{% endblock %}
```

#### 7. Shared nav link

**File**: `templates/layout.html`
**Action**: modify

After the SingleThread link (line 25):

```html
        <a href="/singlethread"{% if active_page == "singlethread" %} class="active"{% endif %}>SingleThread</a>
        <a href="/checkstitch"{% if active_page == "checkstitch" %} class="active"{% endif %}>CheckStitch</a>
        <a href="/contact"{% if active_page == "contact" %} class="active"{% endif %}>Contact</a>
```

#### 8. Nav assertions in home/contact

**Files**: `src/interfaces/handlers/home/web.rs` (line 61),
`src/interfaces/handlers/contact/web.rs` (line 113)
**Action**: modify

Both currently assert the SingleThread nav link is present; adding a sibling
link cannot break them, but the shared nav is only proven to carry a
CheckStitch entry if asserted. Add one line after the existing SingleThread
assert in each file:

```rust
        assert!(body.contains(r#"<a href="/checkstitch">CheckStitch</a>"#));
```

(`home` asserts the Home link is `class="active"` and the CheckStitch link is
not, so the plain form above is correct in both files.)

#### 9. Route documentation

**File**: `ROUTES.md`
**Action**: modify

Insert a new block after the `### GET /singlethread` block's closing `---`
(currently line 33), before `### GET /contact`. Mirror the SingleThread block
exactly, minus the App Store badge sentence, pointing at the new template:

```markdown
### GET /checkstitch

Renders the CheckStitch page with an app-icon hero (icon badge + tagline),
platform badges (iPhone, iPad, Mac, Watch), a gradient decorative divider,
screenshot and watch-image cards with hover transitions, feature lists, an
FAQ section with collapsible Q&A pairs (native <details>/<summary> widgets), and a
closing CTA line. There is no App Store download badge. Includes a random
Unsplash wallpaper and photographer credit (linked name when a profile URL is
available, plain text otherwise). The wallpaper and credit gracefully degrade
to hidden when the Unsplash fetch fails.

- Response: `200 OK` — `text/html` (minijinja `templates/checkstitch.html`)
- Errors: `500` via `WebError` (template render failure)
- Rate limit: global per-IP GCRA limiter. Over limit → `429 Too Many Requests`,
  plain-text body `too many requests`, with `Retry-After` and `X-RateLimit-*` headers.

---
```

### Tests

Inline `#[cfg(test)] mod tests` in `src/interfaces/handlers/checkstitch/web.rs`.
Copy the three wallpaper/credit tests from `singlethread/web.rs` and adapt the
URL, title and hero assertions.

```rust
#[cfg(test)]
mod tests {
    use crate::test::{
        seed_wallpaper_no_url, start_app, start_app_with, start_unsplash_stub, test_client,
    };
    use axum::http::StatusCode;

    #[tokio::test]
    async fn index_serves_ok_html() {
        // happy path
        // 200 + text/html, <title>CheckStitch</title>, <h1>CheckStitch</h1>,
        // hero tagline "Your list is how you think",
        // <a href="/checkstitch" class="active">CheckStitch</a>,
        // closing line "One checklist in. A list of reminders out.",
        // assert NO app store badge:
        //   !body.contains("apps.apple.com") && !body.contains("app-store.svg")
        // escaped wallpaper URL form (&#x2f;), "Photo by", "hidden md:block", "bg-black/50"
    }

    #[tokio::test]
    async fn index_still_renders_when_wallpaper_fetch_fails() {
        // sad path: identical to singlethread/web.rs:267-285 with the /checkstitch URL
    }

    #[tokio::test]
    async fn index_shows_credit_as_text_when_no_photographer_url() {
        // sad path: identical to singlethread/web.rs:287-306 with the /checkstitch URL
    }
}
```

Do **not** carry over SingleThread's `assert_eq!(body.matches(app_store_href).count(), 2)`
or `/static/app-store.svg?v=` count assertions — the happy-path test above
asserts their absence instead.

Note: the `html_escape` helper shown in SingleThread's module is **omitted**
here in Phase 1 (it would be unused and trip the clippy `-D warnings`
dead_code gate). It is added in Phase 3 together with the FAQ tests.

### Verification

#### Automated

- [x] `cd /Users/vardy/dev/alanvardy-var-1057-add-checkstitch-page && cargo sqlx database create && cargo sqlx migrate run` succeeds (fresh worktree prerequisite) — test.db already present/migrated; gate ran clean
- [x] `cargo nextest run checkstitch` passes (3 tests)
- [x] `cargo nextest run home contact` passes (nav assertions unchanged/added)
- [x] `./scripts/test.sh` passes, including the `git diff --exit-code -- static/site.css` CSS-drift check

#### Manual

- [ ] `cargo run` then `curl -s localhost:<port>/checkstitch | head` shows `<title>CheckStitch</title>`, the nav link, no `apps.apple.com` and no `app-store.svg`
- [ ] `curl -s localhost:<port>/checkstitch | grep -c 'checkstitch-icon.png'` returns 1
- [ ] `/` and `/contact` still render, now with a CheckStitch nav entry

---

## Phase 2: Screenshot gallery and feature sections

Adds all seven ticket screenshots and the body of the page: the 3-phone row,
the two-up iPad block, the two feature/summary sections, the 2-up watch block,
and the immutable-caching route test.

### Changes

#### 1. Screenshot assets (7 files)

**Files**: `static/checkstitch-shot-main.jpg`,
`static/checkstitch-shot-edit.jpg`, `static/checkstitch-shot-settings.jpg`,
`static/checkstitch-shot-ipad.jpg`, `static/checkstitch-shot-ipad-edit.jpg`,
`static/checkstitch-watch-list.png`, `static/checkstitch-watch-create.png`
**Action**: create

The ticket's screenshots live behind authenticated Linear upload URLs. The
Linear API token works as a **raw** `Authorization` header (no `Bearer`
prefix) against `uploads.linear.app`. Write this as `/tmp/fetch_asssets.sh`
and run `bash /tmp/fetch_asssets.sh` (fish cannot run this):

```bash
#!/bin/bash
set -euo pipefail
cd /Users/vardy/dev/alanvardy-var-1057-add-checkstitch-page
tok="$(linear auth token)"
base='https://uploads.linear.app/ff0d3571-21e8-4208-922b-4e92f52dd966'
tmp="$(mktemp -d)"
fetch() { curl -fsSL -H "Authorization: $tok" "$base/$1" -o "$tmp/$2"; }

# Watch Ultra (410x502) -> png, as supplied
fetch '7dcb369c-4d82-4c9a-96e3-7b411773c709/92af9c0c-c13f-4c8d-b765-4201839ba2d8' watch-list.png
fetch '1bd1cc2b-a459-463d-a18e-fc836bbe188c/c9a654b2-9b6c-4c2c-918d-7809da2a80ef' watch-create.png
# iPhone (1284x2778)
fetch '247614bd-102d-41dc-8263-aa01f8837cc9/a59fc50d-8faf-4081-96d4-18df5ae55672' iphone-main.png
fetch 'a3f9ab09-0ef6-4222-b586-7ffc9875a9a1/f0230fe6-9f51-4205-9b16-414ce4d5818f' iphone-edit.png
fetch '58fba8f8-1d6e-4c08-b315-98c50bc0dacf/05847091-30e9-4c80-ade5-b5cfaf163d1d' iphone-settings.png
# iPad (2732x2048)
fetch 'f1271fa8-8ecd-4efa-8151-6cc3d8730eee/f6f06bf9-7abf-47e4-a0bf-7d2174508849' ipad-main.png
fetch '3e1bb115-ef35-4c40-9ace-cfb35e71b07b/388d2601-a7c6-4b0b-a4e8-5d957e573347' ipad-edit.png

# Downscale to SingleThread's sizes so the page stays light; JPEG for photos,
# PNG left untouched for the watch captures.
sips -s format jpeg -s formatOptions 80 -Z 1306 "$tmp/iphone-main.png"     --out static/checkstitch-shot-main.jpg
sips -s format jpeg -s formatOptions 80 -Z 1306 "$tmp/iphone-edit.png"     --out static/checkstitch-shot-edit.jpg
sips -s format jpeg -s formatOptions 80 -Z 1306 "$tmp/iphone-settings.png" --out static/checkstitch-shot-settings.jpg
sips -s format jpeg -s formatOptions 80 -Z 1024 "$tmp/ipad-main.png"       --out static/checkstitch-shot-ipad.jpg
sips -s format jpeg -s formatOptions 80 -Z 1024 "$tmp/ipad-edit.png"       --out static/checkstitch-shot-ipad-edit.jpg
cp "$tmp/watch-list.png"   static/checkstitch-watch-list.png
cp "$tmp/watch-create.png" static/checkstitch-watch-create.png
```

Verify before committing: `file static/checkstitch-*` — the three phone JPEGs
are ~604x1306, the two iPad JPEGs are 1024x768, the watch PNGs are 410x502,
and no file exceeds ~400 KB.

#### 2. Template body

**File**: `templates/checkstitch.html`
**Action**: modify

Insert the following between the existing `<p class="text-muted">` intro
paragraph (Phase 1, item 6) and the closing CTA paragraph. Every class string
below is copied verbatim from `singlethread.html` — do not introduce new
arbitrary-value classes.

```html
<div class="flex flex-wrap justify-center gap-6">
    <figure class="card m-0 flex-1 basis-[10rem] max-w-[14rem] p-3">
        <img src="{{ asset_url('checkstitch-shot-main.jpg') }}" alt="A list of checklists on iPhone"
             class="w-full rounded-lg border border-neutral-700">
        <figcaption class="text-muted text-sm mt-2 text-center">Your checklists</figcaption>
    </figure>
    <figure class="card m-0 flex-1 basis-[10rem] max-w-[14rem] p-3">
        <img src="{{ asset_url('checkstitch-shot-edit.jpg') }}" alt="Editing a checklist's items on iPhone"
             class="w-full rounded-lg border border-neutral-700">
        <figcaption class="text-muted text-sm mt-2 text-center">Edit any checklist</figcaption>
    </figure>
    <figure class="card m-0 flex-1 basis-[10rem] max-w-[14rem] p-3">
        <img src="{{ asset_url('checkstitch-shot-settings.jpg') }}" alt="The CheckStitch settings screen"
             class="w-full rounded-lg border border-neutral-700">
        <figcaption class="text-muted text-sm mt-2 text-center">Settings</figcaption>
    </figure>
</div>

<h2 class="heading-subsection">Why it helps</h2>
<ul class="list-disc pl-6 marker:text-accent space-y-2">
    <li><strong>One tap, a whole list.</strong> Running a checklist creates a reminder for every item at once, so the typing is finished before you start.</li>
    <li><strong>Your order survives.</strong> Number Reminders prefixes each title with its position — "1. Buy milk", "2. Call the plumber" — so the sequence you wrote is the sequence you see in Reminders.</li>
    <li><strong>Notes come along.</strong> Anything you type under an item rides into the reminder's notes, so the details stay attached to the task.</li>
</ul>

<h2 class="heading-subsection">Everything you need, nothing you don't</h2>
{# Two-column: feature list on the left, both iPad shots on the right. Deliberately uses the
   lg: breakpoint rather than md: so the pictures stay stacked under the copy on tablets. #}
<div class="flex flex-col lg:flex-row gap-8 lg:items-center">
    <div class="lg:flex-1">
        <p>CheckStitch does one job properly. Write a checklist, decide where it goes, and run it — as often as you like:</p>
        <ul class="list-disc pl-6 marker:text-accent space-y-2">
            <li>One checklist, many items. Each item becomes exactly one reminder, with the item's name as the title.</li>
            <li>Pick the destination. Send a checklist's reminders to your Reminders inbox or to any list you already have.</li>
            <li>Number Reminders, optionally. Prefix each title with its position when the order matters.</li>
            <li>Notes carry over. Anything you type under an item becomes the reminder's note.</li>
            <li>Duplicate and reuse. Turn a routine into a checklist and run it again next week, with no retyping.</li>
            <li>Export and import. Back up your checklists or move them between devices from Settings.</li>
            <li>Make it yours. Choose an interface style and, if you like, a background wallpaper.</li>
        </ul>
    </div>
    <div class="flex flex-wrap justify-center gap-6 lg:w-1/2 lg:flex-none">
        <figure class="card m-0 flex-1 basis-[10rem] max-w-[14rem] p-3">
            <img src="{{ asset_url('checkstitch-shot-ipad.jpg') }}" alt="Checklists with a background image on iPad"
                 class="w-full rounded-lg border border-neutral-700">
            <figcaption class="text-muted text-sm mt-2 text-center">On iPad</figcaption>
        </figure>
        <figure class="card m-0 flex-1 basis-[10rem] max-w-[14rem] p-3">
            <img src="{{ asset_url('checkstitch-shot-ipad-edit.jpg') }}" alt="Editing a checklist on iPad"
                 class="w-full rounded-lg border border-neutral-700">
            <figcaption class="text-muted text-sm mt-2 text-center">Editing a checklist</figcaption>
        </figure>
    </div>
</div>

<h2 class="heading-subsection">Thoughtful by design</h2>
<ul class="list-disc pl-6 marker:text-accent space-y-2">
    <li>Reminders are created through Apple Reminders and stay on your device or in your own iCloud account. They are never sent to the author or to any third party.</li>
    <li>CheckStitch only ever creates reminders. It never reads, edits, completes, or deletes them afterwards.</li>
    <li>Your checklists live on your device and sync through your own iCloud — no account to create, nothing to sign in to.</li>
    <li>No analytics, no tracking, and no advertising. The only network traffic is the optional background wallpaper.</li>
</ul>

<h2 class="heading-subsection">Built for quiet productivity</h2>
{# Two-column: Apple Watch shots on the left, copy on the right. Also lg: rather than
   md: so the pictures stay stacked on tablets instead of sitting side-to-side. #}
<div class="flex flex-col lg:flex-row gap-8 lg:items-center">
    <div class="flex justify-center gap-6 w-full lg:w-1/2 lg:flex-none">
        <figure class="card m-0 max-w-[12rem] p-3">
            <img src="{{ asset_url('checkstitch-watch-list.png') }}" alt="Apple Watch showing a list of checklists"
                 class="w-full rounded-lg border border-neutral-700">
            <figcaption class="text-muted text-sm mt-2 text-center">Your checklists</figcaption>
        </figure>
        <figure class="card m-0 max-w-[12rem] p-3">
            <img src="{{ asset_url('checkstitch-watch-create.png') }}" alt="Apple Watch showing the Create reminders button"
                 class="w-full rounded-lg border border-neutral-700">
            <figcaption class="text-muted text-sm mt-2 text-center">Create reminders</figcaption>
        </figure>
    </div>
    <div class="lg:flex-1">
        <p>CheckStitch works the same way everywhere. Build a checklist on your iPhone or iPad, review it on your Mac, and run it from your wrist when your hands are full — the checklist is there, waiting.</p>
        <p>Routines, packing lists, opening and closing procedures: anything you do more than once belongs in a checklist. Write it once, then let CheckStitch do the typing.</p>
    </div>
</div>
```

The "Everything you need" list lives in the section *before* the FAQ, matching
SingleThread's order. Phase 3's ordering test depends on this.

#### 3. Screenshot caching test

**File**: `src/interfaces/routes.rs`
**Action**: modify

Add a sibling to `singlethread_screenshots_are_served_with_immutable_caching`
(currently lines 214-247), with the same body and this case table:

```rust
    #[tokio::test]
    async fn checkstitch_screenshots_are_served_with_immutable_caching() {
        let addr = start_app().await;
        let client = test_client();
        let cases = [
            ("/static/checkstitch-shot-main.jpg", "image/jpeg"),
            ("/static/checkstitch-shot-edit.jpg", "image/jpeg"),
            ("/static/checkstitch-shot-settings.jpg", "image/jpeg"),
            ("/static/checkstitch-shot-ipad.jpg", "image/jpeg"),
            ("/static/checkstitch-shot-ipad-edit.jpg", "image/jpeg"),
            ("/static/checkstitch-watch-list.png", "image/png"),
            ("/static/checkstitch-watch-create.png", "image/png"),
        ];
        // for (path, content_type) in cases { 200, content-type match,
        // cache-control contains max-age=31536000, panic message naming path }
    }
```

Sad path — the immutable-caching loop's `panic!` message naming the failing
path is the failure mode; additionally assert a missing screenshot 404s:

```rust
    #[tokio::test]
    async fn missing_checkstitch_screenshot_is_not_found() {
        let addr = start_app().await;
        let res = test_client()
            .get(format!("http://{addr}/static/checkstitch-shot-nope.jpg"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
```

#### 4. Gallery assertions in the page test

**File**: `src/interfaces/handlers/checkstitch/web.rs`
**Action**: modify

Extend `index_serves_ok_html` with the mirrored screenshot assertions:

```rust
        assert!(body.contains("Why it helps"));
        assert!(body.contains("Everything you need, nothing you don't"));
        assert!(body.contains("Thoughtful by design"));
        assert!(body.contains("Built for quiet productivity"));
        assert!(body.contains(r#"<img src="/static/checkstitch-shot-main.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/checkstitch-shot-edit.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/checkstitch-shot-settings.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/checkstitch-shot-ipad.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/checkstitch-shot-ipad-edit.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/checkstitch-watch-list.png?v="#));
        assert!(body.contains(r#"<img src="/static/checkstitch-watch-create.png?v="#));
        assert!(body.contains("Apple Watch showing the Create reminders button"));
        // every asset_url reference must resolve — a missing file panics the handler
        assert_eq!(body.matches("app-store.svg").count(), 0);
```

### Verification

#### Automated

- [x] `cargo nextest run checkstitch` passes, including the screenshot-source asserts
- [x] `cargo nextest run immutable_caching` passes (both pages)
- [x] `cargo nextest run missing_checkstitch_screenshot` passes
- [x] `./scripts/test.sh` passes, including the CSS-drift check
- [x] `file static/checkstitch-shot-*.jpg static/checkstitch-watch-*.png` reports the expected dimensions

#### Manual

- [ ] `cargo run`, then open `/checkstitch`: hero icon, four platform badges, no App Store button, 3 phone cards, 2 iPad cards, 2 watch cards, all captions legible
- [ ] Browser devtools: each `static/checkstitch-*` request returns 200 with a `?v=` hash suffix
- [ ] `curl -sI localhost:<port>/static/checkstitch-shot-ipad.jpg` shows `cache-control: max-age=31536000`

---

## Phase 3: FAQ section

Adds the `FAQS` const, wires it into the render context, and drops the FAQ
markup into the template just above the closing CTA.

### Changes

#### 1. FAQ data and handler wiring

**File**: `src/interfaces/handlers/checkstitch/web.rs`
**Action**: modify

Add the two struct definitions plus the const, copied in shape from
`singlethread/web.rs:9-17`:

```rust
struct FaqCategory {
    title: &'static str,
    items: &'static [FaqItem],
}

struct FaqItem {
    question: &'static str,
    answer: &'static str,
}

const FAQS: &[FaqCategory] = &[ /* see copy below */ ];
```

and extend the handler render context:

```rust
    let faq_categories: Vec<serde_json::Value> = FAQS.iter().map(|category| {
        let items: Vec<serde_json::Value> = category.items.iter().map(|item|
            serde_json::json!({ "question": item.question, "answer": item.answer })).collect();
        serde_json::json!({ "title": category.title, "items": items })
    }).collect();
    let html = state.templates.get_template("checkstitch.html")?.render(
        context! { wallpaper_url, photographer, photographer_url, active_page => "checkstitch", faq_categories })?;
```

**Exact FAQ copy** (4 categories, 14 items — use verbatim):

```rust
const FAQS: &[FaqCategory] = &[
    FaqCategory {
        title: "Getting started",
        items: &[
            FaqItem {
                question: "How do I get CheckStitch?",
                answer: "CheckStitch is being prepared for its App Store release on iPhone, iPad, and Mac, with a matching Apple Watch app included. There is no account and no setup: install it, open it, and tap + to name your first checklist.",
            },
            FaqItem {
                question: "How do I run my first checklist?",
                answer: "Tap +, give the checklist a name, and add your items — each one gets a title and an optional note. Pick the Reminders list the items should go to, then tap Create reminders. One reminder is created for each item, and CheckStitch leaves them alone after that.",
            },
        ],
    },
    FaqCategory {
        title: "Features",
        items: &[
            FaqItem {
                question: "What exactly does CheckStitch create?",
                answer: "One Apple Reminder per item. The item's name becomes the reminder's title and the item's note becomes the reminder's note. Those reminders are then yours to use like any other — CheckStitch never reads, edits, completes, or deletes them afterwards.",
            },
            FaqItem {
                question: "Can I choose where the reminders go?",
                answer: "Yes. Every checklist has a destination list — your Reminders inbox or any list you already have. You can change it as often as you like while editing the checklist.",
            },
            FaqItem {
                question: "What is Number Reminders?",
                answer: "A toggle that prefixes each reminder's title with its position, like \"1. Buy milk\", so the order you wrote survives the trip into Reminders. Leave it off when the order doesn't matter.",
            },
            FaqItem {
                question: "Can I reuse a checklist?",
                answer: "That is the point. A checklist is a reusable template: go back to it next week, next month, or next Monday morning, and run it again to recreate the reminders without retyping a thing.",
            },
            FaqItem {
                question: "Does CheckStitch work on the Apple Watch?",
                answer: "Yes. Installing CheckStitch on your iPhone also installs a watch app, so you can read your checklists and create their reminders from your wrist without pulling out your phone.",
            },
            FaqItem {
                question: "Do my checklists sync between my devices?",
                answer: "Yes. Your checklists are stored on your device and synced through your own iCloud account, so the checklist you built on your iPhone is waiting for you on your iPad and Mac.",
            },
        ],
    },
    FaqCategory {
        title: "Privacy",
        items: &[
            FaqItem {
                question: "Do you collect my data?",
                answer: "No. CheckStitch has no analytics, no tracking, and no advertising. Your reminders and checklists are never sent to the author or to any third party.",
            },
            FaqItem {
                question: "Does CheckStitch read or change my existing reminders?",
                answer: "No. It only creates reminders from your checklist items. It never reads, edits, completes, or deletes reminders after they have been created.",
            },
            FaqItem {
                question: "Why does the app use the network at all?",
                answer: "Only for the optional background. The wallpaper and artist credit are fetched through a proxy at vardy.cc, and that request never includes any reminder, checklist, or preference data. With the background switched off, CheckStitch makes no network requests at all.",
            },
        ],
    },
    FaqCategory {
        title: "Other",
        items: &[
            FaqItem {
                question: "Can I back up my checklists?",
                answer: "Yes. Settings has Export and Import, so you can save your checklists to a file, or move them onto another device whenever you like.",
            },
            FaqItem {
                question: "Are you going to create an Android version?",
                answer: "There are no current plans for one. CheckStitch is built entirely on Apple Reminders and the Apple platforms. If this is something you would like, send me an email!",
            },
            FaqItem {
                question: "How do I get in touch?",
                answer: "Use the contact form or send an email. I read everything, and it is the fastest way to get a feature or a fix into the app.",
            },
        ],
    },
];
```

#### 2. FAQ markup

**File**: `templates/checkstitch.html`
**Action**: modify

Insert between the "Built for quiet productivity" block and the closing CTA
paragraph, copied verbatim from `singlethread.html`:

```html
<h2 class="heading-section">Frequently Asked Questions</h2>
{% for category in faq_categories %}
<h3 class="heading-subsection">{{ category.title }}</h3>
<div class="space-y-0">
    {% for item in category.items %}
    <details class="faq-item">
        <summary><span class="faq-chevron" aria-hidden="true"><svg viewBox="0 0 16 16" width="1.125em" height="1.125em" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" focusable="false"><path d="M6 4l4 4-4 4"/></svg></span>{{ item.question }}</summary>
        <p class="faq-answer text-muted mt-2">{{ item.answer }}</p>
    </details>
    {% endfor %}
</div>
{% endfor %}
```

### Tests

Add to the `tests` module in `src/interfaces/handlers/checkstitch/web.rs`,
mirroring `singlethread/web.rs:308-540` but with the CheckStitch copy. Add the
`all_faq_items()` helper and the `html_escape` helper here (Phase 1 deferred
`html_escape` to Phase 3, so Phase 3 must add it).

- [x] `faq_all_questions_appear` — every question, via `html_escape`
- [x] `faq_all_answers_appear` — every answer, via `html_escape`
- [x] `faq_privacy_disclosures_documented` — replaces SingleThread's
      `faq_app_intents_documented`; asserts the `Privacy` category heading plus
      `"Do you collect my data?"` and `"no analytics, no tracking, and no advertising"`.
      The exact answer text is asserted through `html_escape`.
- [x] `faq_section_after_quiet_productivity_before_cta` — index of
      `"Built for quiet productivity"` < index of `"Frequently Asked Questions"`
      < index of `"One checklist in. A list of reminders out."`
- [x] `faq_no_javascript` — no `<script` and no `onclick`
- [x] `faq_summary_has_chevron` — every question is preceded by the closing
      `faq-chevron` span tag
- [x] `faq_items_grouped_under_category_headings` — each of the four category
      headings appears, and the first item of each category follows its heading
- [x] `faq_items_all_non_empty` (plain `#[test]`) — no empty question/answer
- [x] `faq_items_no_duplicate_questions` (plain `#[test]`) — no repeated question

Sad-path value: `faq_items_all_non_empty` and `faq_items_no_duplicate_questions`
guard the data, and `faq_section_after_quiet_productivity_before_cta` guards
the layout contract.

### Verification

#### Automated

- [x] `cargo nextest run checkstitch` passes (all page + FAQ tests)
- [x] `./scripts/test.sh` passes end to end, including the CSS-drift check, clippy `-D warnings`, and the TODO grep
- [x] `git status --short` shows no modification to `static/site.css` (proof no new Tailwind class was introduced)
- [x] `git log --oneline -3` shows one commit per phase

#### Manual

- [ ] `cargo run`, open `/checkstitch`: the FAQ renders 4 categories and 14 collapsible `<details>` items; each opens and closes without JavaScript
- [ ] The closing CTA reads "One checklist in. A list of reminders out." and no App Store badge follows it
- [ ] `/` and `/contact` nav shows Home, SingleThread, CheckStitch, Contact; the active page is highlighted on each

---

## Commit sequence

One commit per phase, all on `alanvardy-var-1057-add-checkstitch-page` (never
push to `main`):

1. `feat: add CheckStitch page skeleton and nav link`
2. `feat: add CheckStitch screenshots and feature sections`
3. `feat: add CheckStitch FAQ`