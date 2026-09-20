Recon: CheckStitch marketing page (mirror SingleThread page in this axum web app)

## 1. Files to change/create

### src/interfaces/handlers/singlethread/web.rs  (~470 lines; FAQS 5 categories, 24 items)
- Imports (lines 1-7): use axum::{extract::State, response::Html}; use minijinja::context; use crate::app::error::WebError; use crate::app::picture; use crate::app::state::AppState;
- Structs: struct FaqCategory { title: &str, items: &[FaqItem] }  and  struct FaqItem { question: &str, answer: &str }
- FAQS const type (line 19): const FAQS: &[FaqCategory] = &[ ... ]; categories: Getting started(2), Features(7), Privacy(3), Other(5), Shortcuts(7) = 24 items.
- index handler body (lines 142-166) VERBATIM:
  pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
      state.metrics.inc_page_view("singlethread");
      let (wallpaper_url, photographer, photographer_url) = picture::wallpaper_context(&state).await;
      let faq_categories: Vec<serde_json::Value> = FAQS.iter().map(|category| {
          let items: Vec<serde_json::Value> = category.items.iter().map(|item|
              serde_json::json!({ "question": item.question, "answer": item.answer })).collect();
          serde_json::json!({ "title": category.title, "items": items })
      }).collect();
      let html = state.templates.get_template("singlethread.html")?.render(
          context! { wallpaper_url, photographer, photographer_url, active_page => "singlethread", faq_categories })?;
      Ok(Html(html))
  }
