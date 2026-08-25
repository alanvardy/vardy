# Structure Outline

## Approach

Extend both page handlers to pass `photographer` + `photographer_url` from the
existing `Picture` struct into template context, and render a fixed-position
credit overlay in `layout.html` — no new route, no schema changes, no hand-written
CSS. The credit is entirely template-driven, gated on `photographer` being
non-empty.

## Phase 1: Core credit line — both pages, happy path

Delivers the Unsplash credit line visible on `/` and `/singlethread`. Both
handlers pass photographer data; `layout.html` renders the overlay. The credit
appears as a linked name when `photographer_url` is populated, and as plain text
when it is empty.

**Files**: `src/interfaces/handlers/home/web.rs`,
`src/interfaces/handlers/singlethread/web.rs`, `templates/layout.html`,
`src/test/mod.rs`, `static/site.css`

**Key changes**:
- `index(State(state)): Result<Html<String>, WebError>` — both home and singlethread
  handlers: destructure `Picture` to capture `photographer` + `photographer_url`
  alongside `url`, pass all three into `context! { wallpaper_url, photographer, photographer_url }`
- `templates/layout.html` — new credit `<div>` after the `.wallpaper` div:
  `{% if photographer %}<div class="fixed bottom-3 right-3 …" aria-hidden="true">Photo by …</div>{% endif %}`
  with an inner `{% if photographer_url %}<a href="…" …>{% endif %}` conditional
- `seed_wallpaper(db: &SqlitePool)` — extend INSERT to include
  `photographer_url = 'https://unsplash.com/@test'` so existing tests get a
  linked credit line without changes
- `static/site.css` — regenerated via `scripts/build-css.sh` to include any new
  Tailwind utility classes

**Verify**:
```fish
./scripts/test.sh
```
- `cargo test` passes — existing tests still green; new assertions added to
  `index_serves_ok_html` (both home and singlethread) confirm `Photo by` and
  photographer name/link appear in the response body
- Manual: `cargo run`, open `/` and `/singlethread` — credit pill visible
  bottom-right with semi-transparent dark backdrop, photographer name linked,
  "on Unsplash" text

---

## Phase 2: Edge-case hardening

Hardens the credit line against degraded data and adds test coverage for every
conditional branch. No template or handler logic changes — test-only additions
plus a lightweight seed helper for the empty-URL case.

**Files**: `src/interfaces/handlers/home/web.rs`,
`src/interfaces/handlers/singlethread/web.rs`, `src/test/mod.rs`

**Key changes**:
- `seed_wallpaper_no_url(db: &SqlitePool)` — new test helper in `src/test/mod.rs`:
  `INSERT INTO unsplash_pictures (url, photographer) VALUES ('…', 'NoLink Photographer')`
  (no `photographer_url` → `DEFAULT ''`)
- `index_shows_credit_as_text_when_no_photographer_url` — new test in both handler
  test modules: seed via `seed_wallpaper_no_url`, assert body contains `Photo by
  NoLink Photographer`, assert no `<a href=` wrapping the name
- `index_hides_credit_when_wallpaper_fetch_fails` — extend existing
  `index_still_renders_when_wallpaper_fetch_fails` tests: assert `Photo by` does
  NOT appear in body when wallpaper fetch is 500 (no photographer data → gate
  hides block)

**Verify**:
```fish
./scripts/test.sh
```
- All new tests pass; edge cases covered: linked credit, unlinked credit, no
  credit

---

## Testing Checkpoints

| After Phase | What must be true |
|---|---|
| Phase 1 | `./scripts/test.sh` green. GET `/` and `/singlethread` both contain `Photo by Wallpaper Photographer` with a link to `https://unsplash.com/@test`. |
| Phase 2 | `./scripts/test.sh` green. `Photo by` absent when wallpaper fails. `Photo by NoLink Photographer` renders as plain text (no `<a href>`). |