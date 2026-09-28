# Tasks: 079 — The dApp browser and signing hold up on a bad network, on every client

**Input**: [spec.md](spec.md), [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md), [contracts/core-rules.md](contracts/core-rules.md), [quickstart.md](quickstart.md)

**Tests**: included — every rule moved into the core gets a core test (plan: "every moved rule keeps
or gains a test"), and each client's mapping gets a unit test. Device checks are the quickstart rows.

**Paths**: `A/` = `app-android/vela-wallet/app/src/main/java/app/getvela/wallet/`,
`AT/` = `app-android/vela-wallet/app/src/test/java/app/getvela/wallet/`,
`I/` = `app-ios/VelaWallet/VelaWallet/`, `IT/` = `app-ios/VelaWallet/VelaWalletTests/`,
`D/` = `app-desktop/vela-wallet/src/`, `W/` = `app-web/vela-wallet/src/lib/`,
`TS/` = `app-web/trusted-signer/`, `C/` = `rust/crates/vela-core/src/app/`,
`L/` = `rust/crates/vela-core/i18n/locales/<locale>/`.

## Format: `[ID] [P?] [Story] Description`

---

## Phase 1: Setup

- [x] T001 Create worktree `vela-wallet-079` on branch `079-android-dapp-browser-stability` off main a2c46438; generate Kotlin bindings, arm64 `.so`, iOS xcframework (`rust/scripts/build-ios-xcframework.sh`, `check-ios-core-fresh.sh` ok)
- [x] T002 [P] Add the fault proxy `scripts/device/chaos-proxy.py` (env `CHAOS_UPSTREAM`, `CHAOS_BIND`; cuts matching tunnels on a fault switch)
- [x] T003 [P] Add the iOS probe `app-ios/VelaWallet/VelaWalletUITests/DappBrowserStabilityProbeTests.swift` and skip it by name in `app-ios/VelaWallet/VelaWallet.xcodeproj/xcshareddata/xcschemes/VelaWallet.xcscheme`
- [ ] T004 Run the iOS probe on the iPhone once UI Automation is on (quickstart setup); store screenshots in `specs/079-android-dapp-browser-stability/evidence/ios-before/` and tick the iOS column of the spec's matrix against them — NOT RUN: UI Automation was off during the before-pass; the iOS column rests on the code audit, and the after-run is T077

---

## Phase 2: Foundational (core rules, words, bindings) — blocks every client phase

- [x] T005 Create `C/browser_load.rs` per contracts §1: `LoadPlatform`, `LoadFailureClass`, `LoadFailure`, `classify` (research R1 table incl. the not-a-failure cases), `retry_delay_ms` (R2), `LoadFinished`/`Visit`/`visit_to_record` (R3), `site_letter`; register it in `C/mod.rs`
- [x] T006 [P] Write `rust/crates/vela-core/tests/app_browser_load.rs`: every R1 row for Android and Apple codes, -999/102 → None, the retry schedule per class, visit rules (failed, 404, 500, `about:blank`, `chrome-error://`, `data:`, good page), letters (`app.uniswap.org`→U, `www.example.com`→E, `m.x.io`→X, `127.0.0.1`→1, empty→?)
- [x] T007 In `C/tx_tracker.rs`: age-based receipt cadence past the window (12 s → 60 s after 10 min → 300 s after 1 h; abandon at 24 h unchanged) and `TrackEntryView.outcome` (`Landing | StillConfirming | Unknown | Final`); keep "time never makes a failure"
- [x] T008 [P] Tests for T007 in `rust/crates/vela-core/tests/` (the tracker's existing test file): cadence at 5 min / 30 min / 3 h, `outcome` per state, no terminal status from age alone
- [x] T009 In `C/fee_policy.rs`: `requote_delay_ms(&FeeFailure, attempt)` (3 s, 6 s, 12 s, then 15 s; `None` for unfixable failures) with unit tests beside the existing fee tests
- [x] T010 Export T005/T009 through UniFFI in `rust/crates/vela-core-uniffi/src/lib.rs` (`browser_load_classify`, `browser_load_retry_delay_ms`, `browser_load_visit`, `browser_site_letter`, `fee_requote_delay_ms`) and through wasm in `rust/crates/vela-core-wasm/src/lib.rs` for what the extension uses (`fee_requote_delay_ms`)
- [x] T011 Add the corpus keys of contracts §4 to all 15 locales in `L/explore.json` and `L/componentsUi.json` (zh source text from the contract; real translations for the other 14; zh-HK in Cantonese written form)
- [x] T012 Run the six i18n steps (memory: i18n corpus gates) — `npm run gen:i18n`, `lint:i18n`, `verify:i18n`, `dump:vectors`, the leaf-count pin, `build:wasm` — and `cd rust && cargo test -p vela-core --features i18n-all,crux`; regenerate TS types (`gen:core-types`) and `build-web --check`
- [x] T013 Regenerate Kotlin bindings, the arm64 `.so` and the iOS xcframework from this tree (T001's commands; `check-ios-core-fresh.sh`)
- [x] T014 [P] Mirror the new view field and wire types in each client's wire file: Android `A/feature/send/core/` tracker wire (`TrackEntryView.outcome`), iOS `I/Features/Send/TrackerWire.swift`, desktop (crate types, no mirror), web `W/wallet/core/` tracker types; extend Android `AT/CoreWireDriftTest.kt` for the new field

**Checkpoint**: core suite green; bindings current on every client.

---

## Phase 3: User Story 1 — a signing request is never lost by accident; the person sees it land (P1) 🎯 MVP

**Goal**: explicit-only close; a status body after approval; honest wording for an unlanded op.
**Independent test**: quickstart S1–S6.

### Android
- [x] T015 [US1] Add a non-dismissible mode to `A/core/designsystem/components/VelaModalSheet.kt` (sheet state refuses the hidden target, scrim taps swallowed, `BackHandler` swallows) and use it for the signing sheet (`A/feature/signing/SigningSheet.kt`) and the consent sheet (`A/feature/explore/ExploreScreen.kt`)
- [x] T016 [US1] Put a ✕ in the signing header (`A/feature/signing/components/SigningComponents.kt` `SigningHeader`) wired to the existing dismiss path in `A/navigation/VelaNavHost.kt` (the `SigningSheet(` call at ~682); update the "Dismissal is rejection" doc comment in `SigningSheet.kt` to ruling 1
- [x] T017 [US1] Replace the form with a status body once approved: in `A/feature/signing/SigningLive.kt` map `SignView` (`is_signing`, `is_submitting`, `pending_op_hash`, `error`) + the tracker entry's `outcome` to the data-model's signing status; render it in `SigningSheet.kt` with `StatusHero` from `A/feature/flows/components/FlowBlocks.kt:593`; hide fee, speed and slide after approval
- [x] T018 [US1] Keep the landing result after the core clears the sheet (the desktop `dapp_landing` pattern) in `A/VelaWalletApplication.kt` `openSigning`/`signing` flow: show the tick with short hash + explorer link, close after ~1.2 s; message signatures show "已签名" and close
- [x] T019 [US1] Still-confirming and unknown: render `componentsUi.signing.stillConfirming` / `unknownOutcome` from `outcome`; closing in any post-approval state sends no answer
- [x] T020 [P] [US1] Unit tests in `AT/SigningLiveTest.kt` (or the existing signing test file): status mapping for every `SignView`/`outcome` combination; the ✕ before approval answers 4001 once, after approval answers nothing

### iOS
- [x] T021 [P] [US1] `.interactiveDismissDisabled()` on the signing and consent sheets in `I/Features/Explore/ExploreScreen.swift` (~300/315) and a ✕ in `I/Components/Signing/SigningAtoms.swift` header wired to `onSigningDismissed()`
- [x] T022 [US1] Status body in `I/Features/Signing/SigningLive.swift` + `SigningSheet.swift` using `StatusHeroView` (as `I/Features/Flows/FlowBodies.swift:1306`); keep the landing result for the tick; still-confirming/unknown words
- [x] T023 [P] [US1] Hermetic tests in `IT/` for the status mapping and the one-answer rule
- [x] T023a [US1] iOS tracker clock: tick while any entry is `polling`, not only `status == "pending"` (`I/Features/Send/TrackerWire.swift` `hasPending`, the ticker in `I/Features/Send/TrackerStore.swift`) — the Android device pass showed an op past its window never updating
- [x] T023b [US1] iOS receipt wait: cap each poll by what is left of the window (`I/Features/Signing/Core/SignExecutor.swift` receipt loop) — Android's page waited 268 s with the relay down

### Desktop
- [X] T024 [P] [US1] In `D/signing/live.rs` + `D/wallet/page.rs` (`signing_body` ~14002): switch to the status view as soon as `is_signing`/`is_submitting`/`pending_op_hash` (not only after the core closes the sheet, `page.rs:15156`); hide the dimmed slide (`page.rs:14592`); map every tracker status, not just Submitted (`page.rs:15199`), incl. still-confirming/unknown — done on 079-desktop
- [X] T025 [P] [US1] Desktop tests beside `D/signing/live.rs` for the mapping — done on 079-desktop
- [X] T025a [US1] Desktop tracker clock ticks while any entry is `polling`; receipt wait capped by the remaining window (`D/executor/tracker.rs`, `D/executor/sign_request.rs` `await_receipt`) — done on 079-desktop

### Extension
- [x] T026 [P] [US1] `W/signing/SigningHost.svelte:375`: `dismissible={false}` for the signing sheet and the consent card (`W/dapp/DappRequestHost.svelte`); explicit ✕ keeps `reject_tapped`; `W/wallet/ui/BottomSheet.svelte` gains a non-dismissible mode (no scrim click, no Esc, no drag)
- [x] T027 [US1] Message signatures get the signed tick before closing; `W/wallet/core/tracker-resident.ts` keeps ticking past 120 s while any entry is `polling` (the core paces it) and the receipt shows still-confirming/unknown (`SigningHost.svelte:129-135`)
- [x] T028 [P] [US1] Web unit tests for T026–T027 (BottomSheet non-dismissible, receipt status mapping)

**Checkpoint**: quickstart S1–S6 pass on the Xiaomi and the iPhone; desktop screenshot of the status view.

---

## Phase 4: User Story 2 — the fee explains itself and can be refreshed (P1)

**Goal**: refresh control; plain reason for an unreachable service; automatic re-quote.
**Independent test**: quickstart S7–S8.

- [x] T029 [US2] Android: reuse `FeeRefreshButton` (`A/feature/flows/components/FlowBlocks.kt:1207`) in `A/feature/signing/SigningFee.kt`; `A/feature/signing/SigningLive.kt` sets its label (`send.feeRefresh`) and `refreshing`; `QuoteUnavailable` shows `componentsUi.funding.denialNetworkError`
- [x] T030 [US2] Android: schedule re-quotes with `feeRequoteDelayMs` in `A/feature/signing/core/SigningController.kt` while the sheet is open and unapproved; cancel on approve/close
- [x] T031 [P] [US2] iOS: refresh control in `I/Components/Signing/SigningFooter.swift` (`SigningFeeView`) calling the uncalled `refreshFee()` (`I/Features/Signing/Core/SigningController.swift:400`); reason line; re-quote timer
- [X] T032 [P] [US2] Desktop: refresh icon in `D/signing/components.rs:809` row; reason instead of "—" (`D/flows/live.rs:1089-1090`); honour `stale` on this sheet; re-quote timer in `D/wallet/signing_host.rs` — done on 079-desktop
- [x] T033 [P] [US2] Extension: refresh control + stale note in the signing fee row (`W/signing/live.ts:468-478`, reuse `W/flows/ui/FeeRow.svelte`'s control); re-quote with `fee_requote_delay_ms` over wasm
- [X] T034 [P] [US2] (Android, iOS and web done) Tests per client for the reason mapping and the timer stopping on approve (Android `AT/`, iOS `IT/`, desktop in-crate, web unit) — done: Android `SigningReceiptTest` (mayRequote) + `SigningLiveTest` (reason), iOS `SigningFeeRetryTests`, desktop `the_requote_stops_where_it_must`, web unit

**Checkpoint**: S7 fee appears ≤ 15 s after the relay returns, with no tap, on the Xiaomi and the iPhone.

---

## Phase 5: User Story 3 — a page on a bad network never looks frozen or broken (P1)

**Goal**: progress from the request; the Vela panel with a reason, retrying in place, auto-retry;
Recents only for loaded pages, from one document.
**Independent test**: quickstart L1–L6.

### Android
- [x] T035 [US3] In `A/feature/browser/core/BrowserController.kt` `BrowserEngine`: set `loading=true, progress≥10` in `load()`, `reload()` and for main-frame http(s) navigations in `shouldOverrideUrlLoading`; add `retrying` and `failure: LoadFailure?` to `EngineState`; classify in `onReceivedError`/`onReceivedSslError` with `browserLoadClassify(Android, …)`
- [x] T036 [US3] Keep the panel through a retry: `reload()` on a failed tab sets `retrying=true` and leaves `failure` until `onPageCommitVisible`/`onPageFinished` without error; auto-retry timer from `browserLoadRetryDelayMs` only while the engine is attached (`attach`/`detach`)
- [x] T037 [US3] Visit rule: replace `FAVICON_JS` + `view.title` with one script returning `{href,title,icon}` and pass it through `browserLoadVisit` before `BhistEvent.VisitRecorded` in `BrowserController.kt` `loadFinished`
- [x] T038 [US3] Panel UI in `A/feature/explore/ExploreScreen.kt` (`BrowserNotice` for `failed`): reason line from `failure.reason_key`, host, Retry, "正在重试…" while `retrying`; the panel covers the WebView fully (no engine page visible)
- [x] T039 [P] [US3] Tests: `AT/BrowserMachineTest.kt` (or new `AT/BrowserLoadTest.kt`) — loading on request, panel persists through retry, no visit on failure, visit fields from one document

### iOS
- [x] T040 [P] [US3] `I/Features/Explore/Core/BrowserEngine.swift`: loading in `load()`/`reload()`; classify `didFail`/`didFailProvisional` via `browserLoadClassify(Apple, …)` (ignore -999); `retrying` + auto-retry while in front; replace `localizedDescription` (`:377`) with the reason key; one-script `{href,title,icon}` → `browserLoadVisit` (fix `?? title` at `:213` and the async icon at `:233-243`)
- [x] T041 [US3] Panel in `I/Components/Explore/BrowserWebView.swift` and `I/Features/Explore/ExploreScreen.swift:459-473`: reason + retrying; no blank/previous page during retry; no demo page in the new-tab gap
- [x] T042 [P] [US3] Hermetic tests in `IT/` for the engine state transitions (fake navigation delegate events)

### Desktop
- [X] T043 [P] [US3] `D/webview.rs` + `D/wallet/browser_host.rs`: `loading` from the request; watchdog (research R4) — no commit in 3 s → native HEAD probe → `classify(Probe, …)`; success keeps waiting to 20 s → timeout; commit clears; visit URL from the page script's own message (`webview.rs:459` reads the view URL today) — done on 079-desktop
- [X] T044 [US3] Desktop panel + progress in `D/wallet/page.rs` (`explore_content` ~12289) and `D/explore/components.rs`: a progress hairline and the failure panel (reason, host, Retry, retrying) — done on 079-desktop
- [X] T045 [P] [US3] Desktop tests for the watchdog decisions and the visit URL — done on 079-desktop

**Checkpoint**: L1–L6 on the Xiaomi (fault proxy), the iPhone (Wi-Fi proxy) and the Mac.

---

## Phase 6: User Story 7 — one slide, a signing page a beginner can read, offline (P1)

**Goal**: app hands off with a button; the page shows the origin honestly, says failures plainly,
resets its slide, and opens without the network after one visit.
**Independent test**: quickstart T1–T5.

### Core
- [x] T046 [US7] `rust/crates/vela-core/src/trusted_signer.rs:312-361`: fill `context.dapp = { name: host, origin, source: "vela_browser" }` for requests forwarded by an in-app browser (the dApp origin the sign request carries); tests beside the existing trusted-signer tests
- [x] T047 [US7] `rust/crates/vela-core/src/trusted_signer.rs:541-564`: build the page URL on the content-addressed path `/b/<hash>/sign` with the newest hash of `trusted_signer/integrity.rs` `BUILD_ALLOWED`; tests for the URL shape

### Signer page (`TS/`)
- [X] T048 [US7] **Owner decision needed.** The page cannot tell who opened it: anyone can open sign.getvela.app with a forged fragment claiming `source: "vela_browser"` and a `velawallet:` callback, so dropping `warn.claimedOrigin` for those requests would let a phishing page look vouched for. With T046 the header already names the host (`context.dapp.name`) instead of "未知站点"; what remains is the warning's words and tone (`TS/src/lib/resolve.js:1356-1357`, `TS/src/lib/locales/*.js` `warn.claimedOrigin`) — done (owner delegated): the warning stays, in plain words; released as ec038e11… and LAUNCH moved (d76deae9, 42acc794)
- [X] T049 [US7] Plain intent: the fee leg is shown as the fee row, and the intent says what the person does (`resolve.js:1284-1286,1313-1315`) instead of "批量" — done: a fee leg the fee row shows in full (`inBandLeg` decoded a plain payment) leaves the leg list, so a send reads as the send; any other "fee" stays a leg. `samples/fee-leg-test.mjs` 8/8, hostile 32/32
- [X] T050 [US7] (sticky slider done in 005f8e1d; folding the explanation and fee note still open) Layout (`TS/src/lib/render.js:473-541`, `TS/src/sheet.css:460-469`): explanation and 技术细节 folded; fee note folded; slider sticky at the bottom — done: the fee's explanation folds under its row (`<details>`, the row is the summary); the amount and the 自述 tag stay on the row; 技术细节 was already folded. New build e3ef90a6… replaces the undeployed 32eb2f2b… at the front of BUILD_ALLOWED
- [x] T051 [US7] Failures (`TS/src/sign.js:67,229-237,468-470`): `NotAllowedError` and `AbortError` → one everyday sentence; other errors → the same sentence with the raw text inside 技术细节; the knob returns to the start; a reload with no request → "这个签名请求已结束，请回到 Vela 重新发起"
- [x] T052 [US7] Words in every page locale `TS/src/lib/locales/*.js` (new tag, failure sentence, ended sentence; `ui.ceremonyFailed` rewritten without "仪式")
- [x] T053 [US7] `TS/dist/_headers` (emitted by `TS/samples/build-single.mjs`): `/b/*` → `Cache-Control: public, max-age=31536000, immutable`; root unchanged; rebuild (`bun samples/build-single.mjs`), add the new hash to `BUILD_ALLOWED` in `rust/crates/vela-core/src/trusted_signer/integrity.rs`, keep `--check` green
- [x] T054 [P] [US7] Signer page tests (the package's test runner) for T048–T051

### Clients (native half)
- [x] T055 [US7] Android: when the route is `KeyMethod.TrustedSigner` (`A/feature/send/core/UserOpSpine.kt:95-104`), `A/feature/signing/SigningSheet.kt:210` draws a primary button `componentsUi.signing.continueToSigner` instead of `SlideToConfirm`, sending the same approve (`A/navigation/VelaNavHost.kt:689`)
- [X] T056 [US7] Android: if the Custom Tab cannot open the page (no answer and the tab reports a navigation failure, or the person returns without a result), the waiting card shows `componentsUi.signing.signerUnreachable` + Retry (`A/feature/signing/trustedsigner/TrustedSignerTab.kt`, `TrustedSignerChannel.kt`), keeping the request open — done: on return with no answer the page's address is probed (HEAD, no fragment); unreachable → `componentsUi.signing.signerDown` + `connect.browser.retry`; the "签名中…" line is not drawn under the card. Xiaomi: T5 rows pass (evidence/android-after/t5-*.jpg)
- [X] T057 [P] [US7] iOS: the same button in `I/Features/Signing/SigningSheet.swift:67` for the trusted-signer route (`I/Core/UserOpSpine.swift:475`); unreachable handling around `I/Features/Signing/TrustedSigner/TrustedSigner.swift:357` — done on `079-ios`: primary `componentsUi.signing.openSigner` button, `originSeenByBrowser`; the tab's own signals (`safariViewControllerDidFinish` + 1.2 s, `didCompleteInitialLoad(false)`) start a HEAD to the page's address (no fragment/query); unreachable → `componentsUi.signing.signerDown` + `connect.browser.retry`, no "签名中…" under the card
- [X] T058 [P] [US7] Desktop: the same button in `D/wallet/page.rs:14593` for the trusted-signer route (`D/executor/send.rs:113,149`) — done on 079-desktop
- [X] T059 [US7] Owner step (outward-facing): deploy `TS/dist/` incl. `_headers` to sign.getvela.app; then verify headers with `curl -I https://sign.getvela.app/b/<hash>/sign`; the build to release is `e3ef90a6040fe896a34c6b32fcab232417bf1dd70e62d473c88a0bed7dc97d5f` (then `LAUNCH` = that hash) — done: owner deployed; checked live (200, `immutable`, bytes hash to e3ef90a6…, listed in index.json); `LAUNCH` = e3ef90a6… in 6f8a2adb

**Checkpoint**: T1–T3 on the Xiaomi with the owner's fingerprint; T4–T5 after T059.

---

## Phase 7: User Story 4 — when the site's chain cannot be reached, the browser says so (P2)

- [X] T060 [US4] Android: expose `RpcPool.view` to the browser (`A/VelaWalletApplication.kt` `browser` lazy) and show a one-line notice under the address bar in `A/feature/explore/ExploreScreen.kt` when the tab's chain (`DbrTabView`) ∈ `failed_chains` ∖ `rate_limited_chains`, with Retry (one `eth_blockNumber` through the pool) — done: `BrowserController.poolView` + `askChain`; `ExploreLive.chainUnreachable`; C1/C2 pass on the Xiaomi with `drop match=gnosischain` (evidence/android-after/t060-chain-down.jpg)
- [x] T061 [P] [US4] iOS: the same from `pool.failedChains` (today only in `I/App/RootView.swift:2415`) into `I/Features/Explore/ExploreScreen.swift`
- [X] T062 [P] [US4] Desktop: expose `failed_chains` from `D/executor/pool.rs:358` and draw the notice in `D/wallet/page.rs` — done on 079-desktop
- [X] T063 [P] [US4] Tests per client: shown for failed, hidden for rate-limited, cleared on the next answer — done: Android `ExploreLiveTest`, iOS `theChainNoticeShowsForAFailedChainOnly`, desktop `the_chain_notice_is_for_a_chain_that_is_down_not_throttled`; "cleared on the next answer" is the pool's own rule (core rpc_pool tests)

---

## Phase 8: User Story 5 — honest, complete chrome (P2)

- [x] T064 [US5] Android icons: add `LockOpen` (lucide lock-open) to `A/core/designsystem/components/VelaIcons.kt`; `A/feature/explore/components/BrowserChrome.kt` `AddressBar`: closed lock `fgMuted` for secure, open lock `warningBase` for http, no text; content descriptions `explore.httpsA11y` / `connect.browser.a11yInsecure`
- [x] T065 [US5] Android: remove the visible status text and green from `ConnectionPanel` and `SiteMenuSheetContent` (`A/feature/explore/components/ExploreSheets.kt`) and from `A/feature/browser/ExploreLive.kt` (`statusLine`), icon only
- [x] T066 [US5] Android consent: draw `connection.title` in `ConnectionPanel` and make Connect the primary filled button (`ExploreSheets.kt`); unify the approve word with iOS/desktop/extension on `connect.browser.connect` (iOS uses "批准" today — T070)
- [x] T067 [US5] Android pickers: `PickerOption` gains `logoUrl`/`identiconSeed`/`amount` (`ExploreSheets.kt:442`); `A/navigation/VelaNavHost.kt:1181,1189` fill network logo (the `core/marks` source) + per-network balance from the balance dashboard's cached figures (display currency; blank when unknown) and account identicons
- [x] T068 [US5] Android: signing header shows the host once when name == host (`SigningComponents.kt` `SigningHeader`); site avatars use the recorded favicon via `RemoteLogo` with `browserSiteLetter` fallback (`A/feature/explore/components/ExploreComponents.kt` `LetterAvatar`, `A/feature/browser/ExploreLive.kt:letterOf` removed)
- [x] T069 [P] [US5] iOS: `lock.open` in `I/Components/Explore/AddressBarView.swift:107-113`; icon-only in `ConnectionPanelView.swift:44-47`, `SiteMenuSheetView.swift:39-42`; consent title drawn (`ConnectionPanelView.swift:33`); network rows logo + balance (`ConnectionPanelView.swift:127-131`); header host once (`SigningLive.swift:205-206`); favicon avatars + `browserSiteLetter` (`ExploreLive.swift:381-382`, `LetterAvatarView.swift`)
- [x] T070 [US5] iOS consent approve word → `connect.browser.connect` (today "批准")
- [X] T071 [P] [US5] Desktop: open-lock icon for http (`D/explore/components.rs:497-498`); icon-only connection panel (`D/wallet/page.rs:13699-13703,13791`); network rows logo + balance and the per-chain dot (fix `chain_ethereum()` at `page.rs:13732`); wire "switch account" (`page.rs:13836`); header host once (`D/signing/live.rs:1328-1329`); favicon avatars + `site_letter` (`D/explore/live.rs:152-154`) — done on 079-desktop
- [x] T072 [P] [US5] Extension: header host once (`W/signing/live.ts:705,722-723`)
- [X] T073 [P] [US5] Tests per client: lock state per scheme, no visible safety text (UI string sweep), picker rows carry logo/balance/identicon, header dedupe — done: Android `ExploreLiveTest` (lock, no words, picker rows) + `SigningReceiptTest` (host once), iOS `BrowserChromeTests`, desktop `the_lock_says_the_scheme_and_nothing_else` / `each_network_shows_what_the_account_holds_there`, web header dedupe

---

## Phase 9: User Story 6 — tabs can be told apart (P3)

- [X] T074 [US6] Android: snapshot the WebView (scaled bitmap) on `detach()` in `A/feature/browser/core/BrowserController.kt`; keep per tab in memory; draw it in `A/feature/explore/components/ExploreTabs.kt` tab cards (start pages keep the drawing) — done: snapshot on `detach()` (360 px wide), `BrowserController.snapshots`; U6 passes (evidence/android-after/t074-two-tabs.jpg)
- [x] T075 [P] [US6] iOS: `WKWebView.takeSnapshot` on leaving a tab; draw in `I/Components/Explore/TabCardView.swift:32-37`

---

## Phase 10: Polish, device passes, delivery

- [X] T076 Device pass Android (quickstart A–F) on the Xiaomi with the fault proxy; owner fingerprint for T1–T3 and one S4; restore `http_proxy :0`; evidence in `specs/079-android-dapp-browser-stability/evidence/android-after/` — done: quickstart A (L1–L6), B (C1, C2), C (S1–S4, S6–S8), D (U1–U7), E (T1–T5, with the owner's fingerprint; T4 3 of 3 offline), F (070 A1–A18); proxy restored to `:0`
- [X] T077 Device pass iOS (quickstart A–F) via the probe + Wi-Fi proxy; evidence in `evidence/ios-after/` — done: iPhone 11 probe (checkpoint walk + dead address), evidence/ios-after/; found and fixed the proxy white page (383af421) and the cold-open balances (995f4725). The Wi-Fi fault-proxy rows were not run: the phone's own proxy (Shadowrocket) already made the unstable case
- [X] T078 Desktop pass (quickstart A, C, D) with screenshots at desktop and phone width (memory: look before done) — done by running the app at 1280×800 (signing states, load watchdog + panel, network picker; `desk079/` screenshots); phone width is impossible (the window's minimum is 1280×800)
- [X] T079 Extension pass (quickstart C, D rows that apply) in the packaged extension — done: packaged-extension e2e (Escape/scrim keep the request, ✕ → 4001) and web parallel-space e2e (status, never a dimmed slide; landing closes itself); the side-panel tick is not e2e-tested (helper broken on main too)
- [X] T080 Gates: core (`cargo test --workspace`, clippy `-D warnings`, fmt on own files), Android JVM (`JAVA_HOME` = Android Studio JBR, `-Porg.gradle.java.installations.auto-detect=false`), iOS hermetic on a simulator clone, desktop `cargo test`, web unit + extension e2e, i18n parity, `build-web --check`, the parity rulers in `scripts/check-*.mjs` — done except the iOS device run (see results.md Gates)
- [X] T081 `specs/079-android-dapp-browser-stability/results.md`: per SC verdict with evidence, per-client matrix after the fix, what was not done and why; D2–D7 hand-off to the owner (relay Arbitrum not mining, dApp tx absent from Activity, fee overcharge, simulation warning, address case, history empty copy) — done: results.md
- [X] T082 Memory: project note for 079 (rulings, core rules, device recipe incl. chaos proxy and the iOS UI Automation switch), update the Android/iOS device references — done: memory project_079_dapp_browser_stability
- [X] T083 Commit by path per client in reviewable steps; push; PR to main with the evidence table — done: PR #327

---

## Dependencies & execution order

- Phase 1 → Phase 2 (T005–T014) → every story phase.
- US1, US2, US3, US7, US4, US5, US6 are independent of each other after Phase 2, except: T018 needs
  T017; T036 needs T035; T048/T053 need T046/T047; T059 (owner deploy) gates quickstart T4–T5 only.
- Within a story, client blocks (Android / iOS / desktop / extension / signer page) are independent.

## Parallel examples

- Phase 2: T006, T008 alongside T005/T007; T011 (words) alongside the Rust work.
- US1: T021 (iOS), T024 (desktop), T026 (extension) in parallel with the Android block T015–T020.
- US5: T069, T071, T072 in parallel with T064–T068.

## Implementation strategy

1. **MVP = Phase 2 + US1 on Android**, verified on the Xiaomi (the owner's two sharpest complaints:
   the sheet that closes itself and the silent sheet after the fingerprint).
2. Then US2 + US3 on Android (fee + page loads), then US7 (one slide) — the rest of the owner's
   phone experience.
3. Port each finished story to iOS, desktop and extension in that order, verifying on the iPhone and
   the Mac before moving on.
4. US4, US5, US6 last; the signer page deploy (T059) whenever the owner releases it.
