# The in-app dApp browser — architecture (spec 070)

> **History (2026-09-11).** This document described the React Native / Expo app, retired and deleted in spec 039 (`specs/039-retire-expo-tree/`). It is kept as the design record; the paths and commands it names no longer exist. The idea, however, shipped: an in-app dApp browser injecting an EIP-1193/6963 provider is in the **iOS** app (spec 053, `Features/Explore/Core/BrowserEngine.swift`), the **Android** app (spec 044, `feature/browser/`) and the **desktop** app on macOS and Windows (`app-desktop/vela-wallet/src/webview.rs`, a wry-hosted WKWebView / WebView2; Linux has none — `webview_absent.rs`). The hosted web wallet has no in-page transport; the web's dApp path is the browser extension in `app-web/vela-wallet/extension/`.

> Replaces the 2026-07 design record for the retired React Native app. That
> design's security invariants (§5 there) are all carried below; its mechanism
> (a React Native view manager, a `WebViewTransport` in TypeScript) is gone.

The desktop (gpui + wry), iOS (WKWebView) and Android (WebView) wallets each ship
an in-app browser. The Chrome extension injects the same provider into Chrome
tabs. What a page may ask, and what it is answered, is decided in ONE place.

```
 page ──────────────────────────────────────────────────────────────┐
  │  provider/inpage.js  (EIP-1193 + EIP-6963, one file, every Vela) │
  │  bridge (in the same injected script): hello · req · deliver     │
  └──────────── strings ─────────────┬────────────────────────────────┘
                                     │  VelaHost.postMessage (Android WebMessageListener)
                                     │  webkit.messageHandlers.VelaHost (iOS)
                                     │  window.ipc (desktop)
 shell (Kotlin / Swift / Rust) ──────▼───────────────────────────────┐
  │  owns: WebViews, JS dialogs, external schemes, the RPC pool,     │
  │        the key-value store, the signing sheet, drawing           │
  │  forwards: page_message {tab, frame_origin, is_main_frame, json} │
  │            navigation_started / load_finished / tab_closed /     │
  │            renderer_gone / consent / revoke / signing_answered   │
  └──────────── events ▲ operations ─┬────────────────────────────────┘
                       │             ▼
 vela-core ─ app::dapp_browser (machine) + app::dapp_rpc (pure) ─────┐
  │  the routing table · per-tab, per-DOCUMENT bookkeeping ·         │
  │  per-origin chains · grants that follow the active account ·    │
  │  signing serialised and pinned to the granted address ·         │
  │  bounded reads · receipt translation · the error words           │
  └───────────────────────────────────────────────────────────────────┘
```

## Where things live

| Piece | Home |
|---|---|
| The provider | `rust/crates/vela-core/provider/inpage.js` — bundled by the extension (`extension/build.mjs`), embedded by the core |
| The injected script (provider + bridge) | `vela_core::app::dapp_rpc::provider_script(host)`; UniFFI `dapp_provider_script`, wasm `dappProviderScript` |
| The routing table | `dapp_rpc::classify` — the extension's `lib/protocol.js` `classifyMethod` is a mirror pinned by `src/lib/dapp/core-table.test.ts` |
| Everything a page is answered | the `dapp_browser` machine; contract in `specs/070-dapp-browser-core/contracts/dapp-browser.md` |
| Address-bar input | `dapp_rpc::browser_input` (a host → https, a loopback/LAN host → http, anything else → a DuckDuckGo search) |
| Tabs, favourites, groups | `app::explore_sites` |
| Recents | `app::browser_history` |
| What a signature does | `app::sign_request` + `clear_signing` + `approval_guard` (unchanged by 070) |

## Documents, not navigations

The bridge mints a document id at document start and says `hello` before the
page's scripts run; every request names its document; every answer is addressed
to one and dropped by any other. A new document retires the old one — its open
requests get **4900** (never 4001, which a dApp retries: a double spend), a
signing sheet showing one of them closes. A load that finishes without a hello
retires the old document too. A back-forward-cache restore says hello again.
Navigation callbacks settle nothing by themselves: on Android the new document's
first messages can beat `onPageStarted`.

## Security invariants

1. **The platform names the frame and its origin.** Android: `WebMessageListener`
   (`sourceOrigin`, `isMainFrame`); iOS: `frameInfo.securityOrigin` /
   `isMainFrame`; desktop: the webview's URL (wry exposes no frames; the script
   installs nothing below the top frame). A subframe message is ignored.
2. **Signing needs a grant** (4100), is refused on public http (4100), and is
   pinned to the granted address; `sign_request` refuses a mismatch.
3. **Grants follow the active account**; a grant is dropped only when the
   account list is KNOWN and no longer has its address (never on a cold read).
4. **Allowlisted reads only** — anything unknown is 4200; `eth_sign` is 4200.
5. **Exactly one answer per request**, to the tab and document that asked.
6. **Persist-at-submit** is `sign_request`'s, unchanged: a page killed after
   submit still has its record in Activity.

## Robustness

Renderer death is survived on every shell (Android `onRenderProcessGone`, iOS
`webViewWebContentProcessDidTerminate`): the tab shows "This page stopped
working" with Reload, its requests are settled, the app keeps running. Reads are
bounded (8 in flight per tab, 256 queued, then -32005). JavaScript dialogs work
while a page is on screen and are refused off screen. External schemes
(`mailto:`, `tel:`, `intent:`, app links) leave the browser only for a main-frame
navigation with a person's tap; `wc:` is answered with a sentence, not guessed at.

## Testing

- Core: `rust/crates/vela-core/tests/app_dapp_browser.rs` (every rule) and the
  unit tests in `dapp_rpc.rs`.
- Extension: `src/lib/dapp/core-table.test.ts` (table + provider constants vs
  the core over wasm), `protocol.test.ts`, the extension e2e.
- Android: `BrowserMachineTest` (the real core through the executor).
- Devices: `specs/070-dapp-browser-core/quickstart.md` — the test dApp in
  `app-android/vela-wallet/dev/testdapp` (with a cross-origin "attacker" frame on
  port + 1).
