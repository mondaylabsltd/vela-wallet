# Tasks: 095 — Mac App Store (TestFlight)

**Input**: [spec.md](spec.md), [plan.md](plan.md). Format:
`- [x] T### [US#] description (files) — test / evidence`. US1 = a package the
store accepts, US2 = sandboxed still works, US3 = web pages behave, US4 =
nothing a reviewer would flag.

## Phase 1 — Private API (blocker)

- [x] T001 [US1] Vendor `gpui_macos` at zed `c97b7c0`, byte-identical, manifest flattened; `[patch."https://github.com/zed-industries/zed"]` (`app-desktop/vendor/gpui_macos/`, `vela-wallet/Cargo.toml`) — lock: only the source line changes
- [x] T002 [US1] Remove `CGSMainConnectionID`/`CGSSetWindowBackgroundBlurRadius` and caller, `_opaqueRectForWindowMoveWhenInTitlebar` override + field, `_windowResize…Cursor` (→ macOS 15 `frameResizeCursorFromPosition:inDirections:`) (`vendor/gpui_macos/src/window.rs`) — `nm -u` / strings on the release binary
- [x] T003 [US1] `scripts/check-store-binary.sh` (private imports, private selectors, developer switch names from the source) + CI step on the universal binary (`desktop-macos-packages.yml`) — fails on `main`'s binary (2 imports, 3 selectors, 26 switches), passes on this branch's

## Phase 2 — Developer switches (owner rule)

- [x] T004 [US4] `dev_env::var!/var_os!/flag!`; all 26 existing `VELA_*` reads routed through it (`src/dev_env.rs`, 14 files; 27 with `VELA_SANDBOX_PROBE`) — `dev_env::tests::a_developer_build_reads_the_switches`; release binary carries none of the names (T003)

## Phase 3 — Mac App Store package (blocker)

- [x] T005 [US1] `packaging/macos/entitlements-mas.plist` (sandbox + exactly what the app uses)
- [x] T006 [US1] `packaging/macos/PrivacyInfo.xcprivacy` from a `nm -u` measurement; data types = iOS
- [x] T007 [US2] `packaging/macos/container-migration.plist` — measured with a throwaway sandboxed bundle: folder moved, nothing left
- [x] T008 [US1] `scripts/build-macos-mas.sh` (store + `--dev`), `Info.plist.in` ATS + copyright; CI plist lint — `--dev` bundle built, signature and entitlements verified; shellcheck clean
- [ ] T009 [US1] Store run with the MAS profile → `.pkg` → `altool --validate-app` — **owner** (no MAS profile, no API key on this Mac)

## Phase 4 — Sandboxed behaviour

- [x] T010 [US2] Save panels open in Downloads (`executor/storage.rs::save_panel_dir`, `wallet/money.rs`, `wallet/page.rs`) — `save_panels_open_in_downloads`; sandboxed panel shows Downloads and writes the file
- [x] T011 [US2] Developer sandbox probe (`src/sandbox_probe.rs`, `ctap/usb.rs::hid_device_count`) — probe output in results.md
- [x] T012 [US2] Sandbox smoke on the `--dev` bundle in `/Applications` + a `dev-fixtures` twin — results.md §Sandbox

## Phase 5 — Browser (macOS)

- [x] T013 [US3] New windows → tabs; `mailto:`/`tel:` → system on a link in the top document (`src/webview.rs` `mac_leaves`, `block2`) — existing rule tests (`a_new_window_is_a_tab_only_on_a_gesture`, `another_app_opens_only_on_a_tap_in_the_page_itself`); sandboxed run: tabs open, timer popup blocked, mailto/tel reach the opener
- [x] T014 [US3] http pages load without the wallet (`Info.plist.in` ATS) — LAN page: `isSecureContext` false, no `window.ethereum`

## Phase 6 — Menus and About

- [x] T015 [US4] Corpus: `about.link{Privacy,Terms,Support}`, `componentsUi.appMenu.*` in 15 locales; ledger 1809; `SC005_BUDGET` 140,800 — gen/lint/verify i18n, dump:vectors, residency test
- [x] T016 [US4] About links on desktop (`settings/fixtures.rs`), iOS (`SettingsFixtures.swift`), Android (`SettingsFixtures.kt`), web (`settings/fixtures.ts`) — `about_links_the_policy_terms_and_support`, `theAboutPageLinksThePolicyTermsAndSupport`, `about links the privacy policy, terms and support`, `Settings → About` (15 locales), `About’s links are links`
- [x] T017 [US3/US4] `src/app_menu.rs`: About, Edit, View, Window (+ Minimize), localized, relabelled on language change; AppKit's duplicate full-screen item off — `cmd_w_closes_the_window`, `edit_chords_belong_to_the_menu_only`, `the_menus_are_standard_and_localized`; AX read of the menu bar (en, zh); Edit → Select All selects the web page

## Phase 7 — Artefacts, docs, suites

- [x] T018 Regenerate i18n catalogs, vectors, wasm package; Swift bindings and TS mirrors unchanged — `build-web --check`, `gen-onboarding-types --check`
- [x] T019 `docs/store-submission/mac-app-store.md`; `privacy-evidence.md` §9
- [x] T020 Store screenshot recipe + draft captures (results.md)
- [x] T021 Suites: desktop, core, web, iOS, Android, CI scripts — results.md

## Phase 8 — Owner decisions (2026-10-02, second round)

- [x] T022 Universal Purchase price with the free `.dmg`: keep — recorded in the store doc §11
- [x] T023 `CFBundleLocalizations` (+ `CFBundleDevelopmentRegion` en) on the Mac (`packaging/macos/Info.plist.in`) and iPhone (`VelaWallet/Info.plist`), from the core's `vela_core::i18n::apple_localizations` (UniFFI `i18n_apple_localizations`) — core `every_supported_locale_has_one_apple_code`; desktop `loc::tests::the_bundle_declares_the_corpus_locales`; iOS `LocaleMappingTests.theBundleDeclaresTheCorpusLocales` (also: each declared code resolves back to its corpus language)
- [x] T024 `SC005_BUDGET` 141,800 and the gen-i18n log line
- [x] T025 Desktop "follow the system" on macOS reads the person's preferred languages (`CFLocaleCopyPreferredLanguages`) before `LC_ALL`/`LANG`, resolved by the core's new `system_language`/`match_system_tag` (moved out of iOS `Loc.mapPreferredLanguage` and Android `LocaleResolver`, which now call it through UniFFI `i18n_system_language`); pinned language first; Windows/Linux unchanged (`src/loc.rs`, `src/wallet/page.rs`, `Loc.swift`, `LocaleResolver.kt`) — core `a_platform_tag_finds_its_shipped_locale`, `the_first_served_preference_wins`; desktop `the_systems_preferred_languages_pick_the_language`, `without_a_preferred_list_the_environment_decides_as_before`, `macos_reports_preferred_languages`; iOS `LocaleMappingTests` (existing fixtures + `theWholePreferredListIsWalked`); Android `LocaleResolverTest`

