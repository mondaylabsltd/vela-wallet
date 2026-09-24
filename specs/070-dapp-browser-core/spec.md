# Feature Specification: One dApp browser engine, in the core

**Feature Branch**: `070-dapp-browser-core`
**Created**: 2026-09-22
**Status**: Draft
**Input**: Owner, 2026-09-22 — "帮我把 Android 中的 dapp browser 探索 这个功能模块整明白，测试好，并考虑是否有部分代码可以抽出到 vela core，因为注入到网页内容在 desktop ios 以及 chrome extension 应该都是一样的；同时把 desktop ios chrome extension 中的连接 dapp（inject），签名整明白。体验要一致，UI/UX 要好用，简单易懂。体验还要稳定，不要动不动就出错，进行不下去，要很健壮 … 在不稳定的环境构建稳定的用户体验，以及清晰简单可维护的架构，减少重复代码。"

Owner decisions are marked **[R]**; everything else is derived from source
read on 2026-09-22 (five audits: Android browser, iOS browser, desktop +
extension, signing methods, settings) and marked **[D]** where a choice had to
be made.

## Why

The page-side provider is already one file (`extension/inpage.js` +
`extension/lib/protocol.js`), copied into every client. Everything AROUND it
was written three or four times and has drifted:

| Duplicated | Where | Drift that reaches a person |
|---|---|---|
| Method routing table (sign / state / switch / add / watch / read / unsupported) | `protocol.js`, desktop `executor/dapp_rpc.rs`, iOS `DappRpc.swift`, Android `DappRpc.kt` | `eth_sign` and unknown methods answer **4900 "disconnected"** on native, **4200** in the extension; `wallet_addEthereumChain` answers `null` and changes nothing on native, so the page believes it switched while reads go to the old chain; `wallet_watchAsset` is `null` vs `false` |
| Request routing (`RequestRouter`), request→tab map, open-id set, settle | iOS `BrowserController.swift` + `RequestRouter.swift`, Android `BrowserController.kt` + `RequestRouter.kt`, desktop `page.rs` | a navigation in a BACKGROUND tab settles every tab's requests; answers fall back to the FRONT tab; `accountsChanged` from a background tab's approval goes to the front page (an address leak) |
| Provider assembly (strip `import`/`export`, wrap, append bridge) | desktop `webview.rs:69`, iOS `ProviderBridge.swift:65`, Android `ProviderBridge.kt:61` | three hand-typed bridges with different frame rules |
| The browser's chain | one global `browserChain = 100`, in memory, on every native shell; the extension keeps it per origin (`vela.chain.<origin>`) | a site's chain resets on every launch; one site's switch moves every site |
| "One signing sheet at a time" | Kotlin / Swift / desktop each decide it | desktop silently DROPS the first request when a second arrives; iOS/Android answer -32002 |
| Signing-method definition | core `dapp_permissions::SIGNING_METHODS` (8 names) vs core `sign_request::is_signing_method` (`contains("signTypedData")`) | `eth_signTypedData_v2` skips the insecure-http gate but still reaches a sheet |

And three security defects the audits found by reading:

1. **Android: a cross-origin iframe can speak as the top page.**
   `addJavascriptInterface("VelaHost")` is visible to every frame, the host
   attaches the TOP page's URL and hard-codes `is_main_frame = true`
   (`BrowserController.kt:380`). An ad iframe can call
   `VelaHost.post('{"method":"eth_sendTransaction",…}')` and the sheet shows
   the host dApp's name.
2. **Android + desktop: signing is not pinned to the granted account.**
   `granted_address` is never passed to `sign_request`; a site granted
   account A signs with account B if B is active.
3. **iOS: the origin is read from `webView.url` at message time**, not from
   the frame that sent it — a document that is being navigated away can post
   under the next site's name.

Robustness gaps that make a session "动不动就出错": no renderer-crash handling
on Android (`onRenderProcessGone` absent → the app dies) or iOS
(`webViewWebContentProcessDidTerminate` absent → a blank page, requests hang);
malformed or oversized requests are dropped silently so the page's promise
never settles; iOS closed tabs are never torn down (retain cycle) and keep
running scripts that can still raise sheets; the signing sheet stays open
after its page navigated away and the result is delivered into the next
document.

**[R]** "考虑是否有部分代码可以抽出到 vela core" — this spec moves the whole
decision half of the browser into the core. A shell keeps what only it can
do: own a WebView, post a string into it, run an RPC call, draw.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — A dApp works the same on every Vela (Priority: P1)

