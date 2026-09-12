# Results — 044 Android dApp Browser and Signing

Per phase: what changed, what the tests prove, what the device showed
(`uiautomator` text, screenshot names in the session's scratchpad), and the
defects only the device could show.

## Baselines (T001)

| Artefact | At 043's tip (`0d4a7a9e`) |
| --- | --- |
| `libvela_core_uniffi.so` arm64-v8a | 16,187,032 bytes |
| Machines driven by Android | 16 of 26 |
| Android unit tests | 431, 0 failures |
| Xiaomi system WebView | `com.google.android.webview` 151.0.7922.200 (document-start scripts need ≥ 90) |
| Ceiling for the six machines (plan) | ≤ 19.5 MB arm64 |

## Phase log

### Phase 0 — the bridge grows six machines (T001–T009)

**What changed**: `bridge_object!` for `DappPermissionsCore`,
`ExploreSitesCore`, `BrowserHistoryCore`, `SignRequestCore`,
`ClearSigningCore`, `ApprovalGuardCore`; two rules exported so the shell
never re-types them: `dapp_origin_of` (the grant key and the one fact about
a page the shell attaches) and `dapp_is_signing_method` (the routing
table's first question). Six Kotlin wires (`DpermWire`, `ExploreWire`,
`BhistWire`, `SignWire`, `ClearWire`, `GuardWire`, 1,370 lines) with every
numeric type read from the Rust struct, not the mirror (`u32` → `Int`,
`f64` → `Double`, `i32` codes).

**Tests**: `CoreWireDriftTest` +5 cases over 6 families — 58 assertions
green (views subsets; operations, results, page events, payloads,
outcomes, closed reason/kind/surface unions exhaustive; event payload
fields present). `BridgeSmokeTest`: each machine created, first event,
view, no fault; the origin rule (`https://app.uniswap.org/swap?a=1#x` →
`https://app.uniswap.org`, `HTTPS://user@Example.com:443/` →
`https://example.com`, `about:blank` → none).

**Bridge size**: arm64 `.so` 16,187,032 → **18,892,528** (+2,705,496,
+2.58 MB) for six machines and two exports — under the 19.5 MB ceiling.

**Found while running**: the history machine records nothing before its
store has answered (`Start → ReadHistory → Loaded`), and `BhistView` has no
"ready" flag — a visit dispatched a millisecond after `Start` is dropped.
The controller must wait for the load (the executor's answer is the
signal) before recording the first visit; the smoke test does.

### Phase 1 — the engine and the provider (T010–T016)

**What changed**: `androidx.webkit`; a `syncVelaProviderAssets` Gradle
task copies `extension/inpage.js` and `extension/lib/protocol.js` from the
web tree into the APK (`assets/provider/`, 16,420 + 13,758 bytes) — one
script, not a fork; `ProviderBridge` strips the two module keywords at load
(the desktop's `provider_script()`), wraps the pair in one scope, adds the
desktop's `BRIDGE_JS` with `VelaHost.post` in place of `window.ipc` (top
frame only), installs both with `WebViewCompat.addDocumentStartJavaScript`
(fallback: `onPageStarted`, logged), and delivers answers through
`window.__velaDeliver`. `BrowserEngine` (one `WebView` per tab, the
shell's facts: URL, origin through the core's `dapp_origin_of`, host,
secure, back/forward) and a phase-1 `BrowserController` (open, close,
back, forward, the request sink, the open-from-outside seam). `DappRpc`
— the desktop's routing table with the core's `dapp_is_signing_method` as
its first question. A debug-only network security config permits
cleartext to the loopback address; `dev/testdapp/index.html` extends the
web's test dApp with Chain / Block number / Switch / Send dust / Approve
unlimited. `ExploreScreen` gained a page slot, an initial view, an
open-URL callback; the start page's search field opens a real site.

**Tests**: `DappRpcParityTest` — the three method sets parsed from
`protocol.js` equal the Kotlin table; routes fail closed (`eth_sign` and
`eth_signTransaction` unsupported, `personal_sign` and
`eth_signTypedData_v4` sign, bundler reads flagged); the switch parameter
reads hex and decimal.

**Device** (SC-001, `p44-1-provider.png`): `am start … --es vela.openUrl
http://127.0.0.1:8137/` (served over `adb reverse`) → 探索 opens on the
page with the address bar reading `127.0.0.1:8137`; the page's own state,
read through Chrome DevTools (debug builds expose the engine;
`devtools.sh` in the scratchpad evaluates an expression):
`announced: [{name: "Vela Wallet", rdns: "app.getvela",
sameAsWindowEthereum: true, frozenInfo: true}], legacy: {present: true,
isMetaMask: true, isVela: true}`; `browser.inject path=DocumentStart`
(WebView 151). The provider's own warm-up asked `eth_chainId` and
`eth_accounts` at load — unanswered until phase 3.

**Device-found**: a document-start script asks BEFORE `onPageStarted` has
told anyone the URL — the first cut attached an empty URL to those
requests (`url=`). The bridge now reads `WebView.url` on the UI thread at
the moment of each request. Also: `uiautomator` does not list the page's
text nodes, so the page's state is read through DevTools, not the dump.

