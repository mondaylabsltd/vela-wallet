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

### Phase 2 — browse with a memory (T017–T022)

**What changed**: `ExploreExecutor` / `BhistExecutor` (`vela.explore`,
`vela.browserHistory`, the desktop's keys, the core's serde shape through
`Wire.json`); `BrowserController` now hosts `explore_sites` and
`browser_history` and owns one engine per tab with a URL (`reconcile` on
every view: engines for closed tabs die, the selected tab's is created and
loaded once); every intent waits for `ready` — the machine drops a
mutation before its document has loaded, and the first visit waits for the
history executor's answer (phase 0's finding). `ExploreLive.home` builds
the drawn model from `ExploreView` + `BhistView` + the engine's state: tiles
(a site's id is the URL it opens), the recent group (title over the host it
actually is), custom groups (hidden ones off the page), tabs with the
core's selection, the group-manage rows with the core's hidden flags, the
bookmarked star, the tab count. `ExploreScreen` gained `ExploreCallbacks`
(tiles, rows, tabs, groups, site menu, star, clear) and the keyboard's Go
submits the address; the toolbar's back/forward/star are wired.

**Tests**: `ExploreMachineTest` — favourites deduped by origin with the
URL refreshed and the name kept, a group with a member, two tabs with the
core's selection rule, all read back by a second host over the same store;
recents one row per origin with the latest URL, an untitled page named by
its host, cleared and persisted. `ExploreLiveTest` — tiles/groups/recents/
tabs from a view, hidden system groups off the page, the empty start page,
stable letter/colour per host. Suite: 446, 0 failures.

**Device** (SC-008; `p44-2-start-populated.png`, `p44-2-groups-sheet.png`,
`p44-2-start-with-group.png`, `p44-2-after-restart.png`,
`p44-2-wallet-tab.png`, `p44-2-page-again.png`):
- 添加到收藏 on the page → the start page reads `收藏 | 1 | Vela test dApp
  (Android) | 添加 | 最近的 dApp | 清空 | Vela test dApp (Android) |
  127.0.0.1:8137`; the switcher's 新建标签页 → count `2`; a second address
  typed and submitted with the keyboard's Go opened in the start tab;
  分组管理 → 新建分组 → a `新建分组 ⋯` section on the start page.
- `am force-stop` → reopen → 探索: the selected tab's page is back where it
  was (`关闭网页 | 127.0.0.1:8137 | … | 添加到收藏 | 1`); the store holds
  `"favorites":[{"origin":"http://127.0.0.1:8137",…,"name":"Vela test dApp
  (Android)"…}]`, `"last_visited_ms":…`, `"groups":[{"id":"g-1789187658247"…`.
- The wallet tab while a page lives: a screenshot with no page pixels; back
  in 探索 the start tab is selected, the switcher lists the page's tab, and
  its card returns to the page.

**Device-found defects, fixed**:
1. An open from a deep link raced the document load: the machine dropped
   `TabOpened` and no page appeared. Every intent now waits for `ready`.
2. The switcher's Done, with only the start tab left, drew the DEMO page —
   a fixture on a live route. The browsing view now exists only while a
   page does.
3. A new group showed only after reopening the manage sheet: the sheet held
   a snapshot. Live, it reads the model's rows.
4. Not a defect, recorded for the loop: `uiautomator` lists the switcher's
   close buttons as clickables beside the cards; a tap aimed by bounds must
   pick the card (`android.view.View`, x < 100), or it closes the tab.

