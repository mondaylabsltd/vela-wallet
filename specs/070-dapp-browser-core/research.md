# Research — 070 dApp browser core

## R1. A new machine, not a widened `dapp_permissions`

**Decision**: add `vela_core::app::dapp_browser` (tab-aware) and a pure
`vela_core::app::dapp_rpc`; keep `dapp_permissions` for its pure policy
functions (`decide_browser_request`, `resolve_granted`, `should_drop_grant`,
`origin_of`, `is_insecure_http`) and the web popup's `PopupRequest`. Once the
three native shells run on `dapp_browser`, the browser half of
`dapp_permissions` (its `ProviderRequest` / navigation / consent state) has no
caller and is deleted in the same feature.

**Why**: `dapp_permissions` models ONE document (`current_origin`,
`connected_addr`, one `attempt`). Every tab bug in the audits comes from two
shells sharing that one-document core across many tabs. Widening its events
with `tab` would break the web popup's pure entry for nothing; a new machine
reuses the policy verbatim and owns the lifecycle.

**Rejected**: keeping bookkeeping in shells and only moving the routing
table — it removes one of the five duplicated pieces and none of the tab
defects.

## R2. Documents, not navigation events, decide what is stale

**Decision**: the bridge mints a random document id at document start and
posts `{t:"hello", doc}` before the page's own scripts run; every request
carries `doc`; every delivery names `doc` and the bridge drops one addressed
to another document.

The core settles a tab's open requests when:

1. a `hello` from a NEW document arrives (the old document is gone — its
   requests get 4900), or
2. the shell reports the load FINISHED (or failed) with no `hello` since the
   navigation started (the new document has no provider — error page,
   non-injectable page — so the old document is gone too), or
3. the tab closed or its renderer died.

`NavigationStarted` itself only records that a load began.

**Why**: on Android the new document's first messages can reach the shell
before `onPageStarted` (finding recorded in spec 044 research); settling on
the navigation callback therefore settles the NEW document's warm-up
requests (`eth_chainId`, wagmi's reconnect) with 4900 — a page that looks
broken on first load. Messages from one document arrive in order, and
`hello` is the first, so "a new document exists" is a fact the core learns
in order. The per-document id also makes delivery exact without any
platform-specific reply channel.

**Rejected**: counting navigations against hellos — drifts permanently after
one navigation to a page without a provider.

## R3. The provider moves into the core crate

**Decision**: `app-web/vela-wallet/extension/inpage.js` moves to
`rust/crates/vela-core/provider/inpage.js` as a classic script (its six
imports from `protocol.js` — `CHANNEL, RDNS, WALLET_NAME, ERR, rpcError,
toHexChainId` — are declared inside it). The core embeds it with
`include_str!` and returns the full injected script from
`dapp_browser::provider_script(host)`; the extension's esbuild entry points
at the same file. A web unit test pins the six constants against
`extension/lib/protocol.js`, which keeps the service worker's own copies.

**Why**: the owner's "注入到网页内容 … 应该都是一样的". Today the file is
already shared by copy; with the core as its home, the three native shells
stop copying and stop stripping module syntax (three implementations of the
same line filter), and the extension builds from the same bytes.

## R4. Error codes

| Situation | Code | Source |
|---|---|---|
| `eth_sign`, unknown method | 4200 Unsupported method (EIP-1193) | extension; 044 contract `shell-operations.md:16`; 4900 means "disconnected" |
| user rejected consent / signature | 4001 | unchanged |
| document gone, browser closed, renderer gone | 4900 | unchanged — never 4001 (retry = double spend) |
| unconnected / wrong frame / insecure origin | 4100 | unchanged |
| switch/add to a chain the wallet lacks | 4902 | extension |
| bad chain param | -32602 | unchanged |
| malformed request with an id | -32600 (shape) / -32602 (too large, duplicate id) | extension `isWellFormedRequest` |
| read queue full | -32005 | F06 |
| no endpoint answered | -32603 | unchanged |

## R5. Default chain

**Decision [D]**: 1 (Ethereum), the extension's `DEFAULT_EXT_CHAIN_ID`, for a
site with no stored chain and no grant. The native shells used Gnosis (100)
because the parallel-space fixture lives there; the test dApp has a "Switch to
Gnosis" button, so the harness is unaffected, and a dApp expects to start on
mainnet.

## R6. Search

**Decision [D]**: input that is not a URL (`dapp_rpc::browser_input`: has no
scheme, no dot, or has whitespace) becomes
`https://duckduckgo.com/?q=<encoded>`. DuckDuckGo stores no search history
per its policy, matching A03 (no tracking). A bare host with a dot (`app.uniswap.org`)
gets `https://`. `localhost[:port]` and IPs get `http://` only when they are
loopback (dev), else `https://`.

## R7. Signing serialisation

**Decision**: the core queues sign requests (FIFO) instead of answering
-32002. A shell that cannot open its sheet (the wallet's own signing is up)
answers the forwarded request with -32002 through `SigningAnswered`, and the
core moves on. Desktop's silent drop disappears because the shell never sees
a second forward while one is open.

## R8. What stays in the shells

WebView lifecycle, posting strings, JS dialogs, file choosers, external
schemes, the RPC/bundler/relay executors, the key-value store, and drawing.
The executor contract is in `contracts/dapp-browser.md`; every operation has a
neutral answer so an escaped failure still settles the page.
