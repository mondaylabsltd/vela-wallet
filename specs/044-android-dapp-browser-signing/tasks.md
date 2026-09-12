# Tasks: Android dApp Browser and Signing

**Input**: `plan.md`, `research.md` (D1–D15), `data-model.md`, `contracts/`,
`quickstart.md`. Paths are repository-relative; `…/` is
`app-android/vela-wallet/app/src/main/java/app/getvela/wallet/`.

**Standing rules**: every phase ends with a device pass recorded in
`results.md` (what the device showed, `uiautomator` text, screenshot name);
every Rust edit is followed by `cargo fmt` and, at closeout, the web wasm
rebuild; `-PvelaSkipRustBuild` only when `rust/` is untouched since the last
NDK build; `JAVA_HOME` = Android Studio's JBR.

## Phase 0 — Measure and the bridge (Setup)

- [ ] **T001** Baselines into `specs/044-android-dapp-browser-signing/results.md`:
      arm64 `.so` at 043's tip (16,187,032), unit-test count (431), the
      system WebView version on the Xiaomi (`dumpsys package com.google.android.webview`),
      whether `DOCUMENT_START_SCRIPT` is supported there.
- [ ] **T002** `rust/crates/vela-core-uniffi/src/onboarding_bridge.rs` — six
      `bridge_object!`s: `DappPermissionsCore`, `ExploreSitesCore`,
      `BrowserHistoryCore`, `SignRequestCore`, `ClearSigningCore`,
      `ApprovalGuardCore`.