- Test module #[cfg(test)] mod tests lines ~169-470, 12 tests = 10 tokio tests + 2 plain tests:
  index_serves_ok_html (~186): 200, text/html, <title>SingleThread</title>, <h1>, hero tagline, Why it helps, FAQ heading, <details, <summary><span class=faq-chevron, app-store href count==2 AND /static/app-store.svg?v= count==2, target=_blank, each screenshot src=/static/singlethread-shot-*.jpg?v=, watch png srcs, icon png, nav <a href=/singlethread class=active>SingleThread, NO legacy classes "st- or st- or section-heading or home-columns, escaped wallpaper url(https:&#x2f;&#x2f;example.com...), Photo by, hidden md:block, bg-black/50.  CHECKSTITCH MUST DROP the two app-store count==2 asserts (no badge).
  index_still_renders_when_wallpaper_fetch_fails (~267): unsplash stub 500, DELETE unsplash_pictures, still 200, no background-image and no Photo by.
  index_shows_credit_as_text_when_no_photographer_url (~292): seed_wallpaper_no_url, Photo by NoLink Photographer, NOT the linked form.
  faq_all_questions_appear (~310) and faq_all_answers_appear (~333): every q and a matched via html_escape helper (apostrophe to &#x27;, slash to &#x2f;).
  faq_app_intents_documented (~356): Shortcuts h3 plus one question plus one answer.
  faq_section_after_quiet_productivity_before_cta (~380): FAQ after Built for quiet productivity and before closing CTA.
  faq_no_javascript (~407): no <script, no onclick.
  faq_summary_has_chevron (~426): each question text preceded by the closing span tag.
  faq_items_grouped_under_category_headings (~439): each category h3 heading and first-item ordering between headings.
  faq_items_all_non_empty and faq_items_no_duplicate_questions (plain tests ~467, ~481).
  Helper fn html_escape(input: &str) -> String (~461-467) reproducing minijinja AutoEscape::Html (escapes & < > " apostrophe slash).
  Note: outputs are minijinja HTML-autoescaped; tests assert escaped entity forms, never raw strings or full class attributes.

### src/interfaces/handlers/mod.rs line 5 - add: pub mod checkstitch;  (singlethread declaration is line 5).

### src/interfaces/routes.rs
- Line 49: .route("/singlethread", get(handlers::singlethread::web::index))  -> add sibling .route("/checkstitch", get(handlers::checkstitch::web::index)) right after it.
- Screenshot-immutable-caching test singlethread_screenshots_are_served_with_immutable_caching is lines 215-240. Core VERBATIM:
  let cases = [
      ("/static/singlethread-shot-main.jpg", "image/jpeg"),
      ("/static/singlethread-shot-settings.jpg", "image/jpeg"),
      ("/static/singlethread-shot-swipe.jpg", "image/jpeg"),
      ("/static/singlethread-shot-ipad.jpg", "image/jpeg"),
      ("/static/singlethread-watch-list.png", "image/png"),
      ("/static/singlethread-watch-detail.png", "image/png"),
  ];
  for (path, content_type) in cases { asserts 200, content-type matches, cache-control contains max-age=31536000, with a panic message naming path }

### templates/singlethread.html : copy to templates/checkstitch.html
- Structure: {% extends "layout.html" %}; blocks title (line2), heading (line3), content (lines 4-145). No includes.
- OMIT hero app-store badge block lines 26-33 (an a-href to apps.apple.com plus an img using asset_url of app-store.svg; grep shows app-store.svg at lines 30 and 138) and the closing CTA block lines 134-141.
- All images referenced via asset_url( FILENAME ). FAQ rendered by a for-loop over faq_categories. Screenshot figure uses class card m-0 ... p-3; img class w-full rounded-lg border border-neutral-700.

### templates/layout.html nav block
- Nav block lines 23-30. Line 25 VERBATIM:
        <a href="/singlethread"{% if active_page == "singlethread" %} class="active"{% endif %}>SingleThread</a>
  Add after it a CheckStitch link using active_page == "checkstitch". Pattern: the active_page boolean from the handler minijinja context var adds class="active".

### src/app/assets.rs
- asset_url(file: &str) -> String (around line 43): SHA-256 hashes every file under static/ at first use (ASSET_HASHES is a OnceLock HashMap), returns /static/FILE?v=<12-hex>. Panics on unknown files, so every referenced asset must physically exist in static/.

### ROUTES.md
- ### GET /singlethread block is lines 19-33, VERBATIM head:
  Renders the SingleThread page with an app-icon hero (icon badge plus tagline), platform badges
  (iPhone, iPad, Mac, Watch), an Apple App Store download badge (top of the hero and at the
  closing CTA), a gradient decorative divider, screenshot and watch-image cards with hover
  transitions, feature lists, an FAQ section with collapsible Q&A pairs (native details/summary
  widgets), and a closing CTA line. Includes a random Unsplash wallpaper and photographer credit
  (linked name when a profile URL is available, plain text otherwise). The wallpaper and credit
  gracefully degrade to hidden when the Unsplash fetch fails.
  - Response: 200 OK - text/html (minijinja templates/singlethread.html)
  - Errors: 500 via WebError (template render failure)
  - Rate limit: global per-IP GCRA limiter. Over limit -> 429 Too Many Requests, plain-text body
    too many requests, with Retry-After and X-RateLimit-* headers.
  block ends with a --- line at line 33. New ### GET /checkstitch block goes after that --- (line 34), before ### GET /contact. Same shape, omit the App Store badge sentence, Response line points at templates/checkstitch.html.

### Static images static/  (all flat in static/, no subdirectory)
  Naming pattern APP-ROLE, mirror as checkstitch-*. Sizes: singlethread-icon.png 82KB; singlethread-shot-main.jpg 238KB; singlethread-shot-settings.jpg 30KB; singlethread-shot-swipe.jpg 146KB; singlethread-shot-ipad.jpg 135KB; singlethread-watch-list.png 16KB; singlethread-watch-detail.png 54KB. Other files: alanvardy.jpg, app-store.svg 10KB, github.svg, linkedin.svg, site.css 14KB, wave.svg.

### Nav-set and asset assertions to update
- src/interfaces/handlers/home/web.rs line 61:  assert!(body.contains(r#"[a href=/singlethread]SingleThread[/a]"#));  (quote exactly the existing line when editing)
- src/interfaces/handlers/contact/web.rs line 113: same singlethread nav-link assert.
- src/interfaces/handlers/singlethread/web.rs line 241: active-nav assert for /singlethread/class active (stays in singlethread tests).
- singlethread/web.rs lines 219-220: the two app_store count()==2 asserts are SingleThread-only, NOT carried into checkstitch.
- routes.rs lines 83 and 135: static_icon_is_served and static_files_have_immutable_cache_control reference singlethread-icon.png (leave as-is; optional checkstitch equivalent).
- No other singlethread references exist in src/ or templates/ (grep across src/ confirmed). The string CheckStitch appears NOWHERE in this repo today (0 grep hits in src/ + templates/ + ROUTES.md).

## 2. Established pattern (7 lines)
Copy singlethread/web.rs to src/interfaces/handlers/checkstitch/web.rs; register pub mod checkstitch; in handlers/mod.rs; change inc_page_view key, active_page, template name, and FAQS content to CheckStitch; delete the app-store badge markup and its two count()==2 test asserts; copy the template dropping both badge blocks; add the nav link at layout.html line 25; add the route plus the immutable-caching test in routes.rs; add checkstitch-* screenshot/png/icon files under static/; document in ROUTES.md. Every template asset_url reference must resolve to a real file or asset_url panics in tests.

## 3. Gate and setup
- Gate ./scripts/test.sh runs, in order: FORMAT cargo fmt --all ; SQLX cargo sqlx prepare -- --tests ; cargo check --all-targets ; BUILD CSS ./scripts/build-css.sh then git diff --exit-code -- static/site.css (CSS-drift gate: any new Tailwind class forces site.css to change and must be committed in the same change) ; cargo clippy --all-targets --all-features --locked -- -D warnings ; cargo nextest run ; TODO grep via ripgrep matching FIXME|fixme|dbg!|DEBUG:|FIXTURE:|TODO markers in src and must be empty. test.sh sources .env for DATABASE_URL.
- Fresh worktree: .env ALREADY EXISTS here; test.db does NOT yet. Run cargo sqlx database create and then cargo sqlx migrate run first (test.db is gitignored SQLite at sqlite:test.db). .sqlx/ offline metadata is present (2 query json files) - do not let a prepare against a blank DB delete it (git checkout -- .sqlx restores). Reset helper script is ./scripts/reset_db.sh.

## 4. Boundary assessment
- Crosses NO schema (no migration/DB change), NO API contract, NO new subsystem. Touches a handler module, the route registry, the shared nav template, ROUTES.md, ~7 static assets, and nav assertions in home/web.rs and contact/web.rs.
- Test surface: ~12 replicated inline tests in the new module plus 1 route caching test plus nav-link asserts in home/contact.
- UI: reuses the SingleThread exact layout and Tailwind classes, so static/site.css regeneration is NOT required iff no new class is introduced (medium.md mandates no new classes). If one sneaks in, the CSS-drift gate fails and forces regeneration plus commit within the same change.

## 5. CheckStitch copy content
- medium.md gives NO CheckStitch FAQ text, screenshot descriptions, or App Store id (it lists only file names and the omit-badges rule). The source of truth is the ../CheckStitch iOS repo, which the task BOUND explicitly forbids touring. Real copy is therefore unavailable in-repo; the implementer must source it from the CheckStitch app/repo (out of scope here) or draft placeholder copy. CheckStitch appears nowhere in this repo today (0 grep hits).
- App Store id for CheckStitch: none (the badge is omitted because there is no App Store presence yet).
