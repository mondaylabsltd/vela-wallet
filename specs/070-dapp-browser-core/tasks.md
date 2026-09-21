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
- [ ] T037 JVM tests (executor + controller against the real core); device pass quickstart A

## Phase 4 — iOS (US1–US5)

- [ ] T040 `DbrWire.swift`, executor; `frameInfo.securityOrigin`; `webViewWebContentProcessDidTerminate`; `didFinish/didFail` → `load_finished`
- [ ] T041 Engine teardown (remove handler, weak proxy), no settle on leaving Explore, background tab filter removed (core owns it)
- [ ] T042 Delete `DappRpc.swift`, `RequestRouter.swift` decisions, bridge string/assembly
- [ ] T043 Signing pinning + `signing_answered` + `cancel_signing`; wire `receiptFor` via core
- [ ] T044 UX list (as T034), connected sites, scan routing
- [ ] T045 Hermetic tests; device pass quickstart B

## Phase 5 — desktop

- [ ] T050 `webview.rs` uses `dapp_provider_script(Desktop)`; `browser_host.rs` on `DappBrowser`; delete `executor/dapp_rpc.rs` table
- [ ] T051 Signing queue replaces the silent drop; granted address passed; connected sites panel
- [ ] T052 `cargo test` desktop

## Phase 6 — extension + docs

- [x] T060 esbuild entry → core `provider/inpage.js`; `protocol.js` constants pinned by test; `eth_coinbase` / `wallet_revokePermissions` in the table; stale 4001 comments fixed
- [x] T061 web unit test: `classifyMethod` vs `dappRpcClassify` (wasm) over every method name either side knows
- [ ] T062 extension e2e
- [ ] T063 Delete the browser half of `dapp_permissions` (once iOS and desktop are off it); rewrite `docs/dapp-browser/ARCHITECTURE.md` (done)
- [ ] T064 CI pre-push checklist (i18n vectors, rustfmt ×2, pkg-web, Swift wire defaults)
