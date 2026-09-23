# Tasks: 070 — One dApp browser engine, in the core

Legend: `[P]` parallelisable, `[USn]` user story. Paths are repo-relative.

## Phase 0 — pure pieces (US1)

- [x] T001 Move `app-web/vela-wallet/extension/inpage.js` → `rust/crates/vela-core/provider/inpage.js` as a classic script (declare the six protocol constants inside; behavior unchanged)
- [x] T002 `rust/crates/vela-core/src/app/dapp_rpc.rs`: `Route`, `classify`, allowlists, `SIGNING_METHODS`, `chain_param`, `validate`, `browser_input`, JSON builders, `SignErrorKind` default messages
- [x] T003 `provider_script(host)` + the one bridge (hello/doc/deliver guard), top-frame only
- [x] T004 [P] unit tests for T002/T003 (allowlists byte-equal to `extension/lib/protocol.js`, parsed at test time)

## Phase 1 — the machine (US1, US2, US4)

- [x] T010 `rust/crates/vela-core/src/app/dapp_browser.rs`: model (tabs, documents, sites, consent, sign queue, read queue, known op hashes), events/ops/view per the contract
- [x] T011 Conformance tests `rust/crates/vela-core/tests/app_dapp_browser.rs`: every route; hello/navigation/load_finished/tab_closed/renderer_gone settling; per-tab delivery; per-origin chain persistence; add_chain; follow-the-account; revoke/revoke_all; sign queue; read bound; receipt translation; malformed input; iframe refusal
- [x] T012 `sign_request::is_signing_method` delegates to `dapp_rpc`; `dapp_permissions` decisions reused unchanged

## Phase 2 — bindings

- [x] T020 UniFFI: `DappBrowserCore` bridge object; free fns `dapp_provider_script`, `dapp_browser_input`
- [x] T021 wasm: `DappBrowserCore` (tests) + `dappRpcClassify`; regenerate `pkg-web` + `assets/wasm`; `gen:core-types`

## Phase 3 — Android (US1–US5)

- [x] T030 Wire types `DbrWire.kt`; executor (store, deliver, reads via `RpcPool`, relay receipts, feed row)
- [x] T031 `BrowserEngine`: `addWebMessageListener`, core script, `onRenderProcessGone`, `onPageFinished`/`onReceivedError` → `load_finished`, progress, JS dialogs (MutableContextWrapper), external schemes with gesture, pause hidden tabs
- [x] T032 Delete `DappRpc.kt`, `RequestRouter.kt`, bridge string/assembly in `ProviderBridge.kt`, request bookkeeping in `BrowserController.kt`
- [x] T033 Signing: `forward_to_signing` → `openSigningRequest` with granted address + chain; `SendResponse` → `signing_answered`; `cancel_signing` → `TransportDropped`
- [x] T034 UX: search, editable address bar, Back = page back, star toggle, site-menu actions, network row, truthful lock, error page, loading bar, crashed state
- [x] T035 Settings: connected sites list + Disconnect; storage clear → `revoke_all`
- [x] T036 Explore scan routing (URL / address / `wc:`)
- [x] T037 JVM tests (executor + controller against the real core) — 628 pass; device pass quickstart A on the Android phone (A1–A19, a real dust tx on Gnosis)

## Phase 4 — iOS (US1–US5)

- [x] T040 `DbrWire.swift`, executor; `frameInfo.securityOrigin`; `webViewWebContentProcessDidTerminate`; `didFinish/didFail` → `load_finished`
- [x] T041 Engine teardown (remove handler, weak proxy), no settle on leaving Explore, background tab filter removed (core owns it)
- [x] T042 Delete `DappRpc.swift`, `RequestRouter.swift` decisions, bridge string/assembly
- [x] T043 Signing pinning + `signing_answered` + `cancel_signing`; wire `receiptFor` via core
- [x] T044 UX list (as T034), connected sites, scan routing
- [x] T045 Hermetic tests (VelaWalletTests 669 pass); device pass quickstart B — `BrowserAcceptanceTests` 9/9 on the iOS 26.2 simulator (UI automation cannot be enabled remotely on the iPhone 11); on the iPhone 11 (iOS 26.5.2) the non-tap rows over Web Inspector. Found and fixed there: a launch into Explore left the endpoint pool unbooted (page reads -32603)

