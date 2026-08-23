# Research Questions

## Context
Focus on the iOS app target (`SingleThread/`), the watchOS app target
(`SingleThreadWatch/`), and the shared `SingleThreadCore` SPM package.
Key files include `SettingsView.swift`, `ContentView.swift`,
`SingleThreadApp.swift` (both app targets), `SkippedReminderSyncService.swift`,
`AppGroup.swift`, the preference store types (`SortOption.swift`,
`ShowDatePreference.swift`, `ExcludedProjectStore.swift`, `ReminderSkip.swift`),
and `SingleThreadWidget/NextThingWidget.swift`.

## Questions
1. What is the full inventory of user-facing settings in the phone app's
   settings screen? For each setting: its storage key, type, default value,
   whether it is platform-gated (iOS-only etc.), and exactly which
   `UserDefaults` store it persists to (`UserDefaults.standard` vs App Group
   suite vs not persisted at all)?
2. How does phone→watch communication work end-to-end? Trace the
   `SkippedReminderSyncService`: what transport mechanisms does it use
   (application context vs `sendMessage`, etc.), what payload keys exist, what
   triggers a push from the phone side (which callbacks/`.onChange` handlers
   are wired in `SingleThreadApp.swift`), and what happens on clobbering or
   out-of-order updates?
3. How does the watch app consume each preference it receives — at launch,
   while running, and across restarts? For each payload key the watch can
   receive, trace where it lands: in-memory state, a persisted local store, an
   `@AppStorage` property, or nowhere. Note anything that only takes effect
   after a relaunch or next push.
4. Which preferences reach the watch through paths other than the sync
   service — e.g. shared App Group defaults read by the widget extension or
   intents — and how do the watch targets' entitlements affect which stores
   are actually shared versus falling back to `.standard`?
5. What test coverage exists for the sync service, the preference stores, the
   settings screen, and the watch apps? Describe the test seams used (mock
   session protocol, `InMemoryEventStore`, `--seed` / `--ui-testing` launch
   args) and which behaviors are currently covered by unit tests versus watch
   UI tests.
