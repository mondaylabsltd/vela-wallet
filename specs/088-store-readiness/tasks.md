# Tasks: 088 — Store submission readiness

**Input**: [spec.md](spec.md), [plan.md](plan.md), [audit.md](audit.md) §4.
Format: `- [ ] T### [P] [US#] description (files) — test`. US1 = builds the
stores accept, US2 = testers can create and sign, US3 = nothing a reviewer
would flag, US4 = the documents are true.

## Phase 1 — Core rules (decided once)

- [x] T001 [US3] Secure-context rule: https, or http on exact loopback (`rust/crates/vela-core/src/app/dapp_permissions.rs` `is_secure_context`) — `tests/app_dapp_permissions.rs::secure_context_is_https_or_exact_loopback`
- [x] T002 [US3] The in-app browser machine reads no message from a frame off a secure context (`dapp_browser.rs::page_message`) — `tests/app_dapp_browser.rs::a_page_off_a_secure_context_is_never_answered`, `a_loopback_test_dapp_may_sign`
- [x] T003 [US3] The injected script installs nothing unless `window.isSecureContext` (`dapp_rpc.rs::provider_script`) — `dapp_rpc::tests::script_is_one_classic_script_per_host`
- [x] T004 [US3] Outside-link rule: an https page with a plain host, and the host to show (`dapp_rpc.rs::external_page_host`; UniFFI `dapp_external_page_host`) — `dapp_rpc::tests::an_external_link_names_the_host_it_will_open`
- [x] T005 [P] Regenerate bindings and artifacts: Kotlin bindings (gitignored), `vela_core_uniffi.swift` + xcframework, dev-fixtures xcframework, `rust/pkg-web`, `assets/wasm` — `build-web --check` current; `cargo test -p vela-core` 2200 passed; workspace 2243 passed; clippy and fmt clean

## Phase 2 — Erase copy (i18n)

- [x] T006 [US3] `settings.eraseDevice.keeps` in 15 locales: what servers keep and for how long, and that the on-chain record can't be deleted; en and ja `desc`/`loses` trimmed (`rust/crates/vela-core/i18n/locales/*.json`) — gen/lint/verify i18n, `dump:vectors`, `i18n_residency` (ja + en 138,631 of 138,800, −40 B)

## Phase 3 — Android

- [x] T007 [US1] Release signing from env vars or the gitignored `keystore.properties`; release fails without them; `-PvelaUnsignedRelease` opt-in (`app/build.gradle.kts`, `.gitignore`) — bundleRelease without the key fails at `velaCheckUploadKey`; with it, the AAB passes `jarsigner -verify`
- [x] T008 [US1] versionCode = `-PvelaVersionCode`, else `VELA_VERSION_CODE`, else commit count (`app/build.gradle.kts`) — merged manifest 3211 (default) and 3212 (override); a bad value fails
- [x] T009 [US1] JNA 5.17.0 and an explicit 16 KB link flag for the 64-bit Rust targets (`gradle/libs.versions.toml`, `rust/scripts/build-android.sh`) — `llvm-readelf -lW` on every 64-bit `.so` in the AAB: 0x4000
- [x] T010 [P] [US3] Release ignores debug launch extras (`LaunchExtras.kt`, `MainActivity.kt`) — `LaunchExtrasTest` (4); release APK on the emulator + gallery/route/page extras → Welcome
- [x] T011 [P] [US3] Outside link asks first (`BrowserController.askToOpen/answerExternal`, `feature/browser/ExternalPageSheet.kt`, `VelaNavHost.kt`, `I18nKeys.kt`) — `BrowserMachineTest` (10); emulator: sheet names `app.uniswap.org`, Cancel dismisses, http refused
- [x] T012 [P] [US3] Tracker expedited only on API 31+ (`feature/wallet/core/TrackerWork.kt`) — `TrackerWorkRequestTest` (2); `TrackerWorkDeviceTest` on the API 30 emulator: SUCCEEDED (the pre-fix request FAILED with "Not implemented")
- [x] T013 [P] CI packaging: `-PvelaUnsignedRelease`, full-history checkout (`.github/workflows/android-package.yml`)
- [x] T014 [US3] Splash shows the app mark on its tile, not Android's default robot (`res/values/themes.xml`) — emulator screenshot
- [x] T015 Unit tests — `:app:testDebugUnitTest` 870 passed, 0 failed

## Phase 4 — iOS

- [x] T016 [P] [US3] Outside link asks first (`RootView.openLink`, `BrowserController.askToOpen/answerExternal`, `Features/Explore/ExternalPageSheet.swift`, `I18nKeys.ExternalPage`) — `ExternalPageTests` (3)
- [x] T017 [P] [US3] Info.plist: true export-compliance comment, Bluetooth string narrowed to caBLE, `NSLocalNetworkUsageDescription` added — archived Info.plist read back
- [x] T018 [P] [US3] iPhone portrait only (`project.pbxproj`) — archived `UISupportedInterfaceOrientations~iphone` = Portrait
- [x] T019 [US1] Archive + App Store export dry run; entitlements checked (`VelaWallet.entitlements` comment) — archive succeeded; export reached `codesign` with the App Store profile and the expected entitlements, then stopped at the macOS keychain prompt, which only the owner can answer
- [x] T020 [US1] Build number from history in CI (`.github/workflows/ios-package.yml`) — archive `CFBundleVersion` 3217
- [x] T021 [US3] No dev fixtures or parallel-space keys in Release — no fixture symbol, no fixture key scalar in the archived binary (the debug fixture library has it, as a control)
- [x] T022 Unit tests — VelaWalletTests on a cloned simulator: 1075 tests in 139 suites passed

## Phase 5 — Site and documents

- [x] T023 [P] [US4] `/delete` and `/support` pages; privacy erase paragraph fixed; footer and sitemap (`app-web/getvela.app`) — `bun run check` 0 errors, vitest 885 passed, `vite build`, local preview 200
- [x] T024 [P] [US4] Review notes and privacy answers (`docs/store-submission/privacy-and-review.md`) — traced to code and privacy-evidence
- [x] T025 [P] [US4] Listing copy from the claim ledger (`docs/store-submission/store-listing-copy.md`) — field lengths counted
- [x] T026 [US2] Owner checklist, with the assetlinks file and line and the review-timing caveat (`owner-checklist.md`)
- [x] T027 Launch checklist §A1 corrected (`docs/NATIVE-LAUNCH-CHECKLIST.md`)
- [x] T028 plan.md, tasks.md, results.md

## Left for the owner (see owner-checklist.md)

- [ ] T029 [US1] Upload with the real key; accept Apple agreements; Play Organization account and Paid choice
- [ ] T030 [US2] Play app-signing SHA-256 into `assetlinks.json/+server.ts` (after line 24) and deploy the site
- [ ] T031 [US2] SC-004 release smoke on a real phone (the Xiaomi), and a TestFlight check of the USB security-key route
