# Implementation Plan

## Overview

Extend both page handlers to pass `photographer` + `photographer_url` from the existing `Picture` struct into template context, and render a fixed-position credit overlay in `layout.html` — no new route, no schema changes, no hand-written CSS.

---

## Phase 1: Core credit line — both pages, happy path

### Changes

#### 1. Home handler: pass photographer data into context
**File**: `src/interfaces/handlers/home/web.rs`
**Action**: modify

Replace the single-field `.map(|p| p.url)` with a destructure that captures all three fields from `Picture` and passes them into `context!`:

```rust
// Before (line 16):
let wallpaper_url = picture::current(&state).await.ok().map(|p| p.url);

// After:
let (wallpaper_url, photographer, photographer_url) = picture::current(&state)
    .await
    .ok()
    .map(|p| (p.url, p.photographer, p.photographer_url))
    .unwrap_or_default();
```

Update the `context!` call (line 20) from:

```rust
.render(context! { wallpaper_url })?
```

to:

```rust
.render(context! { wallpaper_url, photographer, photographer_url })?
```

#### 2. SingleThread handler: same change
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify

Identical changes to the `index` function — same code as the home handler. Replace the same two lines (`.map(|p| p.url)` and `context! { wallpaper_url }`).

#### 3. Template: add credit overlay to `layout.html`
**File**: `templates/layout.html`
**Action**: modify

After the wallpaper `<div>` (line 10), add the credit block:

```html
{% if photographer %}
<div class="fixed bottom-3 right-3 px-3 py-1.5 rounded bg-black/50 text-sm" aria-hidden="true">
  Photo by {% if photographer_url %}<a href="{{ photographer_url }}" target="_blank" rel="noopener noreferrer" class="underline">{{ photographer }}</a>{% else %}{{ photographer }}{% endif %} on Unsplash
</div>
{% endif %}
```

The full `<body>` opening becomes:

```html
<body>
    <div class="wallpaper" aria-hidden="true" {% if wallpaper_url %}style="background-image: url('{{ wallpaper_url }}')"{% endif %}></div>
    {% if photographer %}
    <div class="fixed bottom-3 right-3 px-3 py-1.5 rounded bg-black/50 text-sm" aria-hidden="true">
      Photo by {% if photographer_url %}<a href="{{ photographer_url }}" target="_blank" rel="noopener noreferrer" class="underline">{{ photographer }}</a>{% else %}{{ photographer }}{% endif %} on Unsplash
    </div>
    {% endif %}
    <nav>
```

#### 4. Test harness: populate `photographer_url` in `seed_wallpaper`
**File**: `src/test/mod.rs`
**Action**: modify

Extend the INSERT to include `photographer_url` so existing tests get a linked credit without changes:

```rust
// Before (line 137-138):
"INSERT INTO unsplash_pictures (url, photographer) \
 VALUES ('https://example.com/wallpaper.jpg', 'Wallpaper Photographer')",

// After:
"INSERT INTO unsplash_pictures (url, photographer, photographer_url) \
 VALUES ('https://example.com/wallpaper.jpg', 'Wallpaper Photographer', 'https://unsplash.com/@test')",
```

#### 5. Home handler tests: add credit assertions to `index_serves_ok_html`
**File**: `src/interfaces/handlers/home/web.rs`
**Action**: modify

Add assertions at the end of the `index_serves_ok_html` test (before the closing `}`):

```rust
// credit line appears with linked photographer name
assert!(body.contains("Photo by"));
assert!(body.contains("Wallpaper Photographer"));
assert!(body.contains(r#"href="https://unsplash.com/@test""#));
assert!(body.contains("on Unsplash"));
```

#### 6. SingleThread handler tests: add credit assertions to `index_serves_ok_html`
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify

Same four assertions added to the `index_serves_ok_html` test (before the closing `}`):

```rust
// credit line appears with linked photographer name
assert!(body.contains("Photo by"));
assert!(body.contains("Wallpaper Photographer"));
assert!(body.contains(r#"href="https://unsplash.com/@test""#));
assert!(body.contains("on Unsplash"));
```

#### 7. Regenerate `static/site.css`
**File**: `static/site.css`
**Action**: regenerate via `./scripts/build-css.sh`

Run `./scripts/build-css.sh`. New Tailwind utility classes (`fixed`, `bottom-3`, `right-3`, `px-3`, `py-1.5`, `rounded`, `bg-black/50`, `text-sm`, `underline`) are picked up by Tailwind's content scanning of `templates/layout.html`.

### Verification

#### Automated
- [x] `./scripts/test.sh` passes
- [x] `cargo test` passes
- [x] `index_serves_ok_html` (home) passes with new credit assertions
- [x] `index_serves_ok_html` (singlethread) passes with new credit assertions
- [x] `git diff --exit-code -- static/site.css` passes

