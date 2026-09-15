# Research — 053 iOS dApp Browser and Signing

Every decision here was checked against the repository before it was written.
Where a claim is about a file, the path is given so the next person can
disagree with the evidence rather than with me.

---

## D0 — This cut adds no Rust, again

All six machines are `bridge_object!`-exported
(`rust/crates/vela-core-uniffi/src/onboarding_bridge.rs`) **and already present
in the committed `vela_core_uniffi.swift`**: `ExploreSitesCore`,
`BrowserHistoryCore`, `DappPermissionsCore`, `SignRequestCore`,
`ClearSigningCore`, `ApprovalGuardCore`. Every free function the executors need
is there too — `dappOriginOf`, `dappIsSigningMethod`, `hashTypedData`,
`keccak256`, `safeMessageHash`, `eip1271Signature`, `multicall3EncodeAggregate3`
/ `…Decode…`, `erc20EncodeBalanceOf`, `decodeCalldata`, `matchSelector`,
`computeSelector`, `functionSelector`, `checksumAddress`, `toQuantity`.

**So `rust/scripts/build-ios-xcframework.sh` is not on this cut's critical
path.** If you find yourself re-running it, something was added that the spec
says is not needed. SC-014 turns that into a gate: a diff on the bindings at
closeout is the signal.

The one ABI helper Android has and the bindings do not export as a single
function is `allowance(owner,spender)`. It composes from
`functionSelector("allowance(address,address)") + abiEncodeAddress × 2`, which
is three existing exports and no new Rust.

---

## D1 — WKWebView, one per tab, and where the seam is

**The seam is exactly one view.** `ExploreScreen`'s `.browsing` arm draws
`ScrollView { DemoPageView(page:) }`
(`app-ios/VelaWallet/VelaWallet/Features/Explore/ExploreScreen.swift`), and
`DemoPageModel`'s own doc comment has said since spec 021 that "a real
WKWebView replaces this view wholesale". Everything around it —
`AddressBarView`, `BrowserToolbarView`, `ExploreTabsScreen`, the three sheets —
already takes display-ready models and fires closures. `BrowserModel` is
already the browser-chrome contract (`url, host, secure, connected, canBack,
canForward, bookmarked, account, tabCount, page`); only `page` goes.

**One `WKWebView` per open tab**, held by the controller, wrapped in a
`UIViewRepresentable`. A shared `WKProcessPool` and the default
`WKWebsiteDataStore` so a login on one tab is a login on the next — what a
browser means by "tabs".

`DemoPageModel` and `DemoPageView` **stay**, because E4/E6/E7 are gallery
states and a gallery that renders a live web view is not a gallery.

### The content world is `.page`, and this is not a detail

`WKUserScript(source:injectionTime:forMainFrameOnly:in:)` defaults to
`.defaultClient` — an isolated world. A `window.ethereum` defined there is
invisible to the page. Both scripts and the message handler are therefore
registered `in: .page`:

```swift
controller.addUserScript(WKUserScript(source: provider, injectionTime: .atDocumentStart,
                                      forMainFrameOnly: true, in: .page))
controller.add(handler, contentWorld: .page, name: "VelaHost")
```

and every reply is `evaluateJavaScript(_:in:contentWorld:)` with `.page`.

### `forMainFrameOnly` is belt, the JS guard is braces

Android passes `is_main_frame = true` by construction because the bridge posts
only when `window.top === window`. iOS keeps that guard **in the script**, and
additionally sets `forMainFrameOnly: true`. The guard is what the core's
invariant rests on; the flag is a second lock on the same door.

---

## D2 — The provider is copied, never ported

`extension/inpage.js` (434 lines) and `extension/lib/protocol.js` (354 lines)
are copied into the app bundle at build time by a new
`app-ios/scripts/bundle-provider.sh` + two `.xcfilelist`s, exactly as
`bundle-animations.sh` copies the launch animations and `bundle-catalogs.sh`
copies the locale catalogs. `ProviderBundleTests` compares the bundled bytes
with the web tree's through `#filePath` — the precedent is `ReceiveTests`.

