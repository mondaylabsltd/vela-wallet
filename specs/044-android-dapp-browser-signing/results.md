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

### Phase 3 — connect (T023–T027)

**What changed**: `BrowserExecutor` — dperm's eight arms (grants under
`vela.perm.<origin>`; answers and events in the wire's shapes; the
desktop's words for the core's reasons; the "connected to" row in the
feed's shape; a forwarded request handed to `RequestRouter`).
`RequestRouter` — the desktop's `Route::State/Switch/Ack/Read/Unsupported`
arms: `eth_chainId` hex and `net_version` decimal from the browser's
chain, a switch only to a chain the settings machine has (else 4902), an
acknowledgement that changes nothing, reads through the person's pool
(`RpcKind.Bundler` for the 4337 methods), 4900 for the rest; a signature
goes to phase 4's controller (until then: 4900, never a silent wait).
`BrowserController` hosts `dapp_permissions`: seeded like the desktop
(`AccountsUpdated`, `AccountSwitched`, `ChainChanged(100)` — the session's
accounts, re-told on every change), `ProviderRequest` from the bridge with
the origin through the core's `dapp_origin_of`, `NavigationStarted` on
every load of the selected tab and on every tab switch (the desktop's
"born after the load" finding), answers routed back to the tab that
asked, open forwarded ids settled by a navigation. `ExploreLive` draws the
connected pill, the E7 connection sheet (the CORE's origin and address;
the wallet's own name) and the consent card (the sheet's not-yet-connected
form: `connect.browser.title/body/connect/cancel`).

**Tests**: `BrowserMachineTest` — consent carries the core's origin;
approval answers one address and writes the `connect` row; the grant is
keyed by origin, not URL; the second ask is instant; a read is forwarded,
never answered by the executor; revoke fires `disconnect` and re-asks; a
dismissal is 4001 once. The router: facts in both notations, a pool read,
a node error passed through, no endpoint → -32603, a switch to a known
chain (and 4902 for an unknown one), an acknowledgement, 4900 for
`eth_signTransaction`, a signature forwarded. Suite: 448, 0 failures.

**Device** (SC-002, SC-003, SC-007; `p44-3-consent.png`,
`p44-3-connected.png`, `p44-3-connection.png`), the page's own state read
through DevTools:
- Connect → the consent sheet: `1 | 127.0.0.1:8137 | 不安全站点 — 未加密 |
  取消 | Parallel space | 0x88cC…6894 | 切换账户 | 网络 | Gnosis |
  该网站想查看你的地址并请求你签名。未经你批准，它无法转移任何资金。 |
  连接 | 取消` → 连接 → `eth_requestAccounts: ["0x88cCA0…6894"]`, events
  `connect {chainId: "0x64"}`, `accountsChanged`.
- Second Connect: the same answer, no sheet.
- `eth_chainId → "0x64"`; `eth_blockNumber → "0x2df947f"` while the pool
  logged `rpc.post eth_blockNumber host=rpc.gnosischain.com outcome=ok`.
- `wallet_switchEthereumChain 0x1` → `null`, event `chainChanged "0x1"`,
  `eth_chainId → "0x1"`; the next consent card reads `网络 Ethereum`.
- The account pill → the connection sheet → 断开连接 → events
  `accountsChanged []`, `disconnect {code: 4900}`; Connect → consent again
  → 取消 → `{ok: false, code: 4001, message: "User rejected the request"}`,
  once.

### Phase 4 — sign, and it lands (T028–T036) — SC-304's answer

**What changed**: `UserOpSpine` — the submit spine extracted from
`SendExecutor.submitInner` (same order, same ports); a person's transfer
and a dApp's transaction are ONE implementation now (`SendMachineTest`
unchanged, green). `SignExecutor` (seven arms: the page answered through
the owning tab; the pre-check and sponsorship answered as the desktop
answers them; `SignAndSubmit` through the spine with `OpSubmitted`
mid-flight, the record awaited before the answer, and the desktop's
`await_receipt` — the page's `eth_sendTransaction` resolves to the
TRANSACTION hash, the op hash only when the receipt is late; records in
the feed's `dapp_tx` shape; the account switch acknowledged).
`ClearExecutor` (descriptors from the settings machine's data endpoint,
openchain → 4byte selectors cached, routed `eth_call` with the
answered/reverted/could-not-ask distinction, clock, timer). `GuardExecutor`
(metadata by `aggregate3`, allowance and balance reads).
`SigningController` — born per forwarded request, four hosts that die
together; `NetworksChanged` and `AccountsChanged` BEFORE `RequestArrived`;
`clear_kickoff` by method; the fee quoted for the request's calls; the
tracker handed the hash only once every record it names is on disk.
`SigningLive` — the drawn sheet from the four views: the dApp is the host,
the blocks the core's reading (a plain native transfer with no calldata
reads as Send with its amount and recipient — the core resolves it without
a descriptor; drawing the blind card for it read a dust send as a contract
interaction, device-found), the fee from the policy, the slide open only
when the request, the guard and the fee all say so, its verb the core's
intent in the corpus's words. `RequestRouter` translates
`eth_getTransactionReceipt` for a hash this browser answered a page with.
The sheet is raised by the container's `signing` state over whatever is
showing; dismissing it is the swipe the core routes (a reject before the
commitment point, a dismiss after).

**Tests**: `DappSignMachineTest` (debug set, fixture keyset): a page's
`eth_sendTransaction` → the sheet carries the core's origin and chain, the
reading resolves, the fee is ready, the gate opens → approve → one
signature, the record is on disk before the tracker is handed the hash and
before the page is answered, the page receives the TX hash once the
scripted receipt lands, the row flips to confirmed with it; a swipe before
commitment → 4001 once, nothing written, the controller closes.
`SigningLiveTest`: the plain transfer reads as Send/amount/recipient with
`confirmSend`; the slide waits for the guard and the fee; bytes stay blind.
Suite: 450, 0 failures (+ SigningLiveTest 2).

**Device** (SC-004; `p44-4-sheet-send.png`, `p44-4-submitted.png`,
`p44-4-answered.png`, `p44-4-home.png`): Send dust on the test dApp → the
sheet `1 | 127.0.0.1:8137 | Gnosis | 发送 | −0.001 xDAI | 接收方 |
0x7687…D141 | 技术细节 | 网络费 ~0.01 xDAI | 签名账户 Parallel space |
滑动以确认 · 确认发送` → slide (the parallel space signs) → `已提交 —
等待链上确认` → the page prints `eth_sendTransaction: {ok: true, result:
"0x0698bf846a…8fe3"}` — the TX hash, after the receipt; the tracker
patched the row (`tracker.patch confirmed ids=1 tx=0x0698bf846a`) and the
home feed lists it. The first pass (`p44-4-sheet.png`) had read the same
transfer as `合约交互 | 无法解码 — 无 ERC-7730 描述符（0 字节）` — fixed
before this pass; its transfer `0x0fbee0f5…4429` also landed.

**Device-found defects, fixed**: the blind card for a no-calldata transfer;
the slide's verb printed the intent id (`确认send`) — mapped to
`confirmSend`; `DismissTapped` is not the refusal (the core's dismiss is
silent for a committed op) — the sheet's close is the swipe, which the core
routes; and, from the test: JUnit4 refuses a test method that returns
`runBlocking`'s value (`runBlocking<Unit>`), and a controller that forces
`Dispatchers.Main` cannot be driven on the JVM.

