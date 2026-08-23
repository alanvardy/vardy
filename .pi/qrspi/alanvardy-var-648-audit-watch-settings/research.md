# Research Findings

Codebase root for all references: `~/dev/SingleThread` (Swift app; this repo's checkout of the
branch artifacts only). Paths relative to that root.

## Q1: Inventory of phone settings-screen settings

`SettingsView` owns no state — every preference is a `Binding` back to `ContentView`'s
`@AppStorage` properties (`SingleThread/SettingsView.swift:59-61`). `@AppStorage` without a
`store:` argument defaults to `UserDefaults.standard`. App Group suite:
`group.app.alanvardy.SingleThread`; `AppGroup.defaults = UserDefaults(suiteName:) ?? .standard`
with fallback on watchOS/unregistered sims/previews
(`SingleThreadCore/Sources/SingleThreadCore/AppGroup.swift:7-14`).

| # | Label (SettingsView.swift) | Key | Type / default | Platform-gated | Store |
|---|---|---|---|---|---|
| 1 | Appearance (:126-133) | `appearanceMode` | AppearanceMode enum / `.system` (ContentView.swift:188-189) | No | **standard** |
| 2 | Text Size (:134-140) | `textSize` | TextSize enum / `.system` (ContentView.swift:191-192) | No | **standard** |
| 3 | Sort By (:141-147) | `sortOption` (`SortOption.defaultsKey`, SortOption.swift:17) | SortOption enum / `.priority` (ContentView.swift:215-216) | No | **App Group** |
| 4 | Allow Landscape (:148-154) | `allowsLandscape` | Bool / `true` (ContentView.swift:194-196; AppDelegate.swift:53-57) | **Yes, `#if os(iOS)`** (SettingsView.swift:67, 80, 187-190) | **standard** |
| 5 | Show Microphone (:155-158) | `showMicrophoneButton` | Bool / `true` (ContentView.swift:199-200) | No | **standard** |
| 6 | Background (:159-162) | `backgroundEnabled` | Bool / `true` (ContentView.swift:202-203) | No | **standard** (explicit `store: .standard`) |
| 7 | Background Fade (:163-168) | `backgroundFadePercent` | Int 0…90 step 10 / 50 (BackgroundFade.swift:9-27; ContentView.swift:205-206) | No | **standard** (explicit) |
| 8 | Enable action buttons (:169-173) | `enableActionButtons` | Bool / `false` (ContentView.swift:208-210) | **Yes, `#if os(iOS)`** | **standard** |
| 9 | Show Undated (:174-177) | `showUndatedReminders` | Bool / `false` (ContentView.swift:212-213) | No | **App Group** |
| 10 | Show Date (:178-181) | `showDate` | Bool / `true` (ContentView.swift:217-218; ShowDatePreference.swift:18 missing-key→true) | No | **App Group**; duplicated in SingleThreadApp.swift:99-100 |
| 11 | Excluded Projects (:188-196) | `excludedProjectTitles` | `[String]` / `[]` (ExcludedProjectStore.swift:7,14) | No | **App Group**, via store get/set not `@AppStorage` (ContentView.swift:234-237) |

Non-setting: read-only Unsplash attribution footer `backgroundPhotographer`
(SettingsView.swift:198-203). Core never reads UserDefaults directly — stores are injected
(`SingleThreadCore/.../ReminderStore.swift:13-21,65`). Launch-time reads outside SettingsView:
`AppearanceMode.load(from: .standard)` (AppearanceMode.swift:63-69), orientation lock from
`UserDefaults.standard` (AppDelegate.swift:52-57).

## Q2: Phone→watch communication end-to-end

Core file: `SingleThreadCore/Sources/SingleThreadCore/SkippedReminderSyncService.swift` (SRSS).

### Transports (exactly two)
- **`updateApplicationContext`** — all state sync: skips + showUndated (SRSS:89-101),
  excluded titles (:107-114), sortOption (:118-131), showDate (:133-145). "Latest-wins,
  auto-delivers on (re)connect" (SRSS:18-21).
- **`sendMessage`** (no reply handler) — watch→phone interactive complete (:147-154) and
  delete (:156-164) only. No `transferUserInfo`/`sendMessageData` anywhere.
- WCSession abstracted behind `SkipSyncSession` protocol (SRSS:8-15; conformance SRSS:17);
  delegate attached in `activate()` (:66-72); iOS re-activates on `sessionDidDeactivate`
  (:216-220).

### Payload keys (SRSS:233-243)
`skippedReminderIdentifiers`, `excludedProjectTitles`, `showUndatedReminders`,
`sortOption`, `showDate` (applicationContext); `completeReminderIdentifier`,
`deleteReminderIdentifier` (sendMessage). **No timestamp/sequence keys exist.**

### Phone-side triggers (wired in `SingleThread/SingleThreadApp.swift:24-64`, guarded by
`WCSession.isSupported() && !usesInMemoryStore`)
- Receive handlers assigned before `activate()` (write-once-before-activate invariant):
  onComplete :35-38, onDelete :39-41, onExcludedProjectTitles :42-44.