`bundle-catalogs.sh` is **not** reusable as-is (its corpus rule pins an exact
locale count and an empty match aborts); the sibling script is ~40 lines.

At runtime the two are assembled into **one** classic script, the way Android
and the desktop assemble them:

- every line of `protocol.js` with a leading `export ` removed
- every line of `inpage.js` whose trimmed start is `import ` dropped
- the pair wrapped in a single `(() => { … })();`

One IIFE, so the constants reach the provider and nothing reaches the page.

---

## D3 — The origin is read at message time, from the web view

`WKScriptMessageHandler.userContentController(_:didReceive:)` runs on the main
thread. The handler reads `message.webView?.url`, passes it through
`dappOriginOf`, and discards the message when `message.frameInfo.isMainFrame`
is false.

**Never `message.frameInfo.securityOrigin`** as the primary: it is correct, but
`dappOriginOf` is the rule the grants were written with on three other clients,
and a grant keyed by a differently-spelled origin is a grant that silently does
not apply. `securityOrigin` is used only as the cross-check in a test.

**Never a navigation callback.** A document-start script asks before
`didStartProvisionalNavigation` has reported anything; Android found this on
the device, with an empty origin attached to the provider's own warm-up
`eth_chainId` and `eth_accounts`.

---

## D4 — `DappRpc.swift` is the allowlist, checked against `protocol.js`

A Swift port of Android's `DappRpc.kt`, which is a port of the desktop's
`executor/dapp_rpc.rs`. Six routes: `sign`, `state`, `switch`, `ack`,
`read(bundler:)`, `unsupported`. `eth_sign` is matched **before**
`dappIsSigningMethod`, because it is refused as policy.

Three sets, ported verbatim: `READ_ONLY_RPC_METHODS` (21),
`BUNDLER_METHODS` (5), `EXTRA_READ_METHODS` (9).
`eth_accounts` / `eth_requestAccounts` / `wallet_getPermissions` are
**deliberately absent** — the core answers those and they never reach the
router.

`DappRpcParityTest` reads `app-web/vela-wallet/extension/lib/protocol.js`
through `#filePath` and fails when the three sets differ. That test is the
reason this table may be duplicated at all.

**Recorded divergence, inherited**: the contract documents and Android's
research D5 both say `unsupported → 4200`; the shipped Kotlin answers **4900**,
and so does the desktop. iOS follows the code. 4900 rather than 4001 because
**the person did not decline** — 4001 is a lie about a human action.

---

## D5 — Two hosts, and the order they are told things

**`BrowserController`** is app-resident and owns `dapp_permissions`,
`explore_sites`, `browser_history`, the engines, the request sink and answer
routing. **`SigningController`** is per-request and owns `sign_request`,
`clear_signing`, `approval_guard` and a fee session; the three die together
when the sheet closes.

Seeding order is not a style choice:

```
AccountsUpdated → AccountSwitched → ChainChanged          (dapp_permissions)
NetworksChanged + AccountsChanged → then RequestArrived   (sign_request)
```

A `sign_request` that has not been told the network set refuses **every**
transaction with 4902, fail-closed and by design. Getting this order wrong
looks exactly like a broken chain.

`BrowserController` also re-tells the permissions machine the current URL
whenever the selected engine changes: the machine can be born after the page
loaded, and then it knows no origin. That was the desktop's second run-time
finding and Android's `reconcile`.

---

## D6 — One submit spine, already written

`UserOpSpine.submit` is 052's, unchanged: a dApp transaction is the same code
with `calls` taken from the page instead of from the send machine.
`UserOpSpine.signMessage` was ported in 052 alongside it **precisely so that
the two could not become two opinions**, and 053 is where it is first called.

`sign_and_submit` reports twice, as Android's does:

1. the accepted user-op hash mid-flight (`OpSubmitted`) — so the durable record
   precedes anything the dApp could poll; the executor then waits (bounded) for
   the record to be on disk before answering;
2. the final outcome, which is the **transaction hash** when the receipt
   arrives inside the wait and the user-op hash when it does not.

The page must never be blocked on a receipt: spec 028 found that blocking made
`eth_sendTransaction` time out with `-32603`. `eth_getTransactionReceipt` and
`eth_getTransactionByHash` for a user-op hash this wallet minted are translated
by the router.

