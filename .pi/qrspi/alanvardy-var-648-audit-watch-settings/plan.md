# Implementation Plan — VAR-648: Audit watch settings

Codebase: `~/dev/SingleThread` (Swift/Xcode; Makefile-driven gates). All line refs from
research.md against that root.

## Overview

Replace the three ad-hoc context shapes on `SkippedReminderSyncService` (SRSS) with one
full-combined-context `pushAll()` and a generic receive pipeline (persist + notify per key,
absent keys = no-ops). Close the two asymmetries (watch-side `showUndatedReminders`
persistence, live skip application), make `showDate` deterministic via an explicit callback,
add a watchOS unit test target, and document the 7 phone-only cosmetics as intentionally
unsynced.

### Deviations from structure.md (deliberate)

1. **`ShowUndatedRemindersPreference` is created in Phase 1, not Phase 3.** `pushAll()` must
   snapshot all five keys from *injected stores* (structure.md Phase 1 wording), so the store
   type must exist before `pushAll()` does. Phase 3 then only wires watch-side persistence +
   restore onto it.
2. **Phone-side sort persistence moves to the call site.** Today `pushSortOption(_:)` persists
   the option (`sortStore.save`) before pushing. With `pushSortOption` deleted, the phone app's
   `onSortOptionChanged` hook persists via `SortOptionStore().save(option)` before calling
   `pushAll()`. Behavior preserved.
3. **Phase 5's "live propagation" UI test drives the real WCSession delegate entry point**
   (`service.session(WCSession.default, didReceiveApplicationContext:)`) from a scheduled
   launch-arg seam instead of a paired-phone push — a single-watch-simulator UI test cannot
   run the phone app. Everything downstream of the delegate is production code.

---

## Phase 1: Full-context pushes (`pushAll()`)

### Changes

#### 1. New preference store
**File**: `SingleThreadCore/Sources/SingleThreadCore/ShowUndatedRemindersPreference.swift`
**Action**: create

```swift
import Foundation

/// Persists the user's "show undated reminders" preference, mirroring
/// `SortOptionStore`. An absent key resolves to `false` (today's behavior —
/// undated reminders start hidden).
public struct ShowUndatedRemindersPreference {
    // MARK: Lifecycle

    public init(
        defaults: UserDefaults = AppGroup.defaults,
        key: String = "showUndatedReminders") {
        self.defaults = defaults
        self.key = key
    }

    // MARK: Public

    public func load() -> Bool {
        defaults.object(forKey: key) as? Bool ?? false
    }

    public func save(_ enabled: Bool) {
        defaults.set(enabled, forKey: key)
    }

    // MARK: Private

    private let defaults: UserDefaults
    private let key: String
}
```

Notes: `object(forKey:) as? Bool ?? false` (not `bool(forKey:)`) so an unset key is
distinguishable and defaults stay explicit, matching `ShowDatePreference`'s style. The
default suite is `AppGroup.defaults` (phone/widget share it; watchOS falls back to
`.standard` per `AppGroup.swift:13-15`). Key string matches the existing payload key and the
phone's `@AppStorage("showUndatedReminders", store: AppGroup.defaults)` so the phone's
Settings writes are visible to `load()` without extra plumbing.

#### 2. SRSS: replace four push methods with `pushAll()`
**File**: `SingleThreadCore/Sources/SingleThreadCore/SkippedReminderSyncService.swift`
**Action**: modify

- `init` gains one parameter (before `sendsShowDate`):

```swift
public init(
    session: any SkipSyncSession,
    skipStore: SkippedReminderStore,
    excludeStore: ExcludedProjectStore = ExcludedProjectStore(),
    sortStore: SortOptionStore = SortOptionStore(),
    showUndatedStore: ShowUndatedRemindersPreference = ShowUndatedRemindersPreference(),
    showDateStore: ShowDatePreference = ShowDatePreference(),
    sendsShowDate: Bool = true)
```

