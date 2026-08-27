# Implementation Plan

## Overview

Add an FAQ section with collapsible Q&A pairs to the SingleThread page using native HTML `<details>/<summary>` widgets, driven by a `Vec<FaqItem>` static constant defined in the handler, styled with a minimal `.faq-item` component class in the shared CSS layer.

---

## Phase 1: Content Model — `FaqItem` struct + FAQ data

### Changes

#### 1. Define `FaqItem` struct and `FAQ_ITEMS` constant
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify — insert after `use crate::app::state::AppState;` (line 6), before `pub async fn index` (line 8)

```rust
struct FaqItem {
    question: &'static str,
    answer: &'static str,
}

const FAQ_ITEMS: &[FaqItem] = &[
    FaqItem {
        question: "Where is my data stored?",
        answer: "Your Reminders are stored on your phone and on iCloud in Apple Reminders. Your settings are stored on your device and in iCloud. I do not store any of your information myself. The only way that I will find out anything about you is if you email me.",
    },
    FaqItem {
        question: "Why did you choose Apple Reminders?",
        answer: "I chose Apple Reminders because it is free, has first class support on Apple devices, and is a pragmatic choice for many Apple users.",
    },
    FaqItem {
        question: "Are you going to create an Android version?",
        answer: "I'm not against the idea, but there are no current plans to do so. If this is something that you would like, send me an email!",
    },
    FaqItem {
        question: "Are you planning on supporting other task managers?",
        answer: "I have been toying with the idea of supporting more task managers, please let me know if this is something that you desire and for which task manager.",
    },
    FaqItem {
        question: "Where are the wallpapers from and how do you select them?",
        answer: "The wallpapers are from Unsplash. My server at vardy.cc fetches random nature wallpapers from their service and caches them. The app then gets the wallpapers from my server using no identifying information about you. This allows me to obscure my API key and keep the number of requests to Unsplash to a reasonable level.",
    },
    FaqItem {
        question: "Pulp or no pulp?",
        answer: "I try not to be too picky, but I definitely prefer pulp.",
    },
    FaqItem {
        question: "What network requests does this app make?",
        answer: "I only have the app perform network requests to fetch new wallpapers.",
    },
    FaqItem {
        question: "Does this app work off-line?",
        answer: "It sure does! The changes to your reminders are stored on your device and will be synced to iCloud when you're next online. During this time, you will not be able to fetch new wallpapers, but the app will degrade gracefully in this case.",
    },
    FaqItem {
        question: "Can I contact you with questions, bug reports, or feature requests?",
        answer: "I would appreciate it! Please use my contact form and I will read your email personally.",
    },
    FaqItem {
        question: "Is SingleThread free?",
        answer: "SingleThread is free to download and use with no ads, no accounts, and no subscriptions. The full feature set is available to everyone.",
    },
    FaqItem {
        question: "How do I get started?",
        answer: "Download SingleThread from the App Store on your iPhone, iPad, or Mac. It reads your existing Apple Reminders — no import, no setup, no account. Open the app and you'll see your first reminder right away. From there, tap Complete, Skip, or Delete, and the next one appears.",
    },
];
```

#### 2. Add unit tests for FAQ data
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify — add inside the existing `#[cfg(test)] mod tests` block, after the last test function (after line 97)

```rust
#[test]
fn faq_items_all_non_empty() {
    for (i, item) in FAQ_ITEMS.iter().enumerate() {
        assert!(!item.question.is_empty(), "FAQ item {i} has empty question");
        assert!(!item.answer.is_empty(), "FAQ item {i} has empty answer");
    }
}

#[test]
fn faq_items_count() {
    assert_eq!(FAQ_ITEMS.len(), 11);
}

#[test]
fn faq_items_no_duplicate_questions() {
    let mut seen = std::collections::HashSet::new();
    for item in FAQ_ITEMS {
        assert!(seen.insert(item.question), "Duplicate FAQ question: {}", item.question);
    }
}
```

Note: these are `#[test]` (not `#[tokio::test]`) — they are synchronous unit tests that only touch the static constant. No async runtime needed.