---

## D7 — `check_bundler_funding` and `attempt_sponsorship` are answered, not implemented

`pre_check { funding: null }` — the core's own doc says a timed-out or errored
pre-check is **not** a refusal and the submit's underfunded answer is the
authority. `sponsorship { denied, reason: null }` — this shell reaches no
sponsorship path, and `funded` would be a claim that somebody else paid.

Desktop and Android answer identically. This is not a gap; it is the answer.

---

## D8 — The two records a dApp writes

Both go through `TxRecords`, the feed's own store, in **camelCase** (the feed's
row shape, distinct from `vela.accounts`' snake_case — 052's silent-failure
lesson).

| | `type` | shape |
|---|---|---|
| a transaction | `dapp_tx` | `to`/`value` from `params[0]`, native symbol, `signedRequest` clipped at 4,096 bytes with `requestTruncated` |
| a signature | `sign_message` / `sign_typed_data` | **no** value, **no** symbol — a signature moves nothing, and a row claiming a value shows up in the feed as money |
| a connection | `connect` | `id: dapp-<nowMs>-connect`, empty hashes, zero value, `timestamp` in **seconds** |

052's `load_send_history` narrows to `send` rows, so none of these can reach
the anti-poisoning trust signal. That was written before any of them existed,
for exactly this cut.

---

## D9 — Clear signing's four sources

| source | where |
|---|---|
| ERC-7730 descriptors | `HttpGet{path}` against the chain-data base the settings machine names, 6 s |
| 4-byte candidates | openchain, then 4byte, cached in the executor, 5 s |
| chain probes | `RpcEthCall` through `RpcPool` — and the **revert / unreachable** distinction preserved |
| the clock and the caps | `Now` and `Timer` |

`rpc_error: true` is "the node answered with an error object". The core reads
that as a contract that reverted; `result: null, rpc_error: false` is "nobody
answered". An executor that flattens them makes a reverting contract look like
a chain that is down, and the degradation ladder picks the wrong rung.

---

## D10 — Guard reads: one round trip for metadata

`read_token_metadata` is one Multicall3 `aggregate3` — `symbol()` and
`decimals()` per token, `allowFailure` per call so one reverting token does not
cost the others their answer. `iOS` already has `Core/Multicall.swift` over the
same uniffi encoder, so this is a caller, not a new primitive.

`metas: null` (whole read failed) and a token **missing from** a returned list
are different facts with different fallbacks. Answer `Some([])` for an empty
token list, never `null`.

---

## D11 — No new screens, and the one sheet that must move

Everything is drawn: E1–E7 for the browser, cs1–cs33 for signing,
`ConnectionPanelView` for consent, `AllowanceEditorView` for the ceiling,
`SlideToConfirmView` for the commitment, `TechDetailsView` for the fallback.

Two live builders do all of it:

- **`ExploreLive`** → `ExploreHomeModel` from `ExploreView` + `BhistView` +
  `DpermView` + the engine's own state.
- **`SigningLive`** → `SigningModel` from `SignView` + `ClearSigningView` +
  `GuardView` + `FeeView`, mirroring the desktop's `signing/live.rs`.

**The consent surface is `ConnectionPanelView` in its not-yet-connected form.**
The panel today is built by `ExploreFixtures.connection(_:)` and is reached
from E7 and the toolbar's account chip. It gains an approve/reject pair when
`DpermView.consent` is present.

**`SigningSheet` has no reject button — dismissal is rejection.** That is the
drawn design, and the core agrees: `SwipeDismissed` is routed **by phase** —
funding view → funding cancel; error/submitted/submitting → dismiss; otherwise
→ reject. So iOS must dispatch `SwipeDismissed` on the sheet's interactive
dismissal and must **not** dispatch `RejectTapped` there. Android recorded the
same distinction the other way round and got it wrong first.

---

## D12 — The device harness: a local server, not `file://`

`dappOriginOf` gives a `file://` URL no origin, so signing fails closed and the
harness would prove nothing. The test dApp is therefore served over loopback:
an `NWListener` on `127.0.0.1:8137` inside the UI-test runner, which on a
physical device shares loopback with the app.