- [ ] **T003** `rust/crates/vela-core-uniffi/src/lib.rs` — export
      `dapp_origin_of(url) -> Option<String>` (the core's `origin_of`);
      `cargo fmt`; regenerate Kotlin bindings; full NDK build.
- [ ] **T004** [P] `…/feature/browser/core/DpermWire.kt` — events (11),
      operations (8), results (2), `DpermPageEvent`, `DpermRespondPayload`,
      `DpermRejectReason`, `DpermGrant`, `DpermView`/`DpermConsentView`/
      `DpermPopupView`, from the ts-rs mirrors.
- [ ] **T005** [P] `…/feature/browser/core/ExploreWire.kt` + `BhistWire.kt` —
      events (16 / 5), operations (2 / 3), results (2 / 2), `ExploreDoc`,
      `ExploreSite`, `ExploreGroup`, `ExploreTab`, `ExploreView`,
      `BhistEntry`, `BhistView`.
- [ ] **T006** [P] `…/feature/signing/core/SignWire.kt` — events (13),
      operations (7), results (7), `SignView`/`SignRequestView`/
      `SignFundingView`, `SignAccountRef`, `SignApproveOpts`,
      `SignResponsePayload`, `SignRecord`, `SignSubmitOutcome`, …
- [ ] **T007** [P] `…/feature/signing/core/ClearWire.kt` + `GuardWire.kt` —
      clear: events (5), operations (5), results (5), `ClearSigningView`,
      `ClearMessageView`, `ClearSignResult`, `ClearSignField`, …; guard:
      events (11), operations (3), results (3), `GuardView`,
      `GuardEditorView`, `GuardLegView`, `GuardDetectedApproval`, ….
- [ ] **T008** `…/core/crux/CoreBridge.kt` — `asBridge()` for the six;
      `app/src/test/…/CoreWireDriftTest.kt` — the six families (views
      subsets, operations/results/closed families exhaustive).
- [ ] **T009** `app/src/test/…/BridgeSmokeTest.kt` — each of the six is
      created, receives its first event, answers a view, no fault.

**Checkpoint**: 431 → ≥ 437 tests; drift gate green for six families.

## Phase 1 — Foundational: the engine and the provider (US0)

- [ ] **T010** `app-android/vela-wallet/gradle/libs.versions.toml` +
      `app/build.gradle.kts` — `androidx.webkit`; a `syncVelaProviderAssets`
      task copying `app-web/vela-wallet/extension/inpage.js` and
      `lib/protocol.js` into `app/src/main/assets/provider/` (like
      `syncVelaI18nAssets`); `.gitignore` the synced copies.
- [ ] **T011** `…/feature/browser/core/ProviderBridge.kt` — the provider
      bundle (`provider_script()`'s IIFE) + the bridge script (`BRIDGE_JS`
      with `VelaHost.post`), injected via `WebViewCompat.addDocumentStartJavaScript`
      (fallback `onPageStarted`, logged `browser.inject path=…`); the
      `@JavascriptInterface` sink; `deliver(json)` via `evaluateJavascript`;
      top-frame-only in the script.
- [ ] **T012** `…/feature/browser/core/DappRpc.kt` — `Route`, the three
      method sets, `route(method)`, `switchChainParam`, `hexChainId`
      (desktop `dapp_rpc.rs`).
- [ ] **T013** [P] `app/src/test/…/DappRpcParityTest.kt` — parses
      `extension/lib/protocol.js` (via `vela.repo.root`) and compares the
      sets; `BUNDLER`/`READ_ONLY`/`READ_PROXY` blocks.
- [ ] **T014** `app/src/debug/res/xml/network_security_config.xml` +
      debug manifest — cleartext to `127.0.0.1`/`localhost` only;
      `app-android/vela-wallet/dev/testdapp/index.html` — Connect, Sign,
      Block number, Send dust, Approve unlimited, printing every answer and
      every event.
- [ ] **T015** `…/feature/explore/…` — a `BrowserPage` composable
      (`AndroidView` hosting the tab's engine) in the browsing slot where
      `DemoPage` draws; the drawn `DemoPage` stays for the gallery.
- [ ] **T016** `[device]` SC-001: the test page prints `announced: Vela`
      and `window.ethereum: present`; `browser.inject path=document-start`
      (or the fallback, recorded).

**Checkpoint**: a page in the app sees the wallet.

## Phase 2 — User Story 1: browse with a memory (P1)

- [ ] **T017** `…/feature/browser/core/ExploreExecutor.kt` +
      `BhistExecutor.kt` — `vela.explore` / `vela.browserHistory` read and
      write in the shared shape.
- [ ] **T018** `…/feature/browser/core/BrowserController.kt` — hosts
      explore + bhist (+ dperm, phase 3); tab engines keyed by
      `ExploreTab.id`; `VisitRecorded` on document load with the WebView's
      title/favicon; intents: open URL, back/forward, new/close/select tab,
      favourite add/remove/rename, group create/rename/delete/hide/member.
- [ ] **T019** `…/feature/browser/ExploreLive.kt` — `ExploreView` +
      `BhistView` + engines → `ExploreScreenModel` (E1–E6: start empty /
      with favourites, group manage sheet, browsing, tabs, site menu).
- [ ] **T020** `…/navigation/VelaNavHost.kt` + `…/feature/explore/ExploreScreen.kt`
      — the 探索 section reads `ExploreLive`; callbacks to the controller;
      leaving the section leaves the `AndroidView` (FR-013).
- [ ] **T021** [P] `app/src/test/…/ExploreMachineTest.kt`,
      `ExploreLiveTest.kt` — favourites/groups/tabs round-trip; recents
      deduped by origin; the live model from a view.
- [ ] **T022** `[device]` SC-008: favourite + group + second tab survive
      `am force-stop`; leave/return keeps the page; a wallet-tab screenshot
      has no page pixels.

## Phase 3 — User Story 2: connect (P1)

- [ ] **T023** `…/feature/browser/core/BrowserExecutor.kt` — dperm's eight
      arms per `contracts/shell-operations.md`; `ForwardToSigning` routes
      with `DappRpc` (Sign → phase 4's controller; State / Switch / Ack /
      Read / Unsupported answered here).
- [ ] **T024** `BrowserController` — the dperm host; `ProviderRequest` from
      the bridge with `origin = dappOriginOf(webView.url)`; `NavigationStarted`
      on every load and once at birth; `AccountsUpdated`/`AccountSwitched`
      from the session; `ChainChanged` from settings; `RevokeRequested`.
- [ ] **T025** `ExploreLive` + `ExploreScreen` — the consent card (E7's
      sheet, not-yet-connected form) from `DpermConsentView`: origin as the
      fact, claimed name/icon as claims, account, network; approve/reject
      callbacks; the connected pill and the connection sheet from
      `DpermView` (E7).
- [ ] **T026** [P] `app/src/test/…/BrowserMachineTest.kt` — never-connected
      → consent → approve answers one address; second ask instant; dismiss
      → 4001 once; a read proxied through a fake pool; switch to a known
      chain emits `chainChanged`; unknown → 4902; revoke → `disconnect` and
      consent again; grant keyed by origin, not URL.
- [ ] **T027** `[device]` SC-002 + SC-003 + SC-007: the test dApp connects
      and prints `0x88cC…6894`; second Connect without a sheet; Block number
      equals the pool's; `app.uniswap.org` shows the address; revoke → the
      page prints `disconnect`.

## Phase 4 — User Story 3: sign, and it lands (P1) 🎯 SC-304's answer

- [ ] **T028** `…/feature/send/core/UserOpSpine.kt` — extracted from
      `SendExecutor.submitInner` (same order, same ports); `SendExecutor`
      calls it; `SendMachineTest` still green (one implementation, D7).
- [ ] **T029** `…/feature/signing/core/SignExecutor.kt` — the seven arms:
      `SendResponse` → the owning tab's bridge; `CheckBundlerFunding` →
      `probeTreasury`; `AttemptSponsorship` (off in the parallel space);
      `SignAndSubmit` → `UserOpSpine`, `OpSubmitted` mid-flight, the
      receipt's tx hash at the end; `PersistRecord`/`UpdateRecord` →
      `FeedExecutor` (`dapp_tx`); `SwitchActiveAccount` verified.
- [ ] **T030** [P] `…/feature/signing/core/ClearExecutor.kt` —
      descriptors (`HttpGet`), selector DBs (openchain → 4byte, cached),
      `RpcEthCall` via the pool, `Now`/`Timer`.
- [ ] **T031** [P] `…/feature/signing/core/GuardExecutor.kt` — metadata via
      `aggregate3`, allowance/balance `eth_call`s.
- [ ] **T032** `…/feature/signing/core/SigningController.kt` — born per
      forwarded request: sign + clear + guard + fee hosts; `NetworksChanged`
      and `AccountsChanged` BEFORE `RequestArrived`; `clear_kickoff` by
      method; `approve_opts` from the three views; dies with the request.
- [ ] **T033** `…/feature/signing/SigningLive.kt` — `SignView` +
      `ClearSigningView` + `GuardView` + `FeeView` → `SigningScreenModel`
      (dApp identity = the host; blocks; tech disclosure; fee; signer;
      confirm label/enabled; status/funding blocks).
- [ ] **T034** `VelaNavHost.kt` — the sheet raised by a forwarded request
      over the browser (and over the wallet if the request arrives while
      away); slide → `ApproveTapped`; close/back/swipe → one refusal; the
      tracker handoff to `WalletController.trackSubmitted`.
- [ ] **T035** [P] `app/src/testDebug/…/DappSignMachineTest.kt` — a page's
      `eth_sendTransaction` through the real machines with the fixture
      keyset: sheet facts, one signature, record before response, the
      page answered with the hash; reject → 4001 once, nothing written.
- [ ] **T036** `[device]` SC-004: Send dust from the test dApp → the sheet
      → slide → hash printed by the page → pending row → confirmed; the
      relay receipt matches.

**Checkpoint**: SC-304's gap is closed on the device before phase 5.

## Phase 5 — User Stories 4 + 5: the guard, sign-in, connections (P2)

- [ ] **T037** `SigningLive` + `SigningSheet` — the guard's block and the
      editor (presets, custom amount, revoke) from `GuardEditorView`;
      batch legs from `GuardLegView`.
- [ ] **T038** `SigningLive` — the message block for `personal_sign` /
      typed data from `ClearMessageView` (rung: message; SIWE depth is 046).
- [ ] **T039** `ExploreLive` — the connections list (two origins, account,
      chain) and revoke from `DpermView` + the stored grants.
- [ ] **T040** [P] `app/src/test/…/GuardEditorTest.kt`, `SigningLiveTest.kt`
      — unlimited blocked, bounded passes, batch blocked by one leg;
      message block words.
- [ ] **T041** `[device]` SC-005 + SC-006: Approve unlimited blocked, bounded
      `1` leaves (calldata checked); Sign → the page's EIP-1271 check prints
      `valid`.

## Phase 6 — Closeout

- [ ] **T042** Gates as CI runs them: `cargo fmt/clippy/test`, Android unit
      tests, `check-expo-residue`, `check-native-reachability`,
      `verify-i18n-parity`, `gen-onboarding-types --check`; `build-web.mjs`
      rebuild + `--check` LAST.
- [ ] **T043** Bridge size against T001 and the ≤ 19.5 MB ceiling; the
      release APK inspected (no fixtures, no debug network config).
- [ ] **T044** `grep -rn 'ExploreFixtures.buildState\|SigningFixtures.build' …/navigation/`
      — only drawn fallbacks the live builders overlay.
- [ ] **T045** `results.md` — per SC: device (screenshot + text) or test;
      the `.so` delta; what 045–047 inherit (WalletPair out; SIWE depth,
      simulation, scanner → 046; the AddToken native tab → 047).
- [ ] **T046** SC-012: the founder answers a page with their own passkey —
      recorded as done only when it happened. Memory updated.

## Dependencies

Phase 0 → 1 → 2 → 3 → 4 → 5 → 6. Within a phase, `[P]` tasks touch
different files. T028 (the spine extraction) precedes T029.

## Implementation strategy

MVP = phases 0–4 (a page connects and a signed transfer lands). Phase 5
completes the guard, sign-in and revoke; phase 6 closes.