### Verification

#### Automated
- [x] `cargo test faq_items` — three new unit tests pass
- [x] `./scripts/test.sh` passes (all existing tests unchanged; new unit tests green)

#### Manual
- [ ] Review FAQ_ITEMS content for typos and factual accuracy
- [ ] Confirm "Is SingleThread free?" and "How do I get started?" answers are acceptable (these were drafted by the agent; the human should approve)

---

## Phase 2: CSS Component Layer — `.faq-item` class

### Changes

####1. Add `.faq-item` component class
**File**: `css/site.css`
**Action**: modify — append inside the `@layer components` block, after the `nav a.active` rule (after line 193)

```css
.faq-item {
    border-bottom: 1px solid var(--color-border);
}

.faq-item summary {
    cursor: pointer;
    color: var(--color-muted);
    padding: 0.75rem 0;
    list-style: none;
    font-weight: 500;
    transition: color 200ms;
}

.faq-item summary:hover {
    color: var(--color-text);
}

.faq-item summary::marker,
.faq-item summary::-webkit-details-marker {
    display: none;
}

.faq-item .faq-answer {
    color: var(--color-muted);
    padding-bottom: 1rem;
}
```

Design notes:
- `border-bottom` visually separates Q&A items
- `list-style: none` + vendor-prefixed `::-webkit-details-marker` normalizes the disclosure triangle across Safari/Chrome/Firefox
- `cursor: pointer` — the entire `<summary>` is clickable
- `color` transitions on hover — consistent with existing `.card:hover` and `nav a` patterns
- `.faq-answer` padding at bottom only (top spacing comes from the template's `mt-2` Tailwind utility)
- Reuses existing `--color-muted`, `--color-text`, `--color-border` custom properties

### Verification

#### Automated
- [x] `./scripts/build-css.sh && git diff --exit-code -- static/site.css` — CSS compiles cleanly and `static/site.css` is in sync
- [x] `./scripts/test.sh` passes (drift check green; all existing tests still green — no template changes yet, so no rendered HTML to assert)

#### Manual
- [ ] `git diff css/site.css` — review the new `.faq-item` rules for visual intent
- [ ] `git diff static/site.css` — spot-check the compiled output contains `.faq-item` rules

---

## Phase 3: Template + Handler — wire FAQ into the page

### Changes

#### 1. Add FAQ section to template
**File**: `templates/singlethread.html`
**Action**: modify — insert between line 88 (`</p>` closing "Built for quiet productivity") and line 89 (`< p class="text-2xl text-accent text-center mt-12">`)

```html
<h2 class="heading-section">Frequently Asked Questions</h2>
<div class="space-y-0">
    {% for item in faq_items %}
    <details class="faq-item">
        <summary>{{ item.question }}</summary>
        <p class="faq-answer text-muted mt-2">{{ item.answer }}</p>
    </details>
    {% endfor %}
</div>
```

Note: `space-y-0` on the wrapper (not `space-y-3`) because `.faq-item` already has `border-bottom` providing visual separation. The border is the separator, not whitespace.

####2. Pass `faq_items` through render context
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify — update the `context!{...}` macro call in `pub async fn index` (line 14)

Old:
```rust
    let html = state.templates.get_template("singlethread.html")?.render(
        context! { wallpaper_url, photographer, photographer_url, active_page => "singlethread" },
    )?;
```

New:
```rust
    let html = state.templates.get_template("singlethread.html")?.render(
        context! { wallpaper_url, photographer, photographer_url, active_page => "singlethread", faq_items => FAQ_ITEMS },
    )?;
```

####3. Update existing `index_serves_ok_html` test
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify — inside `index_serves_ok_html` test, after the `"Built for quiet productivity"` assert (line 48), add:

```rust
        // FAQ section
        assert!(body.contains("Frequently Asked Questions"));
        assert!(body.contains("<details"));
        assert!(body.contains("<summary>Where is my data stored?</summary>"));
        assert!(body.contains("stored on your device"));
```

Also update the closing CTA assert on line 49 to still pass — it should, since the FAQ is inserted *before* that element. No change needed to the assert itself, but verify positioning.

####4. Add new FAQ integration tests
**File**: `src/interfaces/handlers/singlethread/web.rs`
**Action**: modify — add inside the existing `mod tests` block, after `index_shows_credit_as_text_when_no_photographer_url` (after line 97)

```rust
#[tokio::test]
async fn faq_all_questions_appear() {
    let addr = start_app().await;
    let client = test_client();
    let res = client
        .get(format!("http://{addr}/singlethread"))
        .send()
        .await
        .expect("request failed");
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.text().await.unwrap();
    for item in FAQ_ITEMS {
        assert!(
            body.contains(item.question),
            "FAQ question not found in rendered page: {}",
            item.question,
        );
    }
}

#[tokio::test]
async fn faq_all_answers_appear() {
    let addr = start_app().await;
    let client = test_client();
    let res = client
        .get(format!("http://{addr}/singlethread"))
        .send()
        .await
        .expect("request failed");
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.text().await.unwrap();
    for item in FAQ_ITEMS {
        assert!(
            body.contains(item.answer),
            "FAQ answer not found in rendered page: {}",
            &item.answer[..item.answer.len().min(60)],
        );
    }
}

#[tokio::test]
async fn faq_section_after_quiet_productivity_before_cta() {
    let addr = start_app().await;
    let client = test_client();
    let res = client
        .get(format!("http://{addr}/singlethread"))
        .send()
        .await
        .expect("request failed");
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.text().await.unwrap();

    let quiet_pos = body.find("Built for quiet productivity").expect("quiet productivity heading");
    let faq_pos = body.find("Frequently Asked Questions").expect("FAQ heading");
    let cta_pos = body.find("Your reminders. One at a time.").expect("CTA");

    assert!(quiet_pos < faq_pos, "FAQ must appear after 'Built for quiet productivity'");
    assert!(faq_pos < cta_pos, "FAQ must appear before closing CTA");}

#[tokio::test]
async fn faq_no_javascript() {
    let addr = start_app().await;
    let client = test_client();
    let res = client
        .get(format!("http://{addr}/singlethread"))
        .send()
        .await
        .expect("request failed");
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.text().await.unwrap();
    // No <script> tags or onclick handlers anywhere in the page
    assert!(!body.contains("<script"));
    assert!(!body.contains("onclick"));
}
```

### Verification

#### Automated
- [ ] `./scripts/test.sh` passes — all existing tests still green, new FAQ integration tests green, CSS drift check still green
- [ ] `cargo clippy --all-targets --all-features --locked -- -D warnings` — no new warnings (unused imports, etc)

#### Manual
- [ ] `./scripts/test.sh` output shows all 4 new FAQ tests passing by name
- [ ] `cargo nextest run` output — no test failures

---

## Phase4: Documentation — update `ROUTES.md`

### Changes

####1. Update `/singlethread` block
**File**: `ROUTES.md`
**Action**: modify — in the `### GET /singlethread` block (lines 22–37), update line 26 prose to mention the FAQ

Old (line 26):
```
screenshot and watch-image cards with hover transitions, feature lists, and a
```

New:
```
screenshot and watch-image cards with hover transitions, feature lists, an
FAQ section with collapsible Q&A pairs (native <details>/<summary> widgets), and a
```

### Verification

#### Automated
- [ ] `./scripts/test.sh` passes (no code changes; gate confirms ROUTES.md change is syntactically fine)

#### Manual
- [ ] `git diff ROUTES.md` — review the updated block; verify it still follows `###` … `---` self-contained convention
- [ ] Open the SingleThread page in a browser (use `live-testing` skill) and visually confirm:
  - FAQ section heading appears between "Built for quiet productivity" and CTA
  - Each Q&A renders as a collapsible `<details>` element
  - Clicking a question expands/collapses its answer
  - Disclosure triangle is hidden (CSS `list-style: none`) — questions appear as plain clickable text
  - Hover state on questions works (color transitions from muted to text)
  - Section looks correct on narrow (mobile) and wide viewports