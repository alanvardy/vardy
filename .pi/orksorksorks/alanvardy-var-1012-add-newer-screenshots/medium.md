# Task

Replace the five screenshots on the app's SingleThread page with the five
newer images from ticket VAR-1012: the files in `~/Downloads` named
`F2C642A5-…_1_105_c.jpeg`, `F409E32A-…_1_105_c.jpeg`,
`AF8D1A8C-…_4_5005_c.jpeg`, `70EA8DBF-…_4_5005_c.jpeg`,
`6F6D3A52-…_1_105_c.jpeg`, applied **in that order** to the existing
screenshot slots (`singlethread-shot-main.jpg`, `singlethread-shot-settings.jpg`,
`singlethread-shot-swipe.jpg`, `singlethread-watch-list.png`,
`singlethread-watch-detail.png`) in `templates/singlethread.html`. Copy/convert
the new images into `static/` (renaming to match the current filenames
minimizes churn; the new set appears to be all-phone shots, so adjust the
`<img>` order/alt text as the images dictate — the page currently shows three
phone shots plus two Apple Watch shots). `asset_url()` hashes file content at
runtime, so replacing bytes under the same name automatically produces a fresh
`?v=` cache key — no manual cache-busting. Update both test suites that assert
the screenshot filenames so the gate passes.

## Why MEDIUM
MULTI_MODULE breadth — the change spans `templates/singlethread.html`, five
`static/` assets, and two separate test locations (`web.rs` HTML assertions and
`routes.rs` screenshot-serving tests), well over ~5 files — while M1–M2 hold:
the approach is a known asset swap with no schema, no new subsystem, and no
design decision.

## Key files
- `templates/singlethread.html` — the five `<img src="{{ asset_url('…') }}">`
  screenshot slots (lines ~43–66) plus their alt text
- `static/` — `singlethread-shot-main.jpg`, `singlethread-shot-settings.jpg`,
  `singlethread-shot-swipe.jpg`, `singlethread-watch-list.png`,
  `singlethread-watch-detail.png` (replace bytes with the new jpegs from
  `~/Downloads`)
- `src/interfaces/handlers/singlethread/web.rs` — HTML assertions on the img
  srcs (lines ~190–195)
- `src/interfaces/routes.rs` — `singlethread_screenshots_are_served_with_immutable_caching`
  test (lines ~215–223)