## Phase 5 — desktop

- [x] T050 `webview.rs` uses `dapp_provider_script(Desktop)`; `browser_host.rs` on `DappBrowser`; delete `executor/dapp_rpc.rs` table
- [x] T051 Signing queue replaces the silent drop; granted address passed; connected sites panel
- [x] T052 `cargo test` desktop — 444 pass

## Phase 6 — extension + docs

- [x] T060 esbuild entry → core `provider/inpage.js`; `protocol.js` constants pinned by test; `eth_coinbase` / `wallet_revokePermissions` in the table; stale 4001 comments fixed
- [x] T061 web unit test: `classifyMethod` vs `dappRpcClassify` (wasm) over every method name either side knows
- [x] T062 extension e2e — 23/23 on chromium. SC-305 had been failing on `main` too: the storage row's confirm sheet (spec 058) was untitled, so it never named the site, and the test predated the sheet; both fixed
- [x] T063 Delete the browser half of `dapp_permissions` — done 2026-09-23. The
  machine went from 1,533 lines to 865 and its suite from 46 cases to 23; nine
  events, fifteen functions, four operations and eleven model fields went with
  the in-app browser consent flow, which every shell had already left for
  `dapp_browser`.

  The web request window was the last thing holding it, and it was holding it
  by IMPERSONATION: `provider_request` + a grant read to reach a consent sheet,
  `navigation_started` to make a grant store notice an origin, `browser_closed`
  to read a constant back — a window with no tab, no document and no navigation
  pretending to be a browser. It has three first-class entries now
  (`PopupApproved`, `PopupAccountSwitch`, `settle_on_close`), which is what it
  was always asking for, and the deleted half took six refusal reasons and an
  error payload the window could never raise with it.

  Originally written as "move that flow onto `dapp_browser`". It is not there:
  `dapp_browser` is keyed by tab, document and request id, and a one-shot
  window has none of the three. The goal the task names — ONE browser decision
  path — is met either way.
  ~~**deferred**~~: every in-app browser is off it, but the web wallet's own connect flow (`src/lib/dapp/core/dperm-connect.ts`: `provider_request`, `navigation_started`, `browser_closed`) still drives it. Moving that flow onto `dapp_browser` is its own change. `docs/dapp-browser/ARCHITECTURE.md` rewritten (done)
- [x] T064 CI pre-push checklist: i18n vectors (no diff), rustfmt in `rust/` and the desktop, `build-web.mjs --check` after the rebuild, Swift wires (iOS suite green)

## What the owner's device pass reported, and where it went

- **「dapp 签名时，滑动到底 没有弹出用户签名的地方呀」（iPhone · 平行空间）** — not
  a browser defect, and not a signing defect: the parallel space signs with
  `vela-core`'s fixed keyset by design (043 FR-002), so no passkey sheet can
  appear. What was wrong is that NOTHING SAID SO, and the 「签名方式」 row still
  offered four routes that would not be used — so a slide that worked read as a
  slide that did nothing. Fixed in the 074 batch, not here: `b9f21429a` (debug
  builds show 平行空间内置钥匙 with no chooser, and an options-less row draws no
  chevron), with `170c6ac98` giving the slide a detent so the confirm is felt.
  `BrowserAcceptanceTests` asserts the marker before dragging the slide.
  **Both are in `075-clear-signer-channel`, which is where 070–075 have all
  been merged; a build from THIS branch alone does not have them.**
- The demo page's own signing sheet (`ExploreScreen.swift`, confirm = close) is
  gallery-only: with a `BrowserController` attached, `.browsing` requires a live
  engine, so the shipping app cannot reach it.
