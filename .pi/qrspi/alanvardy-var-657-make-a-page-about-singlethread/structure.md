# Structure Outline

## Approach
Add the `/singlethread` page by cloning the existing home-page pattern (feature handler directory → route line → template extending `layout.html`), establish static-asset capability via `tower-http`'s `ServeDir` behind `/static`, and put a persistent nav bar in `layout.html` so every page inherits it. No database or service layers exist — "vertical" here means dependency/config + route/handler + template + test.

---

## Phase 1: Static Asset Serving + Icon Asset

Delivers end-to-end: a new dependency, a `static/` directory containing the downscaled SingleThread icon, a nested service route, and an HTTP test proving the browser can fetch it.

**Files**: `Cargo.toml`, `src/interfaces/routes.rs`, `static/singlethread-icon.png` (new, generated from `~/Downloads/AppIcon2.png`)
**Key changes**:
- `tower-http = { version = "?", features = ["fs"] }` — new dependency (version aligned to current `axum`; check `Cargo.lock`)
- `ServeDir::new("static")` nested at `/static`:
  ```rust
  .nest_service("/static", ServeDir::new("static"))
  ```
- Icon generation: `sips -Z 256 ~/Downloads/AppIcon2.png --out static/singlethread-icon.png`

**Verify**: `cargo test` passes, including new colocated test asserting `GET /static/singlethread-icon.png` returns 200 with `content-type: image/png`. Manual: `cargo run`, open `http://localhost:3000/static/singlethread-icon.png` in a browser, eyeball icon quality at 256px (if muddy, regenerate at 512px per design risk note).

---

## Phase 2: SingleThread Page

Delivers end-to-end: navigating to `/singlethread` renders the full marketing-style page (intro paragraph + feature list) with the icon displayed near the heading.

**Files**: `src/interfaces/handlers/mod.rs`, `src/interfaces/handlers/singlethread/{mod.rs,web.rs}` (new), `src/interfaces/routes.rs`, `templates/singlethread.html` (new)
**Key changes**:
- `pub mod singlethread;` added to `handlers/mod.rs`
- Handler cloned from `home/web.rs:7-13` shape:
  ```rust
  pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError>
  // renders templates/singlethread.html with context! {}
  ```
- Route: `.route("/singlethread", get(handlers::singlethread::web::index))`
- Template: `{% extends "layout.html" %}` overriding `title`, `heading`, `content`; `<img src="/static/singlethread-icon.png" alt="SingleThread icon" width="96">` inside `content`

**Verify**: `cargo test` passes, including new tests asserting `/singlethread` → 200, `text/html`, body contains title, copy strings, and the `<img ...>` tag. Manual: load `http://localhost:3000/singlethread`, confirm copy reads well and icon renders without layout jump.

---

## Phase 3: Persistent Nav Bar

Delivers end-to-end: both pages share a top nav (Home → `/`, SingleThread → `/singlethread`) inherited from `layout.html`, styled with the previously unused `--surface`/`--accent` tokens, verified by cross-link tests.

**Files**: `templates/layout.html`
**Key changes**:
- `<nav>` element inside `<body>` immediately before `<div class="container">` (outside `heading`/`content` blocks)
- New CSS rules in layout's inline `<style>`: nav uses `--surface` background, links colored `--text`, hover/current accent `--accent` (flexbox layout)
- No Rust changes

**Verify**: `cargo test` passes, including updated/new assertions that **both** `/` and `/singlethread` bodies contain `href="/singlethread"` and `href="/"`. Existing home tests must remain green unchanged (except any nav-substring additions). Manual: click Home ↔ SingleThread in the browser; confirm nav looks right on both pages and hover state uses `--accent`.

---

## Testing Checkpoints

After each phase, `cargo test` green means:
1. **Phase 1**: `/static/singlethread-icon.png` → 200 `image/png`. Home page untouched.
2. **Phase 2**: `/singlethread` → 200 `text/html` with expected title/copy/icon tag; Phase 1 still passing.
3. **Phase 3**: Both pages contain both nav links; all prior tests still pass.

Resume point: if context resets, run `cargo test` and check which of these hold — the first failing checkpoint is where work resumes.

---

## Notes

- Nothing in this design requires horizontal slicing — no DB/service layers exist; every slice is fully vertical through config → route → render → test.
- Dockerfile/`fly.toml` check (design risk): confirm `static/` is included in deployment alongside `templates/` during Phase 1 implementation review; not a separate slice.
- PR note: record the lowercase `/singlethread` path decision (design decision 3) when opening/updating PR against VAR-657.
