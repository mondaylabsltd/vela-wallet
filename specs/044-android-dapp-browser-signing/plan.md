# Implementation Plan: Android dApp Browser and Signing

**Branch**: `044-android-dapp-browser-signing` | **Date**: 2026-09-12 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/044-android-dapp-browser-signing/spec.md`

## Summary

Turn the drawn 探索 tab into a browser that a dApp can talk to, and answer
what it asks: six core machines cross the Android bridge
(`dapp_permissions`, `explore_sites`, `browser_history`, `sign_request`,
`clear_signing`, `approval_guard`); a system `WebView` carries the SAME
provider script the extension and the desktop ship, bridged to Kotlin the
way the desktop bridges it; a routing table ported from `protocol.js` (and
tested against it) says who answers each forwarded request; the signing
sheet reads the three signing machines plus 043's fee policy and submits
through 043's spine, factored so the send flow and a dApp's transaction are
ONE implementation. Every phase is verified on the Xiaomi in the parallel
space, with the web's local test dApp served over the cable — 027's SC-304
("connects but cannot sign") is the first success criterion that counts.

## Technical Context

**Language/Version**: Kotlin 2.x / Compose (Android, minSdk 29), Rust 2024
(vela-core, uniffi 0.29 bridge), JavaScript (the shared provider script,
untouched).
**Primary Dependencies**: `androidx.webkit` (new: `WebViewCompat.
addDocumentStartJavaScript`), the existing `CoreHost`/`JsonShell`/`Wire`
driver, `RpcPool`, `RelayClient`, `FeeExecutor`, `TrackerExecutor`,
`FeedExecutor`, `PasskeyExecutor`/`ParallelSpaceHook` (043).
**Storage**: the one DataStore file (`VelaStore`): `vela.perm.<origin>`
(one grant per origin, the desktop's key), `vela.explore`,
`vela.browserHistory`, connection and dApp-transaction rows in
`vela.transactions` (the feed's shape, `type: "connect"` / `"dapp_tx"`).
**Testing**: JUnit (JVM, uniffi host library; `testDebug` for the parallel
space), `CoreWireDriftTest` against the ts-rs mirrors, a JVM parity test
that parses `extension/lib/protocol.js`, the device loop (`adb` +
`uiautomator` + `screencap`, scripts in the results).
**Target Platform**: Android 10+ (system WebView ≥ 90 for document-start
scripts; older WebViews take the `onPageStarted` fallback, recorded).
**Project Type**: mobile shell over a shared Rust core.
**Performance Goals**: a connected page's instant answers (`eth_accounts`,
`eth_chainId`) in one frame; a read proxied through the pool within the
pool's own budget; the signing sheet open within 1 s of a request.
**Constraints**: no Kotlin judgement (FR-014); the provider script is not
forked (FR-002); the bridge size budget: 043 landed at 16,187,032 bytes
arm64; clear_signing alone is 5,038 lines — ceiling **≤ 19.5 MB arm64**,
measured in closeout.
**Scale/Scope**: six machines (Event 11/16/5/13/5/11, Operation 8/2/3/7/5/3,
ShellResult 2/2/2/7/5/3), ~35 new Kotlin files, two hosts, two live
builders, one WebView surface, seven drawn explore states and thirty-three
drawn signing states gaining live models.

## Constitution Check

`.specify/memory/constitution.md` is the unfilled template; the program's
standing rules apply instead: (1) rules live in vela-core, shells perform
and draw (FR-014); (2) one implementation — the provider script, the
routing table, the submit spine are shared or checked against their twin,
never re-typed; (3) every wire family passes the drift gate; (4) every
phase is verified on the device (founder's rule); (5) no fixture reaches a
live route; (6) words come from the corpus. All six hold in this plan; the
one deliberate deviation is recorded in Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/044-android-dapp-browser-signing/
├── plan.md              # this file
├── research.md          # D1–D15
├── data-model.md        # grants, site memory, requests, records
├── quickstart.md        # the device scenarios
├── contracts/
│   ├── shell-operations.md   # the six machines' operations, who answers
│   └── page-envelope.md      # the page ↔ shell channel
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
rust/crates/vela-core-uniffi/src/
├── onboarding_bridge.rs        # + six bridge_object!s
└── lib.rs                      # + dapp_origin_of (the core's origin rule)

app-android/vela-wallet/app/src/main/java/app/getvela/wallet/
├── feature/browser/            # NEW — the in-app browser
│   ├── core/DpermWire.kt  ExploreWire.kt  BhistWire.kt
│   ├── core/DappRpc.kt                 # protocol.js's table, ported
│   ├── core/BrowserExecutor.kt         # dperm's 8 arms
│   ├── core/ExploreExecutor.kt  BhistExecutor.kt
│   ├── core/BrowserController.kt       # hosts dperm + explore + bhist; the request sink
│   ├── core/ProviderBridge.kt          # WebView: inject, JavascriptInterface, deliver
│   └── ExploreLive.kt                  # ExploreView+BhistView → ExploreScreenModel
├── feature/signing/
│   ├── core/SignWire.kt  ClearWire.kt  GuardWire.kt
│   ├── core/SignExecutor.kt  ClearExecutor.kt  GuardExecutor.kt
│   ├── core/SigningController.kt       # born per request: sign+clear+guard+fee
│   └── SigningLive.kt                  # the four views → SigningScreenModel
├── feature/send/core/UserOpSpine.kt    # EXTRACTED from SendExecutor.submitInner (one implementation)
├── feature/explore/…                   # drawn; gains callbacks + a WebView slot
├── navigation/VelaNavHost.kt           # 探索 section live; the signing sheet raised by a request
└── VelaWalletApplication.kt            # container: browser + signing

app-android/vela-wallet/app/src/main/assets/provider/   # synced from app-web/vela-wallet/extension (Gradle task)
app-android/vela-wallet/app/src/debug/res/xml/network_security_config.xml   # cleartext to 127.0.0.1 only, debug only
app-android/vela-wallet/dev/testdapp/index.html         # Connect / Sign / Send dust / Approve unlimited
app-android/vela-wallet/app/src/test/…                  # wires, executors, routing parity, live builders
app-android/vela-wallet/app/src/testDebug/…             # the page-asks-and-money-moves test (fixture keyset)
```

**Structure Decision**: a new `feature/browser` package for what the tab
does (the machines, the bridge, the routing) beside the drawn
`feature/explore` (which keeps its models and screens); `feature/signing`
grows its `core/` beside its drawn sheet. The submit spine leaves
`SendExecutor` for a class both callers construct, because the desktop's
lesson (one pipeline for a person's transfer and a dApp's transaction)
becomes a structural fact rather than a promise.

## Complexity Tracking

| Deviation | Why it is needed | Simpler alternative rejected because |
| --- | --- | --- |
| A Gradle task copies two files from `app-web/vela-wallet/extension/` into the APK's assets | The provider must be the SAME script (FR-002); Android cannot `include_str!` a sibling tree | Vendoring a copy is a fork that drifts on the first edit; a git submodule is the same tree already |
| The `androidx.webkit` fallback (`onPageStarted` + `evaluateJavascript`) exists for WebViews without document-start scripts | A page's own scripts may run before the provider lands there, so discovery can miss on old engines | Refusing to run on old WebViews would exclude devices for a feature that mostly still works; the fallback is recorded and the device pass names which path ran |
