# Structure Outline

## Approach
Replace the three ad-hoc context shapes on `SkippedReminderSyncService` (SRSS) with one
full-combined-context push and a generic receive pipeline that uniformly persists + notifies
per key (absent keys = no-ops). Then close the two asymmetries the audit found — watch-side
`showUndatedReminders` persistence and live skip application — make `showDate` deterministic
via an explicit callback instead of `@AppStorage` external-write observation, add a watchOS
unit test target, and document the 7 phone-only cosmetics as intentionally unsynced.
Codebase: `~/dev/SingleThread`.

---

## Phase 1: Full-context pushes (`pushAll()`)

Delivers end-to-end: every settings change on either device sends **one** complete context
(skips + exclusions + showUndated + sort + showDate), eliminating the interleaved-shape
overwrite risk. Receive path is unchanged, so this is safe to ship alone.

**Files**: `SingleThreadCore/Sources/SingleThreadCore/SkippedReminderSyncService.swift`,
`SingleThread/SingleThreadApp.swift`, `SingleThreadWatch/SingleThreadWatchApp.swift`,
`SingleThreadTests/SkippedReminderSyncServiceTests.swift`

**Key changes**:
- `func pushAll()` — new; snapshots all five keys from the injected stores at send time
  (replaces `push(_:showUndatedReminders:)`, `pushExcludedProjectTitles(_:)`,
  `pushSortOption(_:)`, `pushShowDate(_:)` — deleted)
- Call sites: `store.onSkipSetChanged` / `onExcludedProjectsChanged` / `onSortOptionChanged`
  hooks and the `.onChange(of: showDate)` handler now call `pushAll()`
- Tests updated: full-shape assertions replace per-key context assertions; existing
  latest-wins / anti-clobber tests become trivially satisfied (keep as regression guards)

**Verify**: `make test` passes (all 29 SRSS tests, updated); manual: build both targets,
confirm no other `updateApplicationContext` call sites remain (`grep -rn "updateApplicationContext"`).

---

## Phase 2: Generic receive pipeline + explicit `showDate` callback

Delivers end-to-end: one uniform receive path — persist then notify for every present key,
absent keys are no-ops — and `showDate` updates deterministically via callback instead of
relying on `@AppStorage` observing an out-of-band write.

**Files**: `SingleThreadCore/Sources/SingleThreadCore/SkippedReminderSyncService.swift`,
`SingleThreadWatch/SingleThreadWatchApp.swift`, `SingleThreadWatch/WatchReminderView.swift`,
`SingleThreadTests/SkippedReminderSyncServiceTests.swift`

**Key changes**:
- `nonisolated(unsafe) var onShowDateReceived: ((Bool) -> Void)?` — new hook (same
  write-once-before-activate invariant as the others)
- `session(_:didReceiveApplicationContext:)` restructured into one private
  `apply(context:)` path; each key: decode → persist → fire handler (or no-op if absent)
- Watch side: handler for `onShowDateReceived` writes `ShowDatePreference(defaults: .standard)`
  and mutates observed UI state; `@AppStorage("showDate")` removed from `WatchReminderView`

**Verify**: `make test` passes with new per-key unit tests (persist asserted + handler invoked +
absent-key no-op, using `FakeSession`); manual: toggle Show Date in phone Settings, watch
rendered date visibility changes without relaunch.

---

## Phase 3: Close the asymmetries — persist showUndated, apply skips live

Delivers end-to-end: all four synced settings now behave identically on the watch — live on
receive **and** persisted across relaunch. This is the user-visible bug-fix phase.

**Files**: `SingleThreadCore/Sources/SingleThreadCore/` (new small preference store following
the `SortOptionStore` pattern), `SkippedReminderSyncService.swift`,
`SingleThreadWatch/SingleThreadWatchApp.swift`, `SingleThreadTests/` (new store tests)

**Key changes**:
- `ShowUndatedRemindersPreference { func load() -> Bool; func save(Bool) }` — new watch-side
  persistence (`.standard`; watch has no App Group entitlements by design)
- Pipeline's showUndated branch: persist to the new store **then** invoke
  `onShowUndatedRemindersReceived` (currently hook-only)
- Pipeline's skips branch: invoke a new `onSkippedIdentifiersReceived: (([String]) -> Void)?`
  after persisting; watch handler triggers `store.reload()` so skips apply without relaunch
- Watch init restores persisted showUndated value before `activate()`

**Verify**: `make test` passes including a simulated-relaunch test (save → new service/store
instance → load returns value) and a live-skip test (handler fires on receive); manual:
toggle Show Undated on phone, kill+relaunch watch app — setting survives.

---

## Phase 4: watchOS unit test target

Delivers end-to-end: watch-side pipeline/persistence behavior is assertable from unit tests,
not only coarse UI tests. Project-file surgery happens once, here.

**Files**: `project.pbxproj` (new target), new `SingleThreadWatchTests/` directory,
Makefile (`watch-test` target)

**Key changes**:
- New watchOS unit test target compiling the cross-platform suites under
  `#if os(iOS) || os(watchOS)` plus Phase 2–3 pipeline tests
- `watch-test:` Makefile target: `xcodebuild test -scheme SingleThreadWatch -destination '$(WATCH_TEST_SIM)'`

**Verify**: `make watch-test` passes; manual: confirm scheme appears in Xcode and runs standalone.

---

## Phase 5: Documentation + end-to-end verification

Delivers end-to-end: the sync contract is documented (4 synced settings, 7 phone-only
cosmetics intentionally excluded) and live propagation is proven by watch UI tests.

**Files**: `SingleThread/SettingsView.swift` or `README.md` (doc comment listing unsynced
cosmetics), `SingleThreadWatchUITests/SingleThreadWatchUITestsFlows.swift`

**Key changes**:
- Doc comment block enumerating phone-only settings and why they don't sync (no watch UI
  counterpart — design decision #1)
- One UI test: change a setting seed/launch arg on the phone side fixture → assert watch list
  reflects it without relaunch (reuse existing `--ui-testing` seams)

**Verify**: `make test && make watch-test && make watch-ui-test` all pass; manual: read the
doc comment against the settings table in research.md Q1.

---

## Testing Checkpoints

After each phase, true statements useful for resuming:

1. **Phase 1**: Only one push method exists (`pushAll`); every context sent contains all five
   payload keys; all 29 SRSS tests pass. No receive-path behavior changed yet.
2. **Phase 2**: Receive path is a single generic apply routine; `onShowDateReceived` exists;
   watch UI no longer reads `showDate` via `@AppStorage`.
3. **Phase 3**: Every received key persists *and* notifies on the watch; showUndated survives
   relaunch; skips apply without relaunch.
4. **Phase 4**: `make watch-test` green — watch-side suites run as unit tests.
5. **Phase 5**: All gates green (`make test`, `make watch-test`, `make watch-ui-test`);
   unsynced-cosmetics doc exists.

Note: nothing here resists vertical slicing — each phase crosses core service + app wiring +
tests. The only horizontal-ish step is Phase 4's project surgery, which is deliberately
isolated so its risk doesn't contaminate behavioral phases.