`Info.plist` gains `NSAppTransportSecurity → NSAllowsLocalNetworking` (a
browser must be able to open an http page, and the harness is one). The page
itself is `app-android/vela-wallet/dev/testdapp/index.html`, copied to
`app-ios/VelaWallet/VelaWalletUITests/Resources/testdapp/` with two changes:

1. **keccak is vendored**, not fetched from a CDN. Android's phase 5 found the
   CDN script did not load on the phone and drove the on-chain check with a
   digest computed on the host. A harness that needs the internet to verify a
   signature is a harness that fails for the wrong reason.
2. every action prints a `#verdict <name> <json>` line into the page. XCUITest
   reads a `WKWebView`'s **rendered** text through the accessibility tree and
   can tap page buttons (`app.webViews.buttons[…]`), but it cannot read JS
   state that was never painted.

`Debug` only: `isInspectable = true`, so Safari ▸ 开发 is the phone's DevTools.

---

## D13 — The browser lives under the sheet, and stays inside 探索

The engine instance survives leaving the tab; only its presentation goes.
A `WKWebView` removed from the hierarchy keeps its process and its page, which
is what "the page kept running while I signed" means.

The signing sheet is a `.sheet` over the explore screen, `.presentationDetents
([.large])` — the shape `ExploreScreen` already has for `signingUp`. Its
interactive dismissal dispatches `SwipeDismissed` (D11).

Nothing of a page may be painted outside 探索. That is FR-013 on Android and it
matters more here, because a `UIViewRepresentable` that outlives its container
is the classic way to leak a web view onto the wrong screen.

---

## D14 — Words come from the corpus, and the corpus already has them

`explore.*`, `connect.*`, `clearSigning.*`, `componentsUi.signing.*`,
`componentsUi.signingApprove.*` are all present in `assets/i18n/*.json` —
`clearSigning`, `connect` and `signHandoff` are top-level namespaces the iOS
app has **never read**. Spec 021 and 023 drew these screens with their text.

Zero corpus change. The nine reject sentences are the shell's words for
`DpermRejectReason`, from `connect.*`.

---

## D15 — Verification order, and the gate

```
0  wires + drift + the provider bundle + the parity test   (no screen changes)
1  the engine: a real page loads and announces the provider
2  the browser remembers: favourites, groups, tabs, history
3  connect: consent, the instant answers, the page events
4  sign: a transaction lands and the page gets a tx hash    ← SC-005 gate
5  the guard, the message signature, the connection list
6  closeout on the device
```

**Phase 4 is the gate.** Nothing in phase 5 starts until dust has left the
golden Safe because a page asked for it. Android ordered its phases the same
way, for the same reason: everything after signing is easier to fix than
signing, and everything before it is worthless if signing does not work.

---

## The Android device-found defects this port inherits

Recorded here so they are cheap the second time.

1. **`browser_history` has no `ready` flag.** A visit dispatched before
   `read_history` answers is dropped. The executor signals the load; the
   controller records nothing before it.
2. **The URL race** (D3). The provider's own warm-up asks before any navigation
   callback fires.
3. **A deep-linked open races the document load.** Every intent waits for
   `ExploreView.ready`.
4. **The switcher's Done with one tab left drew the demo page** — a fixture on
   a live route. The browsing view must exist only while a page does.
5. **A sheet holding a snapshot** showed a new group only after reopening.
6. **A no-calldata native transfer drew the blind card** ("cannot decode — no
   descriptor, 0 bytes"). The core resolves it as send/amount/recipient with no
   descriptor at all; the live builder was reading the wrong field.
7. **The slide's verb printed the intent id** (`确认send`) instead of mapping it
   to a corpus key.
8. **The fee quote's TTL fired while the person read the approval**, shutting
   the slide with no way to reopen it. Re-quote when a quote goes stale under
   an open, idle sheet.
9. **A second request while one is open is refused `-32002`**, not queued —
   the permissions machine's `ConsentBusy` rule applied to signing.
10. **`app.webViews` text is only what was painted.** `#verdict` lines exist
    for this reason.