#### Manual
- [ ] `cargo run` → open `http://localhost:3000/` — credit pill visible bottom-right with semi-transparent dark backdrop, "Photo by Wallpaper Photographer" linked, "on Unsplash" text
- [ ] `cargo run` → open `http://localhost:3000/singlethread` — same credit pill visible
- [ ] Link opens photographer's Unsplash profile in a new tab (`target="_blank"`)

---

## Phase 2: Edge-case hardening

### Changes

#### 1. Test harness: add `seed_wallpaper_no_url` helper
**File**: `src/test/mod.rs`
**Action**: modify

Add after the existing `seed_wallpaper` function (line 141):

```rust
/// Insert a wallpaper row with a photographer name but no photographer_url,
/// so tests can assert the credit renders as plain text (no broken link).
pub async fn seed_wallpaper_no_url(db: &SqlitePool) {
    sqlx::query(
        "INSERT INTO unsplash_pictures (url, photographer) \
         VALUES ('https://example.com/wallpaper.jpg', 'NoLink Photographer')",
    )
    .execute(db)
    .await
    .expect("seed wallpaper no url");
}
```

#### 2. Home handler tests: add `index_shows_credit_as_text_when_no_photographer_url`
**File**: `src/interfaces/handlers/home/web.rs`
**Action**: modify

Update the import to include `seed_wallpaper_no_url`:

```rust
use crate::test::{seed_wallpaper_no_url, start_app, start_app_with, start_unsplash_stub, test_client};
```

Add the test after the existing `index_renders_wallpaper_from_cache` test:

```rust
#[tokio::test]
async fn index_shows_credit_as_text_when_no_photographer_url() {
    let (addr, db) = start_app_with("https://api.unsplash.com").await;
    sqlx::query("DELETE FROM unsplash_pictures")
        .execute(&db)
        .await
        .expect("clear pictures");
    seed_wallpaper_no_url(&db).await;
    let body = test_client()
        .get(format!("http://{addr}/"))
        .send()
        .await
        .expect("request failed")
        .text()
        .await
        .expect("body");
    assert!(body.contains("Photo by NoLink Photographer on Unsplash"));
    // The name must NOT be wrapped in a link
    assert!(!body.contains(r#"" >NoLink Photographer</a>"#));
}
```

#### 3. SingleThread handler tests: same test
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify

Update the import:

```rust
use crate::test::{seed_wallpaper_no_url, start_app, start_app_with, start_unsplash_stub, test_client};
```

Add the identical test after `index_still_renders_when_wallpaper_fetch_fails`:

```rust
#[tokio::test]
async fn index_shows_credit_as_text_when_no_photographer_url() {
    let (addr, db) = start_app_with("https://api.unsplash.com").await;
    sqlx::query("DELETE FROM unsplash_pictures")
        .execute(&db)
        .await
        .expect("clear pictures");
    seed_wallpaper_no_url(&db).await;
    let body = test_client()
        .get(format!("http://{addr}/singlethread"))
        .send()
        .await
        .expect("request failed")
        .text()
        .await
        .expect("body");
    assert!(body.contains("Photo by NoLink Photographer on Unsplash"));
    assert!(!body.contains(r#"" >NoLink Photographer</a>"#));
}
```

#### 4. Home handler tests: extend `index_still_renders_when_wallpaper_fetch_fails`
**File**: `src/interfaces/handlers/home/web.rs`
**Action**: modify

Add one assertion after the existing `assert!(!body.contains("background-image"))`:

```rust
assert!(!body.contains("background-image"));
assert!(!body.contains("Photo by"));
```

#### 5. SingleThread handler tests: extend `index_still_renders_when_wallpaper_fetch_fails`
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify

Same additional assertion:

```rust
assert!(!body.contains("background-image"));
assert!(!body.contains("Photo by"));
```

### Verification

#### Automated
- [x] `./scripts/test.sh` passes — all Phase 2 tests pass
- [x] `index_shows_credit_as_text_when_no_photographer_url` (home) passes
- [x] `index_shows_credit_as_text_when_no_photographer_url` (singlethread) passes
- [x] `index_still_renders_when_wallpaper_fetch_fails` (home) asserts no `Photo by` in body
- [x] `index_still_renders_when_wallpaper_fetch_fails` (singlethread) asserts no `Photo by` in body

#### Manual
- [ ] `cargo run` → all pages render with credit (happy path from Phase 1 still holds)
- [ ] No regression: `/dump` and `/health` do not include the credit (they don't extend `layout.html`)

---

## Testing Checkpoints

| After Phase | What must be true |
|---|---|
| Phase 1 | `./scripts/test.sh` green. GET `/` and `/singlethread` both contain `Photo by Wallpaper Photographer` with a link to `https://unsplash.com/@test`. |
| Phase 2 | `./scripts/test.sh` green. `Photo by` absent when wallpaper fails. `Photo by NoLink Photographer` renders as plain text (no `<a href>`). |