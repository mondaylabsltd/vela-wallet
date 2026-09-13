# Tasks: Android Port Completion and First-Run Parity

## Phase 0 — Preferences and formats
- [X] T001 `core/data/Preferences.kt` (web keys, defaults, flows) + `KeyValueStore.allKeys()`; `PreferencesTest`
- [X] T002 [P] `core/format/Formats.kt` (number/date/time presets ported from `locale-format.ts`, `auto` from the locale, text-scale factors) + `FormatsTest`
- [X] T003 Apply: `WalletLive.Money` + day headers + detail dates + send figures through `Formats`; theme font scale from text scale; avatar style read where the identicon/initials choice is drawn

## Phase 1 — US1 Settings live
- [X] T004 Language/number/date/time/text-scale/avatar sheets → `Preferences`; language → `i18nRuntime.setLocale` / system
- [X] T005 Accounts sheet rows from the session view; tap switches; primary/secondary routes
- [X] T006 `feature/settings/core/DeviceStorage.kt` (key → item mapping, sizes, records, clear item, clear caches) + `DeviceStorageTest`; the storage page live; connections group from `vela.perm.*` with disconnect
- [X] T007 (the erase itself not run on the test phone — it holds the founder's own account and contacts; `DeviceStorageTest` covers the sweep) Erase device: sweep + verify + sign-out; failure callout; `EraseTest` on a FakeStore
- [X] T008 Relayer panel from `probeTreasury`; RPC banner from the pool view; RpcFix → reset endpoints; health pills verified live
- [X] T009 About (BuildConfig version/commit via a gradle buildConfigField, network count) and Feedback (real preview lines, ACTION_VIEW to the prefilled issue)
- [X] T010 Device: SC-001, SC-002

## Phase 2 — US2 Share card
- [X] T011 `feature/flows/ShareCardRenderer.kt` (GraphicsLayer → PNG), receive sheet's 保存图片 → documents.share; a live `shareCard` builder (address, name, network)
- [X] T012 Device: SC-003

## Phase 3 — US3 Deep links
- [X] T013 Manifest scheme/host filters + `singleTop` + `onNewIntent`; `PayLinkRouter` (query → `LinkOpened` on a fresh `PaymentRequestCore` → `PayRequest`) + `PayLinkRouterTest`; nav: locked Send / browser
- [X] T014 Device: SC-004

## Phase 4 — US4 Stability
- [X] T015 ~~`ConnectivityWatch`~~ → `NetHealth` (the manifest refuses ACCESS_NETWORK_STATE; offline = three unreached calls) + the home offline line + refresh on reconnect; `CrashReport` handler + the failure sheet on relaunch + core faults; a debug `vela.testPanic` extra; version line
- [X] T016 Device: SC-005, SC-006

## Phase 5 — Rulers, pass, papers
- [X] T017 `scripts/check-android-event-parity.mjs` + `scripts/check-android-dropped-judgement.mjs`; tables into results.md; strong diffs closed or named
- [X] T018 Scripted device pass 040–047 with screenshots; `docs/KNOWN-BUGS.md`; takeover 02 Android section; `docs/android/install-verify-loop.md`
- [X] T019 Gates + results.md + memory

## Phase 8 — the founder's notes of 2026-09-12 (device-verified on the Xiaomi)

- [X] T020 Settings home without the 通讯录 and 反馈 rows (`feature/settings/SettingsFixtures.kt`)
- [X] T021 Logos as the web draws them: `core/marks/Marks.kt` (chainLogoURL, nativeCoinLogoChainId, badge-hidden rule, checksummed-then-lowercase token paths) + `RemoteLogo`/`LogoStore` (shared client, disk cache, drawn fallback); `TokenMarkModel`/`AssetRowModel`/`NetworkRowModel`/`ChainMarkModel` carry the URLs; `WalletLive.mark` is the one builder; `MarksTest`
- [X] T022 Home account switcher: `WalletHeaderRow.onSwitcher` → `AccountSwitcherSheet` (the settings sheet body) fed by `WalletLive.accountSwitcher` (session accounts + the balance machine's `switcher.balances`); `BalanceEvent.SwitcherOpened/Closed` dispatched
- [X] T023 「点击探索没反应」: `section` hoisted to the NavHost, pushed routes' tab bars answer every tab, `popUnlessRoot`/`swapOverWallet` so a double tap never empties the NavHost (`navigation/VelaNavHost.kt`)