A person opens a dApp (the test dApp, then a public one) in Explore on
Android, iOS or desktop, connects, reads, switches chain, signs a message and
sends a transaction. Every answer the page sees — result, error code, event —
is decided by one core function and is identical on all three clients and
equal to the extension's where the extension has the same situation.

**Why this priority**: it is the owner's first ask and the base the other
stories stand on.

**Independent Test**: the core's conformance table (`tests/app_dapp_browser.rs`)
drives every method through the machine; each shell's test replays the same
table through its executor; on a device the test dApp's buttons produce the
same `#out` on Android and iOS.

**Acceptance Scenarios**:

1. **Given** a connected site, **When** it calls `wallet_addEthereumChain` for
   a chain the wallet has, **Then** the site's chain switches, `chainChanged`
   fires, the call answers `null`, and the next `eth_chainId` returns it.
2. **Given** any site, **When** it calls `eth_sign` or an unknown method,
   **Then** it gets **4200** (unsupported), never 4900.
3. **Given** a site on Base, **When** the app restarts and the site reloads,
   **Then** it is still on Base (the chain is kept per origin).
4. **Given** site A on Gnosis and site B on Ethereum in two tabs, **When** A
   switches to Base, **Then** B stays on Ethereum and hears nothing.
5. **Given** connected sites, **When** the person switches the wallet's
   account, **Then** every grant follows the new account (the extension's
   `followActiveAccount` rule, now the core's for every shell), each open tab
   of those origins hears `accountsChanged`, and a signature is always made
   by the account the site was shown — `forward_to_signing` carries the
   granted address and `sign_request` refuses a mismatch (4100).

---

### User Story 2 — Nothing hangs, nothing leaks, nothing lands in the wrong page (Priority: P1)

A person uses several tabs, navigates while a sheet is open, closes tabs, and
the WebView's renderer crashes. Every request a page made gets exactly one
answer, in the document that asked, and a dead page never raises a sheet.

**Why this priority**: "体验还要稳定，不要动不动就出错，进行不下去".

**Independent Test**: core tests for each lifecycle transition; a device pass
with the test dApp: sign → navigate before approving; sign → close the tab;
connect in a background tab; kill the renderer
(`adb shell am crash`-free: `chrome://crash` in the page on Android, a
`webViewWebContentProcessDidTerminate` injection on iOS).

**Acceptance Scenarios**:

1. **Given** a signing sheet for page P, **When** P navigates away, **Then**
   P's promise gets 4900 once, the sheet closes, and nothing is delivered into
   the next document.
2. **Given** a request from tab 2 while tab 1 is in front, **When** it is
   answered, **Then** only tab 2's document receives it; tab 1 hears nothing.
3. **Given** a connect approved for tab 2, **Then** `accountsChanged` reaches
   every tab showing that origin and no other tab.
4. **Given** the renderer process dies, **Then** the app stays up, the tab
   shows "This page stopped working" with Reload, and every open request of
   that tab got 4900.
5. **Given** a malformed request (no id, oversized params), **Then** a request
   with a usable id gets -32600/-32602 and one without an id is dropped — the
   page never waits forever on something it can be told about.
6. **Given** a cross-origin iframe calls the host bridge directly, **Then**
   the request is refused (4100 for connect/sign) and attributed to the
   iframe's own origin — never to the top page.
7. **Given** two signing requests arrive together, **Then** the second waits
   in line and opens when the first is answered — neither is dropped.

---

### User Story 3 — The browser is easy to use (Priority: P2)

Typing a word searches; a loading bar shows progress; a failed load shows
why with Retry; system Back goes back in the page; the address bar can be
edited on an open page; the star removes a favourite that is already one;
Share, Copy link, Open in system browser and Disconnect in the site menu
work; the connection panel can switch the site's network; an insecure site is
never drawn with a green lock; JavaScript `alert`/`confirm` work.

**Why this priority**: "UI/UX 要好用，简单易懂" — each item was found dead or
missing on at least one native shell.

**Independent Test**: device pass on Android and iOS against the checklist in
`quickstart.md`.

---

### User Story 4 — Connected sites are listed and revocable everywhere (Priority: P2)

Settings shows every connected site (origin, account, network, when) with
Disconnect, on web (exists), desktop, iOS and Android. Disconnecting a site
that is open in a tab tells that page (`accountsChanged []` + `disconnect`).
Clearing "dApp connections" in Storage revokes them in the live session too
(today the core's in-memory copy survives until restart).

**Independent Test**: connect two sites, revoke one from Settings, return to
its tab: `eth_accounts` is `[]` and the page heard `disconnect`.

---

### User Story 5 — The Explore scan button does what it says (Priority: P3)

Decision D1 of `docs/PARITY-2026-09-21.md` (#273), option (b), recommended
there and adopted here **[D]**: a scanned `http(s)` URL opens in the browser;
an address / `ethereum:` link goes to Send through the core's `scan_resolved`;
a `wc:` link says honestly that WalletConnect is not supported and to open
the dApp in Explore instead.

### Edge Cases

- A navigation whose first document message arrives BEFORE the shell's
  "navigation started" callback (Android, recorded in 044): the core keys
  requests by DOCUMENT, not by navigation event (see research R2), so the new
  document's warm-up requests are never settled by the late callback.
- A page that calls `wallet_switchEthereumChain` to a chain the wallet does
  not have: 4902, chain unchanged.
- `wallet_addEthereumChain` for an unknown chain: 4902 with a message that
  networks are added in Vela's Settings **[D]** (the extension's behavior).
- A revoked grant whose account was deleted: dropped only when the account
  list is KNOWN (invariant ② kept).
- Leaving Explore does NOT disconnect anything; only closing a tab,
  navigating, or a crash settles that tab's requests (iOS today clears the
  connected chip on every leave and never restores it).
- The page answers faster than the core: a response for an id that is no
  longer open is dropped (exactly one answer per id).
- A flood of reads: at most 8 in flight per tab, 256 queued; beyond that
  -32005 "limit exceeded" (F06, never implemented natively).

## Requirements *(mandatory)*

### Functional Requirements

**Core — `vela_core::app::dapp_browser` (new machine) and `dapp_rpc` (new pure module)**

- **FR-001**: The core MUST own the complete routing of a provider request:
  validation (shape, id ≤128, method ≤100, payload ≤512 KiB — the
  extension's `isWellFormedRequest`), frame and origin gating, grants,
  consent, state answers (`eth_chainId`, `net_version`, `eth_accounts`,
  `eth_coinbase`, `wallet_getPermissions`), chain switch and add, watch-asset,
  reads (as an operation), signing (as an operation), revoke
  (`wallet_revokePermissions`), and refusal codes. One table, one function
  (`dapp_rpc::classify`), one set of signing methods for the whole core.
- **FR-002**: The core MUST keep state per TAB and per DOCUMENT: which
  document each open request came from, which origin each tab shows, which
  requests each tab has open, and deliver every answer and event to a named
  tab + document. There is no "front tab" fallback anywhere.
- **FR-003**: The chain MUST be per origin and persisted
  (`vela.chain.<origin>`, the extension's key); a new origin starts on the
  grant's chain, else the default chain **[D: 1, Ethereum — the extension's
  `DEFAULT_EXT_CHAIN_ID`]**.
- **FR-004**: Signing MUST be serialised by the core: one request is forwarded
  at a time, the rest wait in order; a queued request whose document dies is
  settled 4900 and removed. `ForwardToSigning` MUST carry the origin's chain
  and the GRANTED address.
- **FR-005**: When a document dies (navigation, tab closed, renderer gone),
  every request it had open MUST be settled 4900 exactly once, and a signing
  sheet showing one of them MUST be told to close (`CancelSigning`).
- **FR-006**: Receipt translation (a page asking `eth_getTransactionReceipt`
  for the user-operation hash it was answered with) MUST be the core's: the
  core remembers the hashes it delivered and asks the shell to resolve them.
- **FR-007**: The core MUST publish the page-side script
  (`dapp_provider_script(host)`): the ONE provider (moved to
  `rust/crates/vela-core/provider/inpage.js`, no module syntax) plus ONE
  bridge whose only per-host difference is the function that posts a string.
  The script installs nothing in a subframe. Every message carries a
  per-document id; `__velaDeliver` drops a message addressed to another
  document.
- **FR-008**: The extension MUST bundle the same `provider/inpage.js`; its
  service worker's routing table stays JS (it cannot run the core) and a web
  unit test MUST fail if it disagrees with `dapp_rpc::classify` over wasm.
- **FR-009**: The view MUST list the connected sites (origin, address, chain,
  granted-at) for Settings, and per tab the origin, connected address, chain,
  and whether the page crashed.
- **FR-010**: Reads MUST be bounded (8 in flight per tab, 256 queued, then
  -32005).

**Shells — Android, iOS, desktop**

- **FR-011**: Each native shell MUST drive `dapp_browser` and delete its own
  routing table, router decisions, request bookkeeping, provider assembly and
  bridge string. What remains: WebView ownership, posting strings, the RPC
  executor, the grant/chain store, and drawing.
- **FR-012**: Android MUST receive page messages through
  `WebViewCompat.addWebMessageListener` (the platform supplies `sourceOrigin`
  and `isMainFrame`); `addJavascriptInterface` is removed. iOS MUST take the
  origin from `message.frameInfo.securityOrigin`.
- **FR-013**: Android (`onRenderProcessGone`) and iOS
  (`webViewWebContentProcessDidTerminate`) MUST survive a renderer death:
  the tab shows a reload state and the core is told.
- **FR-014**: iOS MUST tear down a closed tab's WebView (remove the message
  handler, break the retain cycle) and stop settling or clearing state when
  the person merely leaves Explore.
- **FR-015**: Background tabs MUST be paused (Android `WebView.onPause`,
  iOS: no script delivery to a hidden tab is needed beyond what the core
  addresses) **[D]**.
- **FR-016**: The browser chrome MUST provide: search for non-URL input
  (DuckDuckGo **[D]**, privacy-preserving; decided by a core function so all
  shells agree), a progress bar, an error page with Retry, system Back = page
  back (Android), an editable address bar on an open page, a star that
  toggles, working Share / Copy link / Open in system browser / Disconnect,
  a network row that switches the site's chain, a lock that tells the truth,
  JavaScript dialogs, and external schemes (`mailto:`, `tel:`, app links)
  handed to the OS only for a main-frame navigation with a user gesture.
- **FR-017**: Every connected site MUST be listed in Settings with Disconnect
  on desktop, iOS and Android; the Storage "dApp connections" clear MUST
  revoke in the live core too.
- **FR-018**: The Explore scan button MUST route: URL → browser, address /
  `ethereum:` → Send, `wc:` → an honest "not supported" message.

**Extension alignment**

- **FR-019**: The extension MUST adopt the core's codes where they differ
  (none expected beyond pinning; `wallet_revokePermissions` and `eth_coinbase`
  are added to its table), and its stale "closed window = 4001" comments
  (`background.js:17`, `protocol.js:70`, `transport.ts:18,74`) MUST be
  corrected to 4900.

### Key Entities

- **Tab** — a shell-owned id; holds the current document, its origin, its
  open requests, crashed flag.
- **Document** — a random id minted by the bridge at document start; every
  request and every delivery names one.
- **Site** — an origin with an optional grant (`vela.perm.<origin>`) and a
  chain (`vela.chain.<origin>`).
- **Open request** — (tab, document, id, kind: consent | read | signing |
  queued-signing | resolve-receipt).

## Success Criteria *(mandatory)*

- **SC-001**: The four hand-written routing tables become one core table
  plus one JS mirror pinned by a test; the three provider assemblers and
  three bridge strings become one core function. Kotlin + Swift + desktop
  browser-routing code shrinks by at least 600 lines net.
- **SC-002**: The core conformance suite covers every method in the table
  and every lifecycle transition in User Story 2 (≥ 60 tests), all green.
- **SC-003**: On the connected Android phone and iPhone, the test dApp's full
  button set (connect, chain, block, switch, add, sign, SIWE, SIWE-other,
  eth_sign, send dust, approve-unlimited) produces the expected outcome in the
  parallel space, and a public dApp (Uniswap) connects and reads.
- **SC-004**: Killing the renderer, navigating mid-sign, closing a tab
  mid-sign and a background-tab connect each leave the app running and every
  page promise settled — verified on both phones.
- **SC-005**: No regression: Android JVM suite (623 on main), iOS hermetic
  suite, desktop `cargo test`, web unit + extension e2e, core `cargo test` all
  green.

## Assumptions

- The WebView engines on the test phones support `DOCUMENT_START_SCRIPT` and
  `WEB_MESSAGE_LISTENER` (Android System WebView ≥ 90); a device without them
  gets the page with no provider and a logged reason, never a provider on the
  wrong frame.
- Desktop (wry) exposes no frame information; its bridge posts only from the
  top frame and the core treats its messages as main-frame from the webview's
  URL, which is what wry can vouch for.
- WalletConnect stays out of scope (D1 option c was not chosen).
