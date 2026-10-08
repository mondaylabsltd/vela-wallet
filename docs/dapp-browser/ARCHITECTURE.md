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
| The injected script (provider + bridge) | `vela_core::app::dapp_rpc::provider_script(host, debug_mode)`; UniFFI `dapp_provider_script`, wasm `dappProviderScript` |
| Who is offered the wallet | `dapp_permissions::offers_wallet(origin, debug_mode)` — a secure context (spec 088), plus http on the device's own network while Settings' hidden debug mode is on — developer builds only (spec 091). The page gate and the script both follow it; the script's host test is written by the core (`private_host_js`) |
| The routing table | `dapp_rpc::classify` — the extension's `lib/protocol.js` `classifyMethod` is a mirror pinned by `src/lib/dapp/core-table.test.ts` |
| Everything a page is answered | the `dapp_browser` machine; contract in `specs/070-dapp-browser-core/contracts/dapp-browser.md` |
| Address-bar input | `dapp_rpc::browser_input` (a host → https, a loopback/LAN host → http, anything else → a DuckDuckGo search) |
| Tabs, favourites, which of the two home sections (Favorites, Recent dApps) show — no custom groups since #465 | `app::explore_sites` |
| What Explore shows on entry; which tab an opened site goes into | `app::browser_tabs` — `explore_landing`, `open_target`, `lit_tab` (UniFFI `explore_landing`, `browser_open_target`, `browser_lit_tab`, `browser_waiting_tab`) |
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
while a page is on screen and are refused off screen. External schemes leave the
browser only on a person's tap. Phones hand any app scheme to the system
(`intent:` sanitised). The desktop hands on only `mailto:`/`tel:` (spec 083
Windows, spec 095 macOS): a tap in the page's own document, never in a
cross-origin frame, and escaped as Chromium escapes a handler's command value,
up to 2048 bytes. On macOS a navigation gate in front of wry's delegate reads
the action's `navigationType` and `sourceFrame` — the iOS rule, link activated
in the main frame. `javascript:`/`file:` never leave. `wc:` is answered with a
sentence on the phones, not guessed at (desktop: refused silently for now). A
new window loads in the same tab on the phones and opens a new tab on the
desktop (Windows from the request's own gesture flag; macOS behind WebKit's
popup blocker, `javaScriptCanOpenWindowsAutomatically = NO`), only on a
person's gesture; no shell builds a second engine, so
`window.opener` is gone and popup sign-in does not complete. Since spec 099
every client keeps one engine per tab — a switch shows a live page, it never
reloads it — so each tab's Back is its own history. Downloads are refused
everywhere.

## The layers, and where each is seen (spec 099)

A dApp tab is six layers deep. When something does not work, the first
question is which one stopped; the answer is decided in the core, and every
client says and logs it the same way.

| Layer | What it is | Decided in | Seen as |
|---|---|---|---|
| browser | the tab and its page: loading, loaded, crashed, gone | `browser_load` (the load watch), `dapp_browser` (page state) | the hairline, the load-failure and crash panels, the tab status |
| provider | the wallet offered to the page (EIP-1193 + 6963) | `dapp_permissions::offers_wallet`, `dapp_browser` (provider state) | the tab status: offered, not offered (insecure page, script did not run) |
| wallet | Vela's own rules over a request | `dapp_rpc::classify`, `dapp_browser` | the request's row: not connected, unsupported, unknown chain… |
| network | the chain's RPC, through the person's endpoints | the shell's pool, bounded by `READ_DEADLINE_MS`; `dapp_record` names the failure | the chain notice; the row: no endpoint, timed out, rate-limited, the endpoint's error |
| relay | Vela's relay: quote, simulate, send | `fee_policy`, `sign_request`, `tx_tracker` | the fee row's reason, the landing (relay sending / funding / on the network), the row |
| sheet · signer | the person on Vela's sheet; the passkey | `sign_confirm::confirm_state`, `sign_request` (signer kinds) | the line under a shut slide; the signer's failure on the sheet and in the row |

- **The request record.** Every request a page sends gets a row in
  `dapp_browser` (`dapp_record::DbrRequestRow`): method, class, start and end
  (the shell's clock), outcome and code, and the layer and reason that ended it
  — never params, results, signatures or addresses. The last 200 per tab. The
  view carries counts for every tab (`busy`, `open_requests`, `failed_recent`,
  `last_failure`) and the whole record only for the tab whose status panel is
  open (`inspector`), with a copyable `report`.
- **The log.** One line per request end and per change of a tab's page or
  provider state, written by the core (`DbrOperation::Log`) and put in the
  app's log as it is: `dapp tab=… req=… method=… class=… outcome=… code=…
  layer=… reason=… ms=…`.
- **Live engines.** `browser_tabs::plan_engines` keeps the shown tab and every
  busy tab, then the most recently used up to six (one under memory pressure);
  the rest are suspended and say "reloaded to save memory" when shown again.
- **Where Explore lands.** `browser_tabs::explore_landing`: Explore chosen from
  another section, or again while it is up, shows its home — the dApp's tab
  kept live and offered first in `ExploreView.resumable` (at most three rows);
  a page opened from outside shows its tab; a tab with a request waiting on
  the person (`waiting_tab`) shows that tab, whatever the entry.
- **Where an opened site goes.** `browser_tabs::open_target`: on a page, the
  address bar loads there; over the home, a picked site a tab is already on
  resumes that tab, otherwise the selected start-page tab gets its first page,
  otherwise a new tab — a live dApp is never replaced from the home. A typed
  address never takes the same-site rule. A full strip (24 tabs) has no room
  for a new tab, so the selected tab takes the address rather than the open
  doing nothing.
- **Every read settles** by `READ_DEADLINE_MS` (30 s), whatever the endpoints
  are doing.
- **The landing** counts the chain's usual time from when the relay put the
  bundle on the network (`tx_tracker::landing_pace`, `relay_sent_at_ms`),
  never from acceptance; an op the relay acknowledged and later forgot ends
  "not sent" once the chain has been read to its head with no event for it.

## Testing

- Core: `rust/crates/vela-core/tests/app_dapp_browser.rs` (every rule),
  `app_dapp_browser_099.rs` (the record, the layers, the deadline),
  `app_browser_tabs_099.rs` (live engines, the landing, the open target), `app_sign_landing_099.rs` (the
  gate, the forgotten op, the countdown), and the unit tests in `dapp_rpc.rs`.
- Extension: `src/lib/dapp/core-table.test.ts` (table + provider constants vs
  the core over wasm), `protocol.test.ts`, the extension e2e.
- Android: `BrowserMachineTest` (the real core through the executor).
- Devices: `specs/070-dapp-browser-core/quickstart.md` — the test dApp in
  `app-android/vela-wallet/dev/testdapp` (with a cross-origin "attacker" frame on
  port + 1).