- `store.onSkipSetChanged` → `service.push(ids, showUndatedReminders:)` (:50-52); fired from
  `applySkipSet` (ReminderStore.swift:369-374) and clear-skips in `reload()` (:292-295).
- `store.onShowUndatedRemindersChanged` → push (:53-55); fired from no-op-guarded `didSet`
  (ReminderStore.swift:99-104).
- `store.onExcludedProjectsChanged` → `pushExcludedProjectTitles` (:56).
- `store.onSortOptionChanged` → `pushSortOption` (:63); fired from guarded `setSortOption`
  (ReminderStore.swift:230-236).
- `.onChange(of: showDate)` on WindowGroup → `syncService?.pushShowDate(newValue)`
  (SingleThreadApp.swift:77-81).
- `store.onDeleteReminder` relay is inert on iOS (only watchOS branch fires it,
  ReminderStore.swift:167-172; comment at SingleThreadApp.swift:57-61).

### Clobbering / ordering
No explicit timestamps. Safety comes from: (1) WCSession's latest-wins context queue
(SRSS:19-20,88); (2) combined-context pushes — `pushSortOption` re-sends skip IDs
(SRSS:116-124), `pushShowDate` sends skip IDs + showDate together because "the whole context
is replaced" (SRSS:131-139); (3) receiver guards — absent keys are no-ops (SRSS:169-200),
stale skip IDs pruned and re-persisted in `reload()` via `ReminderSkipLogic.resolve`
(ReminderStore.swift:297-310), receive path uses `refreshExcludedProjectTitles` which does
not fire `onExcludedProjectsChanged` so pushes never echo (ReminderStore.swift:320-326).
Residual: interleaved context *shapes* (combined vs titles-only vs sort-only) can still
overwrite each other's omitted keys.

## Q3: Watch consumption of each payload key

Watch pipeline built in `SingleThreadWatch/SingleThreadWatchApp.init()` (:10-52): creates
`ReminderStore` (:15), restores sort (:20), builds service with `sendsShowDate: false` (:26-27)
and stores pinned to `.standard` (:26,28), sets handlers before `activate()` (:30-43).
Receive path: SRSS:168-201.

