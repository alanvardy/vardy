# Design — VAR-648: Audit watch settings

Codebase: `~/dev/SingleThread`. All `file:line` refs from research.md.

## Current State

The phone settings screen (`SingleThread/SettingsView.swift`) exposes 11 settings,
backed by `@AppStorage` bindings on `ContentView` (SettingsView.swift:59-61).
Persistence is tiered:

- **Phone-only cosmetics** → `UserDefaults.standard`: `appearanceMode`, `textSize`,
  `allowsLandscape` (iOS-only), `showMicrophoneButton`, `backgroundEnabled`,
  `backgroundFadePercent`, `enableActionButtons` (iOS-only)
  (ContentView.swift:188-210).
- **Shared with widget** → App Group suite: `sortOption`, `showUndatedReminders`,
  `showDate`, `excludedProjectTitles` (ContentView.swift:215-237).
- **Watch** → exclusively `SkippedReminderSyncService` (SRSS) application context;
  the watch target has **no entitlements file**, so `UserDefaults(suiteName:)` there
  is effectively `.standard` (pbxproj :853, :881; AppGroup.swift:13-15).

Of the 4 settings that reach the watch, audit results:

| Setting | Received | Live on watch | Persists across relaunch |
|---|---|---|---|
| `sortOption` | ✅ | ✅ re-sorts (SingleThreadWatchApp.swift:36-38) | ✅ restored in init (:20) |
| `excludedProjectTitles` | ✅ | ✅ hook fires `onRemindersChanged` (:41-43) | ✅ re-applied in reload |
| `showUndatedReminders` | ✅ | ✅ sets store + reload (:28-33) | ❌ **not persisted** (SRSS:187-189) |
| `showDate` | ✅ | ⚠️ via `@AppStorage` external-write observation — unverified | ✅ (pinned `.standard`, :26) |
| `skippedReminderIdentifiers` | ✅ | ❌ **applied only on next `reload()`/relaunch** (SRSS:179-181) | ✅ |

Additional structural weakness: three different context shapes are pushed
(combined / titles-only / sort-only — SRSS:89-145) and can overwrite each other's
omitted keys when interleaved across an interrupted connection.

## Desired End State

1. A generic receive pipeline on SRSS that applies **every** applicationContext key
   uniformly: persist (where applicable) + notify observers, with absent keys as no-ops.
2. All four synced settings propagate to the watch **live** and **survive relaunch**:
   `showUndatedReminders` persists on the watch; skips apply immediately;
   `showDate` updates deterministically via an explicit callback (no reliance on
   `@AppStorage` external-write observation).
3. All pushes send the **full combined context** (one shape) — interleaving risk eliminated.
4. The 7 phone-only cosmetics are documented as intentionally not synced.
5. New watchOS unit test target covers the receive pipeline and watch-side store
   behavior; existing 29 SRSS unit tests still pass; watch UI tests cover end-to-end.

**Verification**: unit tests for each key (persist + notify + absent-key no-op +
full-shape push), a test proving `showUndatedReminders` survives a simulated
relaunch, and watch UI tests showing a phone-side settings change reflected on
the watch without relaunch.

## Patterns to Follow

- **Persist + hook on receive**: `refreshExcludedProjectTitles` — updates in-memory
  state and fires `onRemindersChanged` without echoing a push
  (SRSS:182-186; ReminderStore.swift:324-328). This is the canonical receive behavior;
  the generic pipeline generalizes it.
- **Handlers assigned before `activate()`** (SingleThreadApp.swift:35-44,
  SingleThreadWatchApp.swift:30-43) — write-once-before-activate invariant.
- **No-op-guarded setters** so didSet hooks don't fire redundantly
  (ReminderStore.swift:99-104, 230-236).
- **Absent-key no-ops on receive** (SRSS:169-200) — keep in the generic pipeline.
- **Stale skip pruning via `ReminderSkipLogic.resolve`** in reload
  (ReminderStore.swift:297-310).
- **Session abstraction**: `SkipSyncSession` protocol + `FakeSession` test seam
  (SRSS:8-17; SkippedReminderSyncServiceTests.swift:9-33) — new tests must use it.
- **Cross-platform core test suites** compile under `#if os(iOS) || os(watchOS)`
  (SkippedReminderSyncServiceTests.swift) — reuse fixtures in the new watch target.

**Patterns NOT to follow**:
- Per-key bespoke push methods with different context shapes (SRSS:107-145) —
  being replaced by full-context pushes.
- Out-of-band writes read back through `@AppStorage` (SingleThreadWatchApp.swift:26;
  WatchReminderView.swift:56-57) — observation timing is OS-version-dependent.
- Persisting a synced key without notifying observers (the current
  `showUndatedReminders` / skip receive paths) — that asymmetry is the bug class here.

## Design Decisions

1. **Sync scope = the 4 watch-relevant settings** (Option A): the 7 phone-only
   cosmetics have no watch UI counterpart; syncing them adds payload surface for
   no user-visible effect. Document them as intentionally phone-only.
2. **Generic "apply all keys" receive pipeline** (Option B): one code path persists
   and notifies for every key, eliminating the persist-without-hook and
   hook-without-persist asymmetries as a class rather than patching two symptoms.
3. **`showDate` via explicit callback** (Option A): the receive pipeline notifies a
   watch-side handler that writes `ShowDatePreference` and mutates observed state;
   `@AppStorage` in `WatchReminderView` is replaced by store-driven state.
4. **Full combined context on every push** (Option A): a single `pushAll()` that
   snapshots skips + exclusions + showUndated + sort + showDate at send time.
   Removes the interleaved-shape overwrite risk; simplifies latest-wins reasoning.
5. **New watchOS unit test target** (Option B): watch-side persistence/relaunch
   behavior can't be asserted from iOS-side tests; UI tests alone are too coarse.
   Project-file surgery accepted once.

## What We're NOT Doing

- Not syncing appearance, text size, landscape, mic button, background, or action
  buttons to the watch.
- Not adding timestamps/sequence numbers to the context (full-shape pushes make
  them unnecessary).
- Not changing watch→phone transports (`sendMessage` complete/delete stays as-is).
- Not granting the watch target App Group entitlements — `.standard` local storage
  is fine for watch-side persistence.
- Not touching widget/macOS preference sharing.
- Not refactoring `ReminderStore` beyond adding hooks the pipeline needs.

## Open Risks

- Adding the watchOS unit test target requires Xcode project edits; scheme/CI
  wiring may need iteration.
- Full-context pushes send slightly larger payloads; WCSession context size limits
  (~64KB) are far away but worth a test with a large skip/exclusion set.
- `reload()` triggered by live skip application hits EventKit on the watch;
  frequency is low (only on receive) but should be verified against UI jank in
  the watch UI test.
- Watch UI tests are the only end-to-end proof of live propagation; they can be
  flaky — unit tests must carry the behavioral weight.