- Add matching `private let showUndatedStore: ShowUndatedRemindersPreference`.
- Delete `push(_:showUndatedReminders:)`, `pushExcludedProjectTitles(_:)`,
  `pushSortOption(_:)`, `pushShowDate(_:)` entirely (SRSS:88-145).
- Add:

```swift
/// Pushes a complete snapshot of every synced setting as one latest-wins
/// application context. Sending a single context shape removes the risk that
/// interleaved partial shapes overwrite each other's omitted keys across an
/// interrupted connection.
public func pushAll() {
    do {
        var context: [String: Any] = [
            PayloadKey.skippedReminderIdentifiers: skipStore.load(),
            PayloadKey.excludedProjectTitles: excludeStore.load(),
            PayloadKey.showUndatedReminders: showUndatedStore.load(),
            PayloadKey.sortOption: sortStore.load().rawValue
        ]
        if sendsShowDate {
            context[PayloadKey.showDate] = showDateStore.isEnabled
        }
        try session.updateApplicationContext(context)
    } catch {
        let description = error.localizedDescription
        Self.logger.error("Failed to push sync context: \(description, privacy: .public)")
    }
}
```

Receive path untouched in this phase.

#### 3. Phone wiring
**File**: `SingleThread/SingleThreadApp.swift`
**Action**: modify (inside `#if os(iOS)` block)

```swift
store.onSkipSetChanged = { _ in service.pushAll() }
// The old pushSortOption(_:) persisted the option; that responsibility moves
// here so the pushed snapshot always matches what was just saved.
store.onSortOptionChanged = { option in
    SortOptionStore().save(option)
    service.pushAll()
}
store.onExcludedProjectsChanged = { _ in service.pushAll() }
store.onShowUndatedRemindersChanged = { _ in service.pushAll() }
```

