# Tasks: Android Port Completion and First-Run Parity

## Phase 0 — Preferences and formats
- [ ] T001 `core/data/Preferences.kt` (web keys, defaults, flows) + `KeyValueStore.allKeys()`; `PreferencesTest`
- [ ] T002 [P] `core/format/Formats.kt` (number/date/time presets ported from `locale-format.ts`, `auto` from the locale, text-scale factors) + `FormatsTest`
- [ ] T003 Apply: `WalletLive.Money` + day headers + detail dates + send figures through `Formats`; theme font scale from text scale; avatar style read where the identicon/initials choice is drawn

## Phase 1 — US1 Settings live
- [ ] T004 Language/number/date/time/text-scale/avatar sheets → `Preferences`; language → `i18nRuntime.setLocale` / system
- [ ] T005 Accounts sheet rows from the session view; tap switches; primary/secondary routes
- [ ] T006 `feature/settings/core/DeviceStorage.kt` (key → item mapping, sizes, records, clear item, clear caches) + `DeviceStorageTest`; the storage page live; connections group from `vela.perm.*` with disconnect
- [ ] T007 Erase device: sweep + verify + sign-out; failure callout; `EraseTest` on a FakeStore
- [ ] T008 Relayer panel from `probeTreasury`; RPC banner from the pool view; RpcFix → reset endpoints; health pills verified live
- [ ] T009 About (BuildConfig version/commit via a gradle buildConfigField, network count) and Feedback (real preview lines, ACTION_VIEW to the prefilled issue)
- [ ] T010 Device: SC-001, SC-002

## Phase 2 — US2 Share card
- [ ] T011 `feature/flows/ShareCardRenderer.kt` (GraphicsLayer → PNG), receive sheet's 保存图片 → documents.share; a live `shareCard` builder (address, name, network)
- [ ] T012 Device: SC-003

## Phase 3 — US3 Deep links
- [ ] T013 Manifest scheme/host filters + `singleTop` + `onNewIntent`; `PayLinkRouter` (query → `LinkOpened` on a fresh `PaymentRequestCore` → `PayRequest`) + `PayLinkRouterTest`; nav: locked Send / browser
- [ ] T014 Device: SC-004

## Phase 4 — US4 Stability
- [ ] T015 `ConnectivityWatch` + the home offline line + refresh on reconnect; `CrashReport` handler + the failure sheet on relaunch + core faults; a debug `vela.testPanic` extra; version line
- [ ] T016 Device: SC-005, SC-006

## Phase 5 — Rulers, pass, papers
- [ ] T017 `scripts/check-android-event-parity.mjs` + `scripts/check-android-dropped-judgement.mjs`; tables into results.md; strong diffs closed or named
- [ ] T018 Scripted device pass 040–047 with screenshots; `docs/KNOWN-BUGS.md`; takeover 02 Android section; `docs/android/install-verify-loop.md`
- [ ] T019 Gates + results.md + memory
