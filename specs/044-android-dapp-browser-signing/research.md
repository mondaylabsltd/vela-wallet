# Research — 044 Android dApp Browser and Signing

Every decision below was checked against the tree, not remembered.

## D1 — Six machines, not eight

**Decision**: bridge `dapp_permissions`, `explore_sites`, `browser_history`,
`sign_request`, `clear_signing`, `approval_guard`. Leave `dapp_session`
and `ext_cache` out.
**Rationale**: the desktop's in-app browser composes exactly these six
(`wallet/browser_host.rs` owns dperm; `wallet/signing_host.rs` owns
sign+clear+guard+fee; `executor/explore_sites.rs`, `browser_history.rs`).
`dapp_session` is the live TRANSPORT session machine (WalletPair / remote
inject — 027 research D43; WalletPair is out by the founder's ruling);
`ext_cache` is the extension service worker's snapshot (`lib/dapp/core/
ext-cache.ts`), whose job the dperm grant mirror does in-process.
**Alternatives**: bridging all eight "for completeness" — two unused
objects cost ~350 KB each and a drift-gate surface nobody drives.

## D2 — The system WebView, the shared provider, the desktop's bridge shape

**Decision**: `android.webkit.WebView` inside an `AndroidView`, JavaScript
on, one instance per open tab kept in the browser controller (tabs are
`explore_sites`' rows; the engine instances are shell state). The provider
is `extension/lib/protocol.js` + `extension/inpage.js` wrapped exactly as
the desktop's `provider_script()` wraps them (one IIFE so the constants
reach the provider and nothing on the page), followed by a bridge script
that is the desktop's `BRIDGE_JS` with `window.ipc.postMessage` replaced
by `VelaHost.post` (a `@JavascriptInterface`) and `window.__velaDeliver`
kept verbatim. Injected with `WebViewCompat.addDocumentStartJavaScript`
(feature `DOCUMENT_START_SCRIPT`, allowed-origin rules `*`), which is the
Android equivalent of wry's `with_initialization_script`; when the feature
is absent the scripts go in at `onPageStarted` and the fact is logged and
recorded in results.
**Rationale**: FR-002 (same script) and FR-013 (the page runs the person's
own engine). The two Gradle-synced files are read from the repo at build
time like the i18n corpus already is (`syncVelaI18nAssets`).
**Alternatives**: GeckoView (a second engine, +60 MB); `react-native-webview`
(gone with 039).

## D3 — Origin and main-frame are the shell's facts, computed by the core

**Decision**: the origin handed to `ProviderRequest` is `origin_of(webView.url)`
— the core's own rule, exported through uniffi as `dapp_origin_of` in phase
0 — never anything the envelope says. `is_main_frame` is true by
construction: the bridge posts only when `window.top === window`, and the
document-start script's allowed-origin rules apply to every frame, so the
check is in the script and stated there.
**Rationale**: the desktop's run-time finding ("grants were stored under a
full URL with path and query" — 032 phase 27) and its fix (call the core's
`origin_of`). A second origin rule in Kotlin would be a second spelling.

## D4 — Who answers: `DappRpc.kt`, tested against `protocol.js`

**Decision**: a Kotlin port of the desktop's `executor/dapp_rpc.rs`
(`Route.Sign / State / Switch / Ack / Read{bundler} / Unsupported`,
`READ_ONLY_RPC_METHODS`, `BUNDLER_METHODS`, `EXTRA_READ_METHODS`), with a
JVM test that reads `app-web/vela-wallet/extension/lib/protocol.js` from
the repo root (the tests already receive `vela.repo.root`) and fails if
the sets differ.
**Rationale**: allowlist routing fails closed (an unknown signing method
must never fall into a read bucket that proxies it to a public node —
"a wallet that is an open RPC relay with a wallet's name"); the parity test
is what keeps two tables one.

## D5 — Reads and wallet facts

**Decision**: `Route.Read` → `RpcPool.call(chainId, method, params, kind =
if bundler Bundler else Rpc)` on the page's current chain; `Route.State`
answered from the dperm view's current chain (`eth_chainId` hex,
`net_version` decimal); `Route.Switch` → the settings machine's rows say
whether the chain is known → `DpermEvent.ChainChanged` (page hears
`chainChanged`) or the core's 4902 through `SettleForwarded`; `Route.Ack` →
the acknowledgement the desktop returns; `Route.Unsupported` → 4200.
**Rationale**: FR-005; the pool's bans and cooldowns are the person's.

## D6 — Two hosts, shaped like 043's controllers

**Decision**: `BrowserController` (app-resident: dperm + explore + bhist
hosts, the request sink the WebView bridge feeds, `NavigationStarted` on
every document load, `AccountsUpdated`/`AccountSwitched`/`ChainChanged` from
the session and settings machines, and — the desktop's second run-time
finding — the current document's URL told to a freshly born dperm host).
`SigningController` born per forwarded request (sign + clear + guard +
fee hosts that die together, the desktop's `SigningHost` shape: "a request
that ended must not leave a decoded intent or a half-edited allowance
behind"), fed `NetworksChanged` and `AccountsChanged` BEFORE
`RequestArrived` (the desktop's first run-time finding: an untold machine
refuses every transaction with 4902).
**Rationale**: the desktop found all three by running; the tests could not.

## D7 — One submit spine

**Decision**: extract `SendExecutor.submitInner`'s order (isDeployed →
nonce → floors → placeholder fee leg → estimate → apply → replace calls →
SafeOp hash → assertion → envelope → `eth_sendUserOperation`) into
`UserOpSpine` with the same ports; `SendExecutor` and the new
`SignExecutor.SignAndSubmit` both call it. `SignAndSubmit` reports twice
like the desktop: the accepted user-op hash mid-flight (`OpSubmitted`) so
the durable record precedes anything the dApp can poll, then the receipt's
transaction hash as the final outcome (a dApp's `eth_sendTransaction`
resolves to a TX hash; a user-op hash looked up as one is looked up
forever).
**Rationale**: the desktop's executor doc: "a dApp's transaction and a
person's own transfer must be assembled, priced and signed by one
implementation, or the sheet that shows what will happen and the code that
makes it happen are two different opinions."

## D8 — Records and the tracker

**Decision**: `PersistRecord` writes the feed's row shape with
`type: "dapp_tx"` (the tracker's `pendingRecords` already admits
`dapp_tx`), `UpdateRecord` patches status/txHash, `SaveConnectionRecord`
writes the desktop's `type: "connect"` row (`id: dapp-<now>-connect`, empty
hashes, zero value). The tracker follows a dApp's operation exactly as a
send's (043 phase 4).

## D9 — Clear signing's sources

**Decision**: `HttpGet{path}` → descriptors over `VelaHttp` from the same
base the desktop's `descriptor(path)` uses; `SelectorDbLookup` →
openchain then 4byte (the desktop's order, with its cache);
`RpcEthCall` → the pool; `Now`/`Timer` → the clock. `ResolveTransaction`
/ `ResolveTypedData` / `MessagePresented` are the kickoff the desktop's
`clear_kickoff` chooses from the method.

## D10 — The guard's reads

**Decision**: `ReadTokenMetadata` → one Multicall3 `aggregate3` per token
list through the pool (`Abi.encodeName/Symbol/Decimals`, 043's
`MtokExecutor` already does this shape); `ReadErc20Allowance` /
`ReadErc20Balance` → single `eth_call`s.

## D11 — The drawn surfaces gain live models, no new screens

**Decision**: `ExploreLive` builds `ExploreScreenModel` from `ExploreView` +
`BhistView` + the browser controller's tab engines (title, favicon, URL,
secure, canBack/canForward, connected); the consent surface is the drawn
connection sheet (E7) in its not-yet-connected form gaining approve/reject
callbacks; `SigningLive` mirrors the desktop's `signing/live.rs`
(`blocks`, `fee_model`, `guard_editor`, `status_blocks`, `funding_blocks`,
`confirm_enabled`, `confirm_label`, `dapp_identity` — the host itself as the
name: "guessing a pretty name from a domain is exactly the counterfeit
route") onto `SigningScreenModel`. The 022 drawings keep their fixture
builders for the gallery.

## D12 — The device harness

**Decision**: a debug-only network security config permits cleartext to
`127.0.0.1` and `localhost` only; `adb reverse tcp:8137 tcp:8137`; a page
at `app-android/vela-wallet/dev/testdapp/index.html` extends the web's
`e2e/testdapp/index.html` (Connect, Sign) with "Send dust" (a native
transfer to the founder's address) and "Approve unlimited" (an ERC-20
`approve(spender, 2^256-1)` on Gnosis USDC) buttons, printing every answer.
The public-dApp proof is a connect only (no money through a third party's
UI in a scripted pass).

## D13 — Backgrounding and the sheet

**Decision**: the WebView is paused/resumed with the activity; the
controllers are app-resident so a pending request waits within the
process; the signing sheet is a `ModalBottomSheet` over the page
(dismiss = `SwipeDismissed`, system back = `DismissTapped` — one refusal);
leaving 探索 hides the tab's engine (the `AndroidView` leaves composition;
the instance stays in the controller).

## D14 — Words

**Decision**: `explore.*`, `connect.*`, `clearSigning.*` are in the corpus;
the drawn screens already read them; live builders read the same keys.
Rejection messages: the shell's words for the core's `DpermRejectReason`
(the desktop's `reject_message`), from `connect.*`.

## D15 — Verification order

**Decision**: phase 0 (bridge + wires + drift) → phase 1 (WebView +
provider announces on device) → phase 2 (browse: explore/bhist live,
persisted) → phase 3 (connect: consent, instant answers, reads, switch,
events; test dApp + public dApp on device) → phase 4 (sign: spine
extracted, sheet live, dust from the test dApp on device — SC-004 gate) →
phase 5 (guard, message sign-in, connections/revoke) → closeout. SC-004 is
proven before phase 5 starts.
