# In-App dApp Browser (`WalletWebView`) — Architecture

Status: **design / proposal** · Author: design pass 2026-07-06 · Platforms: **iOS 16+ and Android 12 / API 31+ (both first-class)**

> Goal: let a user open an arbitrary web page — by scanning a QR or pasting/typing an
> `http(s)` URL on the home screen — inside a **custom, wallet-owned native WebView** that
> injects an EIP-1193 + EIP-6963 provider (`window.ethereum`), exactly like the iOS Safari
> extension does, but **in-process**. Connect/read stay silent; connect/sign raise the
> existing native React Native `SigningRequestModal`. Visuals and interaction must be
> first-class. We do **not** use `react-native-webview`; we build our own view component
> over `WKWebView` (iOS) and `android.webkit.WebView` (Android) so injection timing, frame
> origin, the message bridge, and navigation lifecycle are ours to control.

---

## 0. The one-paragraph version

The Safari extension already built the reusable core. `packages/safari-extension/src/inpage.js`
(the whole EIP-1193/6963 provider) and `src/lib/protocol.js` (the `vela-1193` message envelope,
method classification, error codes) are **Safari-free** — they only touch `window.postMessage`.
The app side already has the `DAppTransport` seam (`src/services/dapp-transport.ts`), a proven
transient-second-transport install (`beginExtensionSign` in `src/models/dapp-connection.tsx`) that
stamps `__transport`/`__chainId`/`__dapp` per request, and the entire signing pipeline behind
`SigningRequestModal` (clear-signing, asset simulation, never-unlimited guard, gas/funding, passkey,
ERC-4337, persist-at-submit). **The in-app browser is: (1) a custom native `WalletWebView` on both
platforms that injects `inpage.js` at document-start and relays its messages; (2) a 4th
`DAppTransport` — `WebViewTransport` — that plugs the WebView into the existing pipeline; (3) a
net-new per-origin connect-consent gate; (4) two one-line entry-point branches.** Everything the
Safari extension needed for cross-process life — App Group mailbox, `velawallet://sign`, Universal-Link
attestation, `vela.ext.account.json` snapshot — is **deleted**, because it is all one process now.

---

## 1. Layering (both platforms)

```
   React Native (JS, one runtime — same as the wallet)
   ┌──────────────────────────────────────────────────────────────┐
   │ BrowserScreen  (URL bar · security lock · back/fwd/reload ·    │
   │   share · close · loading bar · per-origin connection status)  │
   │ WebViewTransport  (implements DAppTransport)                   │
   │ Connect-consent gate  +  per-origin grant store               │
   │ ── reuses ──> DAppConnectionProvider → SigningRequestModal     │
   │                (clear-sign, asset-sim, gas/funding, passkey,   │
   │                 ERC-4337, persist-at-submit)  [UNCHANGED]      │
   └───────────────┬───────────────────────────────────────────────┘
                   │  props: uri · themeColors
                   │  events: onProviderRequest(trusted) · onNavigationChange
                   │  commands: respond(json) · emitProviderEvent(json)
   ┌───────────────▼───────────────────────────────────────────────┐
   │ Custom native view  "WalletWebView"  (legacy view manager)     │
   │   iOS: RCTViewManager → WKWebView                              │
   │   Android: SimpleViewManager<WebView> → android.webkit.WebView │
   │   • inject inpage.js + bridge shim at DOCUMENT START           │
   │   • stamp TRUSTED origin + isMainFrame from the native side    │
   │   • report navigation lifecycle                                │
   └───────────────┬───────────────────────────────────────────────┘
                   │  injects, at document-start, the SAME script on both platforms:
   ┌───────────────▼───────────────────────────────────────────────┐
   │ inpage.js (window.ethereum, EIP-1193 + EIP-6963)  [VERBATIM]   │
   │ protocol.js (vela-1193 envelope, classifyMethod, ERR codes)    │
   │ webview-bridge.js  (thin shim: relays window.postMessage ↔     │
   │                      native channel — replaces content.js)     │
   └────────────────────────────────────────────────────────────────┘
```

`inpage.js` never learns which surface it is on. Only the **last hop** differs: in Safari it is
`browser.runtime.sendMessage`; in the WebView it is `window.webkit.messageHandlers` (iOS) /
an AndroidX `WebMessageListener` (Android). The `vela-1193` envelope in between is byte-identical.