(`onShowUndatedRemindersChanged` currently doesn't exist on the phone — today the push passes
`store.showsUndatedReminders` inline from `onSkipSetChanged`. Wire the hook too: ContentView's
`@AppStorage` write has already hit the App Group suite when the store var's `didSet` fires,
so `pushAll()`'s `showUndatedStore.load()` reads the fresh value.)

And the `.onChange` in `body`:

```swift
.onChange(of: showDate) { _, _ in
    syncService?.pushAll()
}
```

(`@AppStorage` has already written the App Group suite; `pushAll()` snapshots it via
`ShowDatePreference()`.)

#### 4. Watch wiring
**File**: `SingleThreadWatch/SingleThreadWatchApp.swift`
**Action**: modify (in `init`, replace the two send-side hooks)

```swift
store.onSkipSetChanged = { _ in service.pushAll() }
store.onExcludedProjectsChanged = { _ in service.pushAll() }
```

Do **not** wire `onSortOptionChanged` / `onShowUndatedRemindersChanged` on the watch — they
are receive-driven there and wiring them would echo pushes back.

#### 5. Tests
**File**: `SingleThreadTests/SkippedReminderSyncServiceTests.swift`
**Action**: modify

- Update every test that called a deleted push method to seed the relevant injected store
  (isolated `UserDefaults.standard` + UUID key, existing pattern) and call `service.pushAll()`:
  - `pushUpdatesApplicationContext` → rename `pushAllSendsSkipIDs`; seed `skipStore.save(["A","B","C"])`, assert ids.
  - `pushHandlesError` → call `pushAll()` with `fake.pushShouldThrow = true`; assert no crash.
  - `pushCarriesCombinedContext` → fold into the new full-shape test below.
  - `pushSkipIDsIncludesSortOption` + `pushSortOptionIncludesSkipIDs` → replace both with one:

```swift
@Test
func pushAllSendsFullFiveKeyShape() throws {
    let fake = FakeSession()
    let suffix = UUID().uuidString
    let skipStore = SkippedReminderStore(defaults: .standard, key: "test-all-skip-\(suffix)")
    skipStore.save(["X"])
    let excludeStore = ExcludedProjectStore(defaults: .standard, key: "test-all-excl-\(suffix)")
    excludeStore.save(["Work"])
    let showUndatedStore = ShowUndatedRemindersPreference(defaults: .standard, key: "test-all-und-\(suffix)")
    showUndatedStore.save(true)
    let sortStore = SortOptionStore(defaults: .standard, key: "test-all-sort-\(suffix)")
    sortStore.save(.dueDate)
    let showDateStore = ShowDatePreference(defaults: .standard, key: "test-all-date-\(suffix)")
    showDateStore.set(false)
    let service = SkippedReminderSyncService(
        session: fake, skipStore: skipStore, excludeStore: excludeStore,
        showUndatedStore: showUndatedStore, sortStore: sortStore,
        showDateStore: showDateStore, sendsShowDate: true)
    service.pushAll()
    let context = try #require(fake.lastContext)
    #expect(Set(context["skippedReminderIdentifiers"] as? [String] ?? []) == ["X"])
    #expect(context["excludedProjectTitles"] as? [String] == ["Work"])
    #expect((context["showUndatedReminders"] as? Bool) == true)
    #expect(context["sortOption"] as? String == "dueDate")
    #expect((context["showDate"] as? Bool) == false)
}
```

  - `pushExcludedProjectTitlesUpdatesApplicationContext` → assert exclusions ride along in `pushAll` output (or fold into full-shape test; keep one dedicated assertion either way).
  - `pushIncludesShowDate` / `pushShowDateSendsBothKeys` → delete (subsumed by full-shape test).
  - `sendsShowDateFalseOmitSkey` (`sendsShowDateFalseOmitsKey`) → keep, switching the call to `pushAll()`; assert `context["showDate"] == nil` while the other four keys are present.
- Latest-wins / anti-clobber receive tests are untouched and keep passing (receive path unchanged).

### Verification

#### Automated
- [x] `make test` passes (`./scripts/test.sh --unit-only`) — updated SRSS suite green.
- [x] `grep -rn "updateApplicationContext" SingleThread*` shows exactly two call sites: inside `pushAll()` and nowhere else outside `SkippedReminderSyncService.swift`.

#### Manual
- [ ] `make build && make watch-build` both succeed.
- [ ] Confirm no remaining references to `push(`, `pushExcludedProjectTitles`, `pushSortOption`, `pushShowDate` outside tests-history: `grep -rn "pushShowDate\|pushSortOption\|pushExcludedProjectTitles" SingleThread/ SingleThreadWatch/ SingleThreadCore/` returns nothing.

---

## Phase 2: Generic receive pipeline + explicit `showDate` callback

### Changes

#### 1. SRSS: new hook + unified apply path
**File**: `SingleThreadCore/Sources/SingleThreadCore/SkippedReminderSyncService.swift`
**Action**: modify

- Add hook next to the others (same doc-comment pattern):

```swift
/// Hook fired on the counterpart when the "show due date" preference arrives
/// in an application context. Passes the received value. Same
/// write-once-before-activate / `nonisolated(unsafe)` rationale as
/// `onCompleteReminderReceived`.
public nonisolated(unsafe) var onShowDateReceived: ((Bool) -> Void)?
```

- Replace the body of `session(_:didReceiveApplicationContext:)` with a delegation to one
  private routine; every present key follows decode → persist → fire handler; absent/malformed
  keys are no-ops:

```swift
public func session(
    _: WCSession,
    didReceiveApplicationContext applicationContext: [String: Any]) {
    apply(context: applicationContext)
}

/// Single receive path: decode → persist → notify for each present key;
/// absent keys are no-ops. Handlers are snapshotted before invocation because
/// they are written once from the main actor before `activate()`.
private func apply(context: [String: Any]) {
    if let receivedIDs = context[PayloadKey.skippedReminderIdentifiers] as? [String] {
        skipStore.save(receivedIDs)
    }
    if let receivedTitles = context[PayloadKey.excludedProjectTitles] as? [String] {
        excludeStore.save(receivedTitles)
        let handler = onExcludedProjectTitlesReceived
        handler?(receivedTitles)
    }
    if let received = context[PayloadKey.showUndatedReminders] as? Bool {
        let handler = onShowUndatedRemindersReceived
        handler?(received)
    }
    if let rawValue = context[PayloadKey.sortOption] as? String,
       let option = SortOption(rawValue: rawValue) {
        sortStore.save(option)
        let handler = onSortOptionReceived
        handler?(option)
    }
    if let showDate = context[PayloadKey.showDate] as? Bool {
        showDateStore.set(showDate)
        let handler = onShowDateReceived
        handler?(showDate)
    }
}
```

(The skips branch gains its handler and the showUndated branch gains persistence in Phase 3 —
kept minimal here so each phase stays reviewable.)

#### 2. Watch: observable `ShowDateState`
**File**: `SingleThreadWatch/ShowDateState.swift` (new file in the SingleThreadWatch target)
**Action**: create

```swift
import SingleThreadCore
import SwiftUI

/// Observable holder for the watch-rendered "show due date" flag. Replaces the
/// former `@AppStorage("showDate")` read-back, whose observation of out-of-band
/// UserDefaults writes is OS-version-dependent; updates now arrive through the
/// sync pipeline's explicit `onShowDateReceived` callback.
@Observable
final class ShowDateState {
    private let preference = ShowDatePreference(defaults: .standard)
    private(set) var isEnabled: Bool

    init() {
        isEnabled = preference.isEnabled
    }

    /// Persists a received value and publishes it to observing views.
    func apply(_ value: Bool) {
        preference.set(value)
        isEnabled = value
    }
}
```

#### 3. Watch view: drop `@AppStorage`
**File**: `SingleThreadWatch/WatchReminderView.swift`
**Action**: modify

- Remove `@AppStorage("showDate") private var showDate = true` (lines 56-57).
- Add `private let showDateState: ShowDateState` and thread it through both inits with a
  default so the five `#Preview` blocks (lines 229+) stay unchanged:

```swift
init(store: ReminderStore, showDateState: ShowDateState = ShowDateState()) {
    self.store = store
    self.showDateState = showDateState
}
// convenience preview init: same default parameter
```

- Line 173 render condition becomes `if showDateState.isEnabled, let due = ...`.

#### 4. Watch app: wire the callback
**File**: `SingleThreadWatch/SingleThreadWatchApp.swift`
**Action**: modify

- Add stored property `private let showDateState = ShowDateState()` (must be initialized
  before the service closure captures it — declare before `init` body runs; a default-value
  property initializer runs first, so a plain `= ShowDateState()` is safe).
- In `init`, alongside the other handlers (write-once-before-activate):

```swift
service.onShowDateReceived = { [weak showDateState] value in
    showDateState?.apply(value)
}
```

- Body becomes `WatchReminderView(store: store, showDateState: showDateState)`.

#### 5. Tests
**File**: `SingleThreadTests/SkippedReminderSyncServiceTests.swift`
**Action**: modify

```swift
@Test
func receiveContextFiresOnShowDateHandlerAndPersists() {
    let fake = FakeSession()
    let suffix = UUID().uuidString
    let showDateStore = ShowDatePreference(defaults: .standard, key: "test-date-hook-\(suffix)")
    showDateStore.set(true)
    let service = SkippedReminderSyncService(
        session: fake,
        skipStore: SkippedReminderStore(defaults: .standard, key: "test-date-hook-ids-\(suffix)"),
        showDateStore: showDateStore)
    var received: [Bool] = []
    service.onShowDateReceived = { received.append($0) }
    service.session(WCSession.default, didReceiveApplicationContext: ["showDate": false])
    #expect(received == [false])
    #expect(!showDateStore.isEnabled)
}

@Test
func receiveContextAbsentShowDateDoesNotFireHandler() {
    // extend existing receiveContextMissingShowDateLeavesLocalUnchanged with:
    var fired = false
    service.onShowDateReceived = { _ in fired = true }
    // …send a skips-only context…
    #expect(!fired)
}
```

### Verification

#### Automated
- [x] `make test` passes — new `onShowDateReceived` tests plus all existing receive tests (absent-key no-ops, malformed payloads) green.

#### Manual
- [ ] `make watch-build` succeeds.
- [ ] Run phone app + watch sim pair: toggle Settings → Show Date on the phone; the watch's rendered due date appears/disappears immediately, without relaunching the watch app (this is the deterministic-callback proof; previously dependent on `@AppStorage` external-write observation).
- [ ] `grep -n "@AppStorage(\"showDate\")" SingleThreadWatch/` returns nothing.

---

## Phase 3: Close the asymmetries — persist showUndated, apply skips live

### Changes

#### 1. SRSS: persist showUndated, notify skips
**File**: `SingleThreadCore/Sources/SingleThreadCore/SkippedReminderSyncService.swift`
**Action**: modify

- Add hook:

```swift
/// Hook fired on the counterpart when the skipped-reminder identifier array
/// arrives in an application context. Passes the received IDs. Fired **after**
/// the skip store is persisted, so a watch-side handler can simply reload.
public nonisolated(unsafe) var onSkippedIdentifiersReceived: (([String]) -> Void)?
```

- In `apply(context:)`, complete the two branches so every key follows persist → notify:

```swift
if let receivedIDs = context[PayloadKey.skippedReminderIdentifiers] as? [String] {
    skipStore.save(receivedIDs)
    let handler = onSkippedIdentifiersReceived
    handler?(receivedIDs)
}
…
if let received = context[PayloadKey.showUndatedReminders] as? Bool {
    showUndatedStore.save(received)
    let handler = onShowUndatedRemindersReceived
    handler?(received)
}
```

#### 2. Watch app: live skips + relaunch restore
**File**: `SingleThreadWatch/SingleThreadWatchApp.swift`
**Action**: modify

- Service construction gains the store pinned to `.standard` (watch has no App Group
  entitlements by design):

```swift
let service = SkippedReminderSyncService(
    session: WCSession.default,
    skipStore: SkippedReminderStore(),
    showUndatedStore: ShowUndatedRemindersPreference(defaults: .standard),
    showDateStore: ShowDatePreference(defaults: .standard),
    sendsShowDate: false)
```

- Restore persisted showUndated before handlers/activate (mirrors the existing sort restore
  one line above; direct assignment fires the `didSet` hook, which is unwired on the watch —
  no echo):

```swift
store.showsUndatedReminders = ShowUndatedRemindersPreference(defaults: .standard).load()
```

- New handler with the others (write-once-before-activate). `reload()` prunes stale IDs via
  `ReminderSkipLogic.resolve` and re-applies exclusions, so a plain reload applies the whole
  received skip set live:

```swift
// A phone-side skip lands and applies to this watch's live list without a
// relaunch — reload() re-reads the just-persisted skip store and prunes IDs
// whose reminders no longer exist.
service.onSkippedIdentifiersReceived = { [weak store] _ in
    Task { await store?.reload() }
}
```

#### 3. Tests
**File**: `SingleThreadTests/SkippedReminderSyncServiceTests.swift`
**Action**: modify

```swift
@Test
func receiveContextPersistsShowUndatedAndFiresHook() {
    let fake = FakeSession()
    let suffix = UUID().uuidString
    let showUndatedStore = ShowUndatedRemindersPreference(
        defaults: .standard, key: "test-und-persist-\(suffix)")
    let service = SkippedReminderSyncService(
        session: fake,
        skipStore: SkippedReminderStore(defaults: .standard, key: "test-und-persist-ids-\(suffix)"),
        showUndatedStore: showUndatedStore)
    var received: [Bool] = []
    service.onShowUndatedRemindersReceived = { received.append($0) }
    service.session(WCSession.default, didReceiveApplicationContext: ["showUndatedReminders": true])
    #expect(showUndatedStore.load())          // persisted (was hook-only before this phase)
    #expect(received == [true])               // still notified
}

@Test
func showUndatedPersistsAcrossSimulatedRelaunch() {
    // Receive → throw the service away → a fresh store instance reads the value
    // back, proving the value survives process relaunch.
    let key = "test-und-relaunch-\(UUID().uuidString)"
    let fake = FakeSession()
    let service = SkippedReminderSyncService(
        session: fake,
        skipStore: SkippedReminderStore(defaults: .standard, key: key + "-ids"),
        showUndatedStore: ShowUndatedRemindersPreference(defaults: .standard, key: key))
    service.session(WCSession.default, didReceiveApplicationContext: ["showUndatedReminders": true])
    let freshStore = ShowUndatedRemindersPreference(defaults: .standard, key: key)
    #expect(freshStore.load())
}

@Test
func receiveContextFiresSkippedIdentifiersHandlerAfterPersisting() {
    let fake = FakeSession()
    let key = "test-skips-hook-\(UUID().uuidString)"
    let skipStore = SkippedReminderStore(defaults: .standard, key: key)
    let service = SkippedReminderSyncService(session: fake, skipStore: skipStore)
    var received: [[String]] = []
    service.onSkippedIdentifiersReceived = { received.append($0) }
    service.session(WCSession.default, didReceiveApplicationContext: [
        "skippedReminderIdentifiers": ["B", "C"]
    ])
    #expect(Set(skipStore.load()) == ["B", "C"])  // persisted first
    #expect(received == [["B", "C"]])
}
```

Existing tests keep passing: `receiveContextClearPropagates` and
`receiveContextReplacesLocalIDs` now additionally fire the new skip handler (nil in those
tests — no-op), `receiveContextFiresToggleHookAndKeepsSkipIDs` unaffected.

### Verification

#### Automated
- [x] `make test` passes including the three new tests above.

#### Manual
- [ ] Phone ↔ watch pair: toggle Show Undated on the phone → watch list updates live; kill and relaunch the watch app → the setting survives (undated reminders still shown/hidden per the last received value).
- [ ] Skip a reminder on the phone → the watch's list drops it within seconds, without relaunch or manual refresh.

---

## Phase 4: watchOS unit test target

### Changes

#### 1. New test source directory
**File**: `SingleThreadWatchTests/WatchSyncPipelineTests.swift`
**Action**: create

A watchOS-only suite covering what iOS-side tests cannot assert (watch-target compilation of
the pipeline) plus the Phase 2–3 behaviors. It carries its own private `FakeSession` copy —
the original is `private` inside `SingleThreadTests/SkippedReminderSyncServiceTests.swift`
and cannot be imported across bundles:

```swift
import EventKit
import SingleThreadCore
import Testing
import WatchConnectivity

private final class WatchFakeSession: SkipSyncSession {
    var activated = false
    var lastContext: [String: Any]?
    func activate() { activated = true }
    func updateApplicationContext(_ applicationContext: [String: Any]) throws {
        lastContext = applicationContext
    }
    func sendMessage(
        _: [String: Any],
        replyHandler _: (([String: Any]) -> Void)?,
        errorHandler _: ((any Error) -> Void)?) {}
}

@MainActor
struct WatchSyncPipelineTests {
    @Test func pushAllFromWatchOmitsShowDate() throws { /* sendsShowDate:false shape */ }
    @Test func receiveAppliesEveryPresentKey() { /* one context carrying all five keys:
        each store persisted + each handler fired */ }
    @Test func receiveAbsentKeysAreNoOps() { /* skips-only context leaves the other four
        stores untouched and their handlers unfired */ }
    @Test func showUndatedSurvivesRelaunch() { /* Phase 3 relaunch test, watch-native */ }
    @Test func excludedTitlesRefreshFiltersVisibleReminders() { /* reuse the
        inProjectReminder fixture pattern from the iOS suite */ }
}
```

(Bodies mirror the Phase 1–3 test snippets above; implement with the same
`UserDefaults.standard` + UUID-key isolation.)

#### 2. Project surgery — `SingleThread.xcodeproj/project.pbxproj`
**Action**: modify (hand-edit; generate fresh 24-hex-char IDs prefixed `51W` for greppability)

The project already uses Xcode 16 `fileSystemSynchronizedGroups`, so folders self-populate.
Add, mirroring the existing `SingleThreadTests` target (lines 83, 106-109, 242-263):

1. **PBXFileReference** — `SingleThreadWatchTests.xctest` product
   (pattern: line 83, `explicitFileType = wrapper.cfbundle`).
2. **PBXFileSystemSynchronizedRootGroup** for the `SingleThreadWatchTests` folder
   (pattern: lines 106-109).
3. **PBXNativeTarget** `SingleThreadWatchTests`
   (`productType = "com.apple.product-type.bundle.unit-test"`), with `fileSystemSynchronizedGroups`
   listing **both** the new group **and** the existing `SingleThreadTests` group
   (`51AA3EE8302D5C4500960DFC`) so the `#if os(iOS) || os(watchOS)` cross-platform suites
   compile on watchOS too.
4. **PBXContainerItemProxy + PBXTargetDependency** — dependency on the `SingleThreadWatch`
   app target (pattern: lines 19-25, 507-511).
5. **Products group** — add the new `.xctest` reference (line 203 area).
6. **PBXProject `targets` list** — register the new target (line 407 area).
7. **XCConfigurationList + Debug/Release XCBuildConfiguration** pair:

```
BUNDLE_LOADER = "$(TEST_HOST)";
TEST_HOST = "$(BUILT_PRODUCTS_DIR)/SingleThreadWatch.app/SingleThreadWatch";
PRODUCT_BUNDLE_IDENTIFIER = app.alanvardy.SingleThreadWatchTests;
SDKROOT = watchos;
SUPPORTED_PLATFORMS = "watchos watchsimulator";
TARGETED_DEVICE_FAMILY = 4;
SWIFT_VERSION = 6.0;
SWIFT_APPROACHABLE_CONCURRENCY = YES;
SWIFT_UPCOMING_FEATURE_MEMBER_IMPORT_VISIBILITY = YES;
DEVELOPMENT_TEAM = 6NWX2DHB9Q;
CODE_SIGN_STYLE = Automatic;
GENERATE_INFOPLIST_FILE = YES;
```

8. **Package product dependency** — add an `XCSwiftPackageProductDependency` entry for the
   `SingleThreadCore` local package on the new target (pattern: `51AA3F110000000000000002`).

**Fallback if codegen/group-sharing fails** (shared synchronized-group membership across
targets misbehaves, or some `SingleThreadTests` suites don't compile on watchOS): remove the
shared-group membership from step 3 and keep only the new `SingleThreadWatchTests` group —
the standalone `WatchSyncPipelineTests.swift` file carries the behavioral weight. Iterate
exclusions per-suite only if time permits; do not block phases on it.

#### 3. Scheme
**File**: `SingleThread.xcodeproj/xcshareddata/xcschemes/SingleThreadWatch.xcscheme`
**Action**: modify

Add a `TestableReference` for `SingleThreadWatchTests` to the scheme's `TestAction`
(alongside `SingleThreadWatchUITests`), so `xcodebuild test -scheme SingleThreadWatch`
discovers it. If selected-but-failing, run with `-only-testing` (step 4) regardless.

#### 4. Makefile
**File**: `Makefile`
**Action**: modify

Add to `.PHONY` and a new target (placed next to `watch-ui-test`):

```make
watch-test:
	xcodebuild -scheme SingleThreadWatch \
	  -destination '$(WATCH_TEST_SIM)' \
	  -configuration Debug \
	  -derivedDataPath '$(DERIVED_DATA)' \
	  test \
	  -only-testing:SingleThreadWatchTests
```

### Verification

#### Automated
- [x] `make watch-build` succeeds (target compiles as part of scheme build).
- [x] `make watch-test` passes — `SingleThreadWatchTests` bundle runs green on `$(WATCH_TEST_SIM)`.
- [x] `make test` still passes — iOS-side suites unaffected by pbxproj changes.

#### Manual
- [ ] Xcode shows the `SingleThreadWatchTests` target and it runs standalone via Product ▸ Test with the SingleThreadWatch scheme selected.
- [ ] If shared-suite compilation produced watchOS errors in iOS-only suites, confirm the chosen exclusion strategy and record it in the PR description.

---

## Phase 5: Documentation + end-to-end verification

### Changes

#### 1. Document the unsynced cosmetics
**File**: `SingleThread/SettingsView.swift`
**Action**: modify (top-of-file doc comment)

```swift
/// Settings screen. Eleven settings, two persistence tiers, one sync scope.
///
/// Synced to Apple Watch via `SkippedReminderSyncService` (VAR-648): sort
/// option, show-undated, show date, excluded projects, plus the skip set.
///
/// Intentionally **not** synced — these seven are phone-only cosmetics with no
/// watch UI counterpart (design decision: syncing them adds payload surface
/// for no user-visible effect):
/// `appearanceMode`, `textSize`, `allowsLandscape` (iOS-only),
/// `showMicrophoneButton`, `backgroundEnabled`, `backgroundFadePercent`,
/// `enableActionButtons` (iOS-only).
```

Cross-check the list against the research.md Q1 table (rows 1, 2, 4, 5, 6, 7, 8).

#### 2. Live-propagation watch UI test
**File**: `SingleThreadWatch/SingleThreadWatchApp.swift`
**Action**: modify — new launch-arg seam inside `--ui-testing` handling. After the service is
created and activated, schedule a real receive through the WCSession delegate entry point so
everything downstream (persist + notify + reload/refilter) is production code:

```swift
// UI-test seam: delivers a real applicationContext through the WCSession
// delegate entry point a few seconds after launch, proving settings apply
// live (no relaunch) end-to-end in SingleThreadWatchUITests.
if let index = arguments.firstIndex(of: "--ui-testing-live-excluded"),
   index + 1 < arguments.count {
    let project = arguments[index + 1]
    DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
        service.session(
            WCSession.default,
            didReceiveApplicationContext: ["excludedProjectTitles": [project]])
    }
}
```

(When this arg is present, build the plain `uiTestingStore(arguments:)` — **without**
`--ui-testing-excluded` — so the reminder renders first and disappears on receive.)

**File**: `SingleThreadWatchUITests/SingleThreadWatchUITestsFlows.swift`
**Action**: modify — add one test beside `testExcludedProjectDoesNotRenderReminder`:

```swift
@MainActor
func testLiveExclusionHidesReminderWithoutRelaunch() {
    let app = XCUIApplication()
    app.launchArguments = ["--ui-testing", "--ui-testing-live-excluded", "Work"]
    app.launch()

    // Before the delayed context arrives the card is visible…
    XCTAssertTrue(
        app.staticTexts["Buy groceries"].waitForExistence(timeout: 5),
        "Seeded card should render before the exclusion context arrives")
    // …then the live receive path filters it without an app relaunch.
    XCTAssertTrue(
        app.staticTexts["All Done"].waitForExistence(timeout: 10),
        "Receiving an exclusion context should hide the card live")
}
```

Generous timeout (10 s) absorbs the 2 s delay plus reload/refilter; UI-test flakiness is the
known risk (design.md) — unit tests carry the behavioral weight if this proves unstable.

### Verification

#### Automated
- [x] `./scripts/test.sh` (full gate: format, lint, unit, UI) passes.
- [x] `make watch-test` passes.
- [x] `make watch-ui-test` passes including `testLiveExclusionHidesReminderWithoutRelaunch`.

#### Manual
- [ ] Read the new SettingsView doc comment against the research.md Q1 table — the 7 unsynced keys listed match rows 1, 2, 4, 5, 6, 7, 8 and the 4 synced ones match rows 3, 9, 10, 11.
