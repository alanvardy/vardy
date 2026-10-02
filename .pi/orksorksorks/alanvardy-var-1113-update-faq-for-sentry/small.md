# Task

Update the FAQ content on both the SingleThread and CheckStitch public pages to
reflect that the app now uses **Sentry** to track crash reports, so anonymous
data is sent there in order to fix bugs.

Both `FAQS` arrays are defined inline as Rust `FaqItem` consts in the handler
files. Concretely, the Privacy section (and any "network requests" answer) of
each page currently states that NO data is collected and that no network
requests carry identifying data. Reword these answers so they remain honest
while disclosing that anonymous crash-report data is sent to Sentry and used
only to fix bugs.

Scope is content-only — no schema, route, or structural changes.

## Why SMALL
Meets A–F: two files in one module (`src/interfaces/handlers/`), follows the
existing inline-FAQ pattern, 0 unknowns, no schema/migration, no new subsystem
or shared code, no design sign-off, and only a few local copy/test touch-ups.

## Key files (flag from recon)
- `src/interfaces/handlers/singlethread/web.rs` — `FAQS` const, Privacy
  category `"Do you collect or sell my data?"` (~line 77) and the network
  requests question (~line 73).
- `src/interfaces/handlers/checkstitch/web.rs` — `FAQS` const, Privacy
  category `"Do you collect my data?"` (~line 65) and `"Why does the app use
  the network at all?"` (~line 73).
- Tests assert rendered FAQ text (minijinja-autoescaped): checkstitch has an
  assertion on the substring `no analytics, no tracking, and no advertising`
  (~line 300) that will need to match the new copy. Similar escaped-substring
  assertions may exist in each file's `#[cfg(test)]` — update them to the new
  wording and add/adjust for Sentry mention.