---

## 2. What already exists and is reused (the ~80%)

| Piece | Path | Reuse |
|---|---|---|
| EIP-1193/6963 provider | `packages/safari-extension/src/inpage.js` | **Verbatim.** Inject at document-start. World-guard (`:29-34`) passes in a plain WebView. |
| Wire protocol + policy | `packages/safari-extension/src/lib/protocol.js` | **Verbatim, single-sourced.** `CHANNEL='vela-1193'`, `classifyMethod`, `isSigningMethod`, `READ_PROXY_METHODS` (allowlist), `ERR` (esp. `4001` reject vs `4900` unknown-pending), `pickSignAddress`. |
| Transport seam | `src/services/dapp-transport.ts:14-54` | `WebViewTransport` is the 4th impl (after RemoteInject SSE, WalletPair WS, ExtensionBridge). |
| Transient 2nd-transport install | `src/models/dapp-connection.tsx` `beginExtensionSign :555-570` | Reuse / generalize. Already stamps `__transport`/`__chainId`/`__dapp` per request. **The hard app-side refactor is already paid for by the Safari work.** |
| Per-request response routing | `handleIncoming :277-364`, `sendResponse` sites `:697/768/784/818` | Unchanged. Already concurrency-safe via `__transport`. |
| Signing pipeline | `SigningRequestModal.tsx` / `SigningSheet`, `use-dapp-signing.ts`, approval-guard, `simulateAssetChanges`, gas/funding, passkey, bundler, persist-at-submit | **Unchanged.** The extension proves it renders "for free." |
| Bridge reference impl | `src/services/extension-bridge-transport.ts` (+ its unit test) | Template for `WebViewTransport`'s `sendResponse` fund-safety discipline; keep it off-device unit-testable. |
| Entry-point dispatch | `ConnectScreen.tsx` `handleConnect :46-61`; `HomeScreen.tsx` `connectFromUri :564-578` / `onScan :580-606` | Add one `isHttpUrl` branch to each. |
| QR decoder | `src/components/QRScanner.tsx` | **No change** — it emits one opaque string; the branch lives in the callers. |

**Deleted / not needed** (Safari cross-process only): App Group mailbox (`sign-req/-result.json`),
`velawallet://sign` + `src/app/sign.tsx`, Universal-Link attestation, `vela.ext.account.json` +
`AccountFileWriter.tsx` + `app-group-account-sync.ts`, `app-group-echo.ts`, MV3 `background.js` /
`content.js` Safari UI. The WebView reads live `activeAccount`/`chainId`/networks from context —
no snapshot, no staleness window.

---

## 3. The custom native `WalletWebView` (iOS + Android)

### 3.1 Mechanism — a legacy view manager copied by the config plugin (NOT an Expo module)

The repo commits **no** `android/` or `ios/` ( `.gitignore` ) and has **zero** Expo modules. All native
code (`vela-passkey`, `vela-app-group`) is a classic RN bridge module copied into the generated project
by `plugins/with-native-modules.js`. We follow that exact, proven pattern — a legacy `RCTViewManager`
(iOS) + `SimpleViewManager<WebView>` (Android). Under New Architecture (on by default, RN 0.83 / SDK 55),
legacy view managers render via the UIManager interop layer automatically; a WebView needs none of
Fabric's concurrent-render guarantees, so this costs nothing versus hand-writing a codegen'd Fabric
component. The plugin **already imports `RCTViewManager.h`** — a view was anticipated.

### 3.2 Files to create

```
modules/vela-wallet-webview/
├── ios/WalletWebViewManager.swift     @objc(WalletWebView) RCTViewManager → view() = WalletWebView()
├── ios/WalletWebView.swift            UIView wrapping WKWebView (the meat)
├── ios/WalletWebViewManager.m         RCT_EXTERN_MODULE + prop/command exports
└── android/src/main/java/com/velawallet/webview/
    ├── WalletWebView.kt               FrameLayout wrapping android.webkit.WebView
    ├── WalletWebViewManager.kt        SimpleViewManager<WalletWebView>, @ReactProp, events
    └── WalletWebViewPackage.kt        createViewManagers() = listOf(WalletWebViewManager())
src/modules/webview/
├── index.ts                          requireNativeComponent + Platform guard + command helpers
└── WalletWebView.tsx                 typed wrapper: <WalletWebView uri onProviderRequest onNavigationChange ref/>
```

### 3.3 Config-plugin edits (`plugins/with-native-modules.js`)

1. `withIOSSourceFiles` module list → add `'vela-wallet-webview'` (copies `ios/*` into `ios/<proj>/`).
2. `withXcodeProjectFiles` `nativeFiles` → add `WalletWebViewManager.swift`, `WalletWebView.swift`, `WalletWebViewManager.m`.
3. `withAndroidSourceFiles` `moduleMappings` → add `{ name:'vela-wallet-webview', subdir:'webview' }`.
4. `registerAndroidPackages` → add `import com.velawallet.webview.WalletWebViewPackage` + `add(WalletWebViewPackage())`.
5. Bridging header already has `RCTViewManager.h`. Android manifest: ensure `INTERNET` permission (present).

Then `npx expo prebuild --clean` regenerates both platforms with the view wired in; `requireNativeComponent('WalletWebView')` resolves on both.

### 3.4 Native contract (identical semantics on both platforms)

**Props (JS → native):** `uri: string`, `themeColors: {toolbar,controls}` (optional).

**Document-start injection.** Both platforms inject `inpage.js` + `protocol.js` + `webview-bridge.js`
**before any page JS runs**, main-frame only:
- iOS: `WKUserScript(source, injectionTime: .atDocumentStart, forMainFrameOnly: true)` in the
  `WKWebViewConfiguration.userContentController`.
- Android: `WebViewCompat.addDocumentStartJavaScript(webView, script, {"*"})` (AndroidX WebKit;
  requires the `DOCUMENT_START_SCRIPT` feature — available on our API 31+ WebView).

**Page → native channel (provider requests):**
- iOS: `configuration.userContentController.add(handler, contentWorld: .page, name: "velaBridge")`;
  the shim calls `window.webkit.messageHandlers.velaBridge.postMessage(msg)`.
- Android: `WebViewCompat.addWebMessageListener(webView, "velaBridge", {"*"}, listener)`; the shim
  calls `window.velaBridge.postMessage(JSON.stringify(msg))`.

**Trusted-context stamping (SECURITY — §5.1).** On every inbound message the native side attaches
context the page **cannot forge**, then bubbles `onProviderRequest`:
```jsonc
{ "requestId", "tabId", "navigationId",
  "origin",       // iOS: message.frameInfo.securityOrigin ; Android: listener sourceOrigin — NEVER the JS body
  "isMainFrame",  // iOS: message.frameInfo.isMainFrame ; Android: isMainFrame arg
  "method", "params" }
```

**Native → page (responses + events)** via imperative commands (`UIManager.dispatchViewManagerCommand`,
or an exported `RCT_EXTERN_METHOD`):
- `respond(requestId, json)` → native runs `window.postMessage({ch:'vela-1193',dir:'res',id,result|error}, origin)`
  in the page (self-post, so `inpage.js`'s `ev.source===window` receiver accepts it).
- `emitProviderEvent(json)` → same, `dir:'evt'` (`accountsChanged`/`chainChanged`/`connect`/`disconnect`).

**Navigation lifecycle → `onNavigationChange`:** `{url, canGoBack, canGoForward, loading, title, navigationId, securityOrigin}`
from `didCommit`/`didFinish`/`didFail` (iOS `WKNavigationDelegate`) and `onPageStarted`/`onPageFinished` +
`WebViewClient.shouldOverrideUrlLoading` (Android). Drives the URL bar **and** the settle-on-navigation guard (§5.3).

### 3.5 Injected-JS delivery — single-source, no drift

`inpage.js` and `protocol.js` are **the same files the Safari extension ships.** Extend
`packages/safari-extension/build.mjs` to emit one extra bundle, `walletwebview-inject.js`
(= `inpage.js` + a thin `webview-bridge.js` shim, sharing `protocol.js`), and have the config plugin
copy it into the native module's resources at prebuild. The native side loads that string and injects it
at document-start. One source of truth → the two surfaces (Safari + in-app) can never drift. (Memory:
avoid the cross-repo string-drift class of bug.)

The `webview-bridge.js` shim is tiny — it fills only `content.js`'s pure-relay role (no Safari UI, no
sign hand-off):
```js
// page → native
addEventListener('message', (ev) => {
  if (ev.source !== window) return;
  const d = ev.data;
  if (!d || d.ch !== 'vela-1193' || d.dir !== 'req') return;
  NATIVE_POST(d);                 // webkit.messageHandlers.velaBridge / window.velaBridge
});
// native → page: native evaluates window.postMessage(...) directly; inpage receives it.
```

---

## 4. `WebViewTransport` — the 4th `DAppTransport`

`src/services/webview-transport.ts`, implementing `DAppTransport` (`dapp-transport.ts:24-34`). It is a
**live, persistent** channel (unlike the one-shot `ExtensionBridgeTransport`), because the WebView stays
mounted:

- `connect()` → mark connected, emit `'connected'`. (Nothing to poll — the channel is live.)
- native `onProviderRequest` → `emit('request', requestId, method, params, origin)` with `params`
  already in Ethereum array shape (an EIP-1193 provider speaks native `eth_*` — **no mapping**), and
  `origin` = the **native-stamped** origin.
- `sendResponse(id, result?, error?)` → call the native `respond(id, {result}|{error})` command. Must
  never throw; idempotent per id. **Keep the fund-safety discipline:** `4001` only for explicit user
  reject; every other failure is a non-`4001` code (`-32603`/`4900`) so a dApp never treats an
  ambiguous failure as "safe to retry."
- `pushWalletInfo(info)` → **implement for real** (the extension no-op'd it): call `emitProviderEvent`
  with `accountsChanged`/`chainChanged` when the active account or chain changes. A live channel exists.
- `fetchDAppInfo()` → `{name: page title || host, url, icon: favicon}` from the current navigation.
- `disconnect()` → on WebView close/destroy; settle any pending request first (§5.3).

**Install via the transient pattern**, not `wireTransport` — reuse/generalize `beginExtensionSign` so
each request carries `__transport` (response routing), `__chainId` (the tab's active chain, so signing
targets the right network without mutating global state), and `__dapp` (origin identity). This is the
only correct way to share the single-slot `incomingRequest`.

For **single-tab v1** we also set the global `chainId` to the tab's active chain, so read-only RPC
(which today resolves against the global chain, `handleIncoming :356`) is correct without new work.
Read-only + `wallet_switchEthereumChain` need no special handling — emit them as normal `'request'`
events and the brain forwards them to `handleReadOnlyRPC` / answers `switch` inline.

---

## 5. Security invariants (a WebView that runs arbitrary sites is a wallet attack surface)

These are **requirements**, carried from the Safari hardening (`docs/safari-extension/ARCHITECTURE.md` §9/§12):

1. **Native-authoritative origin.** The `origin` on every request comes from the WebView's committed URL /
   `frameInfo.securityOrigin` (iOS) / `sourceOrigin` (Android). **Never** trust an origin in the JS body.
   The `SigningRequestModal`, history `recordOrigin`, and SIWE guard compare the native origin.
2. **Main-frame-only signing.** Gate connect/sign on `isMainFrame`. A cross-origin iframe must not be able
   to request accounts or signatures. (Provider is injected `forMainFrameOnly`.)
3. **Per-origin permissions; no logout on cold miss.** Grant store keyed by origin (persisted
   `vela.perm.<origin>`). `eth_accounts` returns `[]` for ungranted/locked and **never prompts**; only
   `eth_requestAccounts`/`wallet_requestPermissions` open the connect sheet. Re-validate the granted
   address against current accounts, and drop a grant **only when the account list is present and no longer
   lists it** — never on a transient empty read (mirror `background.js:90-97`).
4. **Settle-on-navigation.** On top-frame navigation to a new origin, reload, or WebView destroy, **reject
   or resolve every in-flight request** and tear down its modal. Use a **non-`4001`** code (`4900`
   unknown-pending) — only an explicit user tap is `4001` (a dApp retries `4001` → double-spend). Deliver a
   pending sign result **before** any `chainChanged`/`accountsChanged` in the same cycle (a spurious
   `chainChanged` makes dApps `location.reload()` and orphan the sign).
5. **Persist-at-submit is the source of truth.** Keep writing `LocalTransaction{status:'pending', dappOrigin,
   userOpHash}` the instant the bundler accepts. A WebView can be killed under memory pressure between submit
   and page-resolve; Vela Activity must still show the real outcome.
6. **Allowlist read routing.** Keep `READ_PROXY_METHODS` as an allowlist so the browser can't be an open RPC
   relay; reject `eth_sign` outright; every sign goes through `SigningRequestModal` + never-unlimited guard +
   asset simulation — no fast path.
7. **Zero-key provider.** The injected page script stays a dumb relay: no key material, no signing logic, no
   store access. Native owns all policy. Keep `isVela:true` + clean EIP-6963; **do not** spoof `isMetaMask`.

---

## 6. Browser chrome / where it mounts

- A **non-modal** full-screen expo-router route `src/app/browser.tsx` (like `src/app/sign.tsx`, deliberately
  **not** `presentation:'modal'`) so the root-level `<SigningRequestModal>` (mounted above the Stack at
  `_layout.tsx:117`) renders **over** the browser. A modal-presentation route would hide the sheet behind it
  on iOS (the documented single-native-modal constraint).
- Chrome: compact top bar (security lock + origin/title + close), bottom bar (back / forward / reload /
  share / open-in-system), a thin determinate loading bar, and a per-origin connection chip (connected
  account + chain, tap to disconnect). Follow `docs/DESIGN-LANGUAGE.md` (minimal, hairline, accent = action).
- Connect-consent UI: a dedicated lightweight `ConnectSheet` (origin + favicon + account/chain picker +
  Approve/Reject) — reuse `AppModal` + the account/currency pickers. Signing continues to use
  `SigningRequestModal` unchanged.

---

## 7. Entry points (both are one branch; QR scanner unchanged)

Shared guard (add next to `parseRemoteInjectURL`):
```ts
export function isHttpUrl(raw: string): boolean {
  try { const u = new URL(raw.trim()); return u.protocol === 'http:' || u.protocol === 'https:'; }
  catch { return false; }
}
```
Order matters: a Remote-Inject link is itself `https://`, so the browser branch is the **fallback after
`parseRemoteInjectURL` returns null**, before the "invalid" alert.

- **A — `HomeScreen.tsx` `connectFromUri :564-578` (primary: scan FAB + Connections paste).** Insert before
  `return false;`: `if (isHttpUrl(trimmed)) { router.push({pathname:'/browser', params:{url:trimmed}}); return true; }`.
  Because `onScan` runs EIP-681 → address → `connectFromUri`, a scanned dApp URL naturally lands here; the
  `true` return also suppresses the paste "invalid link" alert. One edit covers scan **and** paste.
- **B — `ConnectScreen.tsx` `handleConnect :46-61`.** In the `if (!parsed)` block:
  `if (isHttpUrl(trimmed)) { router.push('/browser', {url:trimmed}); return; }` before `showAlert(...invalid)`.

The paste `TextInput` placeholder copy should widen to "Paste a link or dApp URL" (i18n).

---

## 8. Single-tab now → multi-tab later

v1 is **single-tab** (one live `WalletWebView`). Multi-tab is deferred; the contract already carries
`tabId`/`navigationId` so it's additive. The three brain assumptions multi-tab must later solve
(none block single-tab):
1. `incomingRequest` is a single slot → needs a **serialization queue** for concurrent sign sheets.
2. Read-only RPC resolves the **global** chain (`handleIncoming :356`) → needs per-tab chain threaded into
   read dispatch (single-tab sidesteps this by making the tab's chain the global chain).
3. `status`/`dappInfo`/`session` are single values → per-tab connection status UI is net-new.

---

## 9. Browser-completeness backlog (harden after the vertical slice works)

Cookies / `localStorage` / cache persistence · camera / geolocation / file-picker permission prompts bridged
to native · downloads & uploads · full-screen
video · SSL-error and Safe-Browsing UI · renderer-crash recovery (`webViewWebContentProcessDidTerminate` /
`onRenderProcessGone`) · optional
phishing blocklist (e.g. eth-phishing-detect). Each item is a native capability the wallet must decide to
grant — that's the point of owning the WebView.

### 9.1 Post-review hardening (2026-07-10)

Applied after the multi-agent review of PR #67 (JS: typecheck + unit tests green; iOS Swift: compiled for the simulator; Android Kotlin + on-device e2e: NOT yet run):

- **Origin/isMainFrame from the initiating frame (iOS).** `processBridge` now stamps both from the request's own `WKFrameInfo` (`securityOrigin` / `isMainFrame`) across all three channels (prompt, `velawvbridge://` scheme, message handler) — a subframe can no longer forge a main-frame request (§5.1/§5.2). Connect/state methods in `browser.tsx` are gated on `isMainFrame` too.
- **`target=_blank` / `window.open`** → loads in the same view (iOS `createWebViewWith`; Android default). **External schemes** (`mailto:`, `tel:`, `wc:`, app links) → handed to the OS; `javascript:`/`file:`/`data:` navigations are dropped.
- **Load-error state** surfaced to RN (`NavigationChangeEvent.error`) with a branded retry screen; earlier loading feedback (iOS `didStartProvisionalNavigation`).
- **Insecure-origin signing gate** (public `http://` blocked, localhost/LAN exempt); **route-param re-validated** as http(s) so the `velawallet://browser?url=` deep link can't load a non-web scheme.
- **iOS WKWebView leak fixed** (weak message-handler proxy); Android white-screen fixed (page loads even without `DOCUMENT_START_SCRIPT`, with a late-inject fallback); Android SPA URL bar via `doUpdateVisitedHistory`.
- Connect-consent now coalesces duplicate prompts, clears on navigation, and pushes `chainChanged`; debug scaffolding removed; full i18n (en + zh/zh-TW/zh-HK; other 10 locales fall back to `en` pending translation).

Still open: iOS SPA `pushState` URL tracking, camera/file-picker prompts, renderer-crash recovery, phishing blocklist, and a device run of the Android path + the on-device e2e.

---

## 10. Phased plan

| Milestone | Ships | Proves |
|---|---|---|
| **M0 — contract freeze** | Extract `walletwebview-inject.js` from the Safari build (single-source `inpage.js`+`protocol.js`+shim); write the `WebViewTransport` skeleton + off-device unit test (mock native, like the extension test). | The envelope + transport contract, testable without a device. |
| **M1 — native WebView, both platforms** | `modules/vela-wallet-webview` (iOS WKWebView + Android WebView) + config-plugin wiring + `src/modules/webview`. Load a URL, inject provider at document-start, `onProviderRequest`↔`respond`, navigation events. | `window.ethereum.request({method:'eth_chainId'})` answered from RN on **both** iOS and Android. No chrome yet. |
| **M2 — wire to signing** | `WebViewTransport` via the transient install; a hardcoded dApp does `eth_requestAccounts` (connect sheet) → one `signTypedData`/`eth_sendTransaction` through the real `SigningRequestModal`; `accountsChanged`/`chainChanged` push. | End-to-end connect + sign, both platforms, clear-signing + funding + passkey intact. |
| **M3 — chrome + entry points** | `src/app/browser.tsx` full UI; `isHttpUrl` branch in HomeScreen + ConnectScreen; EIP-6963 verified against a real dApp (Uniswap/Aave). | A user scans/pastes a URL and transacts. |
| **M4 — security hardening** | Settle-on-navigation (`4001` vs `4900`), main-frame gating, per-origin grant store + no-logout-on-cold-miss, persist-at-submit paths. | The invariants in §5 hold under adversarial navigation. |
| **M5 — browser completeness** | §9 backlog, prioritized by real dApp breakage. | Feels like a real browser. |
| **M6 (later)** | Multi-tab (§8); optionally route the existing external links (`openBrowser`) through `WalletWebView`. | — |

---

## 11. Open decisions

1. **v1 tab model:** single-tab (recommended) vs multi-tab from the start.
2. **v1 scope:** thin vertical slice first (M0–M2, one hardcoded dApp) vs build more before any device test.
3. **Injected-JS single-source:** share `inpage.js`/`protocol.js` with the Safari extension via one build
   (recommended) vs fork a copy for the browser.
4. **Connect-consent UX:** dedicated lightweight `ConnectSheet` (recommended) vs reuse the signing sheet styling.
5. **External-link migration:** eventually route `openBrowser` links through `WalletWebView` (§10 M6) — out of scope for v1.