| Key | On receive | While running | Across restart |
|---|---|---|---|
| `skippedReminderIdentifiers` | persisted only, no hook (SRSS:179-181) | **not live** — applied on next `reload()` (manual refresh, WatchReminderView.swift:196-206) or relaunch (`task { await store.start() }` WatchReminderView.swift:41-43 → reload ReminderStore.swift:125,251) | yes, via `skipStore.load()` in reload (ReminderStore.swift:299-300), stale pruned :304 |
| `excludedProjectTitles` | persist + hook (SRSS:182-186) | **live**: handler → `refreshExcludedProjectTitles` updates in-memory + fires `onRemindersChanged` (SingleThreadWatchApp.swift:41-43; ReminderStore.swift:324-328) | yes, re-applied in reload (ReminderStore.swift:301) |
| `showUndatedReminders` | hook only, **not persisted** (SRSS:187-189) | **live**: sets `store.showsUndatedReminders` then `reload()` (SingleThreadWatchApp.swift:28-33) | **lost** — resets to `false` until next push (ReminderStore.swift:100) |
| `sortOption` | persist + hook (SRSS:190-195) | **live**: `setSortOption` re-sorts (SingleThreadWatchApp.swift:36-38; ReminderStore.swift:230-236) | yes — restored in init before any push (SingleThreadWatchApp.swift:20; direct assignment doesn't fire hooks, ReminderStore.swift:61-62); fallback `.priority` (SortOption.swift:34-37) |
| `showDate` | persisted to **`.standard`** via `ShowDatePreference(defaults: .standard)`, no hook (SRSS:196-199; SingleThreadWatchApp.swift:26) | read as `@AppStorage("showDate")` in `WatchReminderView` (:56-57, rendered :173) — update timing depends on `@AppStorage` external-write observation | yes |

Outbound-only keys (`completeReminderIdentifier`, `deleteReminderIdentifier`) are wired as
requests on the watch (:49-50) and consumed on the iPhone (SingleThreadApp.swift:35-38).

## Q4: Preferences reaching the watch outside the sync service

- **None reach the watch except via `SkippedReminderSyncService`.** The watch target has
  **no `CODE_SIGN_ENTITLEMENTS` setting and no .entitlements file**
  (`project.pbxproj` watch configs :853, :881), so it has no App Group capability;
  `UserDefaults(suiteName:)` there yields an unshared instance, effectively `.standard`.
- Only two entitlements files exist, both under `SingleThread/`: `AppGroup.entitlements`
  (app group only) used by iOS app (pbxproj :660-662, :710-712) and widget target
  (:912, :943); `SingleThread.entitlements` (sandbox + same group) for macOS (:660-662).
- The **widget extension** (`SingleThreadWidget/NextThingWidget.swift`) reads shared prefs
  from the real App Group suite: `showUndatedReminders` raw bool (:59),
  `showDate` via `ShowDatePreference().isEnabled` (:52), and widget intents read
  `SortOptionStore().load()` (`SingleThreadCore/.../ReminderIntents.swift:19,40`). Phone-side
  writers: ContentView.swift:213-220, SingleThreadApp.swift:99-100.
- Widget platforms are iphoneos/iphonesimulator/macosx only (:926) — never watchOS.
- All other settings (`appearanceMode`, `textSize`, landscape, mic, background…) live in
  plain `UserDefaults.standard` on the phone and never leave it (ContentView.swift:188-210).

## Q5: Test coverage

### Seams
- `SkipSyncSession` protocol (SRSS:8-17) — "WCSession is not mockable, so we abstract";
  recording fake `FakeSession` in `SingleThreadTests/SkippedReminderSyncServiceTests.swift:9-33`.
- `InMemoryEventStore` (`SingleThreadCore/.../InMemoryEventStore.swift:19`) mirrors
  `predicateForIncompleteReminders`; used by unit tests and the `--seed` path.
- `UITestingSeed.fromLaunchArguments` parses `--seed '<json>'`;
  `resetPersistedState()` clears defaults between launches
  (SingleThreadCore/.../UITestingSeed.swift:27,41). iOS consumes in
  `SingleThreadApp.makeStore(arguments:)` (:113-145): `--seed` → InMemoryEventStore;
  `--ui-testing` → single hardcoded reminder + forces action buttons on.
- Watch seams: `--ui-testing` (SingleThreadWatchApp.swift:11) and
  `--ui-testing-excluded "<project>"` (:78-88) build a deterministic reminder store without
  EventKit access.
- No `#if DEBUG` blocks in app sources.

### Coverage by area
- **Sync service**: 29 unit tests in `SkippedReminderSyncServiceTests.swift`
  (`#if os(iOS) || os(watchOS)`): activation, push/receive per key, latest-wins replacement
  (:138), clear-all (:153), malformed-payload no-ops (:164,:177), sort/skip anti-clobber
  (:205,:365), exclusion refresh through a real ReminderStore (:381-402), showDate combined
  push (:420) and `sendsShowDate:false` omission (:464).
- **Preference stores**: SortOptionTests (defaults/invalid key :41,:47, round-trip :55),
  ShowDatePreferenceTests (:6,:16,:25,:34), ExcludedProjectStoreTests (:6-:35),
  ReminderSkipTests covers pure logic (`resolve` pruning :6-32, add/dedupe :38-75); the
  persisted `SkippedReminderStore` itself is exercised only indirectly via sync-service tests.
- **SettingsView**: one structural unit test asserting body description contains all labels
  (`SettingsViewTests.swift:8`, iOS + non-iOS branches :11-31); behavioral UI coverage in
  `SingleThreadUITestsFlows.testSettingsOpensAndShowsControls` (:126-142) and background-toggle
  persistence across relaunch (:145+).
- **Watch apps**: UI tests only, no watch unit target.
  `SingleThreadWatchUITestsFlows.swift` (all with `["--ui-testing"]` :104-109): seeded card
  render (:18,:27), exclusion suppresses card (:37-48), complete (:56-64), skip (:72-80),
  delete (:87-98), refresh button (:104-116); launch tests :15-17.
- iOS UI tests additionally cover accessibility audit, seed-driven flows, appearance launches,
  action buttons (`SingleThreadUITests*.swift`, `ActionButtonsUITests.swift`).

## Cross-Cutting Observations

- Three distinct persistence tiers: phone-only cosmetics → `UserDefaults.standard`;
  phone↔widget shared prefs (`sortOption`, `showDate`, `showUndatedReminders`,
  exclusions, skips) → App Group suite; anything reaching the watch → exclusively
  WatchConnectivity applicationContext (watch has no entitlements).
- Watch-side preference storage is local-only `.standard` (explicitly pinned for showDate,
  SingleThreadWatchApp.swift:26; App Group fallback for the rest, AppGroup.swift:13-15).
- Consistency invariant pattern: write-once handlers before `activate()`, absent-key no-ops on
  receive, no-op-guarded setters, echo-free receive path (`refreshExcludedProjectTitles` vs
  `setExcludedProjectTitles`).
- `showUndatedReminders` is the only synced pref not persisted on the watch; `skippedReminderIdentifiers`
  is the only one persisted but not applied live. Both asymmetries are observable in code, not bugs
  asserted by any test.
- Tests compile the core suites cross-platform but watch behavior is verified only via UI tests.

## Open Areas

- Whether `@AppStorage("showDate")` on the watch re-renders immediately when
  `ShowDatePreference.set` writes externally to `.standard` is unverified — SwiftUI's
  observation of out-of-band writes is version-dependent; no test pins this behavior.
- No test covers interleaving of the three different context shapes (combined / titles-only /
  sort-only) arriving out of order across an interrupted connection; correctness rests on each
  push reloading fresh values at send time.
- macOS Catalyst path uses `SingleThread.entitlements` with the same group ID, but widget/macOS
  interaction was not examined (out of scope of the questions).
