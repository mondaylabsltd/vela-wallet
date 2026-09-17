# Results — Arc Network Integration

**Branch**: `060-arc-network-integration` · **Date**: 2026-09-17

What shipped, what was verified, and — the part that matters most in a money change — what was deliberately *not* done and why.

## What shipped

### Chain data (`ethereum-data`)

- `chains/eip155-5042.json` authored — Arc mainnet, USDC/18 native, empty `stables`, no `dex`, no `wrappedNativeToken`, RIP-7212 marker, Safe proxy factory. Endpoints verified live before being written.
- `chains/eip155-5042002.json` **left alone**: the corpus already carried a curated Arc testnet entry with `contracts.safeProxyFactory`. An earlier pass in this session overwrote it with `arc.io` spellings; that was reverted. Both domains answer (`rpc.testnet.arc.network` returns `0x4cef52`), and rewriting an upstream-synced file to a second valid spelling would have cost the QuickNode and Blockdaemon endpoints it lists for nothing.
- `chainlogos/eip155-5042.png` / `…-5042002.png` were **already present** in the working tree (400×400 Arc arch mark, untracked). Not regenerated.
- `wrangler.toml` — comment only, recording which hostnames point where. No `routes` added: both custom domains are bound out of band, and declaring them would put wrangler in charge of bindings it did not create.
- Arc's RPC list carries four endpoints, each verified against the exact `aggregate3` call the relay makes: `rpc.mainnet.arc.io` (+ its WSS), `arc.drpc.org`, `arc-rpc.publicnode.com`. A single-endpoint chain is a single point of failure.
- Deployed. `https://ethereum-data.getvela.app/chains/eip155-5042.json` serves Arc; `/api/health` reports `ethereum-data`; the existing corpus still serves.
- **Build churn reverted.** `bun run build` regenerates the ERC-7730 registry from upstream, which produced a ~647k-line diff across 191 tracked files plus 138 new ones — nothing to do with Arc. Those were reverted and removed, only `generate-index` was re-run, and the service was redeployed so production matches the repo. `index.html` keeps its regenerated chain/asset counts.

### Relay (`vela-relay`)

- Chain-directory host → `https://ethereum-data.getvela.app` in both shells (`src/utils/rpc.rs`, `vela-relay-cf/src/arms/market.rs`) and in `src/app/rpc/handlers/README.md`.
- The fallback-RPC source label `"awesometools"` → `"chain-directory"`; it names a source in logs, and the source moved.
- **Arc mainnet added to the relay's Alchemy network map.** It carried `arc-testnet` (5042002) but not `arc` (5042) — see the diagnosis below; this is what made mainnet quotes fail while testnet quotes worked.
- **Arc's native coin pegged at $1** in `settlement::pegged_native_usd_price`, alongside Gnosis's xDAI, and both shells' `native_usd_price` now consult that one predicate instead of asking "is this Gnosis". This is load-bearing rather than cosmetic: the client's fee floor is "$0.01 worth of the native coin" and it reads that price **out of the relay's quote**, so a Binance outage would otherwise degrade Arc's floor to a tenth of a cent. A dead `pub use is_gnosis_chain` re-export went with it, and its test was extended to cover every dollar-native chain.
- `is_tempo_chain` is `{4217, 42431}`, so Arc takes the ordinary native path; the admission floor reads `native_decimals` from the chain directory rather than assuming 18. No other Arc-specific relay code was needed.
- Deployed to Cloudflare. `https://vela-relay-cf.getvela.app` reports service `vela-relay`, serves the Arc lane (EntryPoint v0.7), and quotes Arc mainnet **5/5** with `symbol: USDC`, `decimals: 18`, `usdPrice: 1`.

### Wallet (`vela-wallet`)

| Change | Where |
|---|---|
| Arc row in the canonical chain table (13 chains) | `network_admin.rs` `BUILTIN_CHAINS` |
| Provider slugs — Alchemy `arc-mainnet`, dRPC `arc`, Ankr none | `network_admin.rs`, `rpc-providers.ts`, `ChainCatalog.swift` |
| Built-in endpoints → `ethereum-data.getvela.app` / `vela-relay-cf.getvela.app` | core + all four shells (13 sites) |
| `pegged_native_usd(symbol)` — the $1 peg, once | `balance_dashboard.rs`, exported through wasm + uniffi |
| The four hard-coded `"USD" ⇒ $1` literals now call the core | web, iOS, Android, desktop |
| `min_gas_price_wei(chain_id)` — Arc's 20 gwei floor | `fee_policy.rs`, applied inside `derive_chain_gas_price` **and** the static fallback |
| `GasSignals` carries `chain_id` | so a caller cannot forget a floor it never has to remember |
| `native_alias_token(chain_id)` — the add-token refusal | `manage_tokens.rs`, with `native_alias` on the view |
| The refusal's words, 15 locales | `addToken.nativeAliasTitle` / `…Message`, rendered on all four shells |
| Arc chain row per shell | `chains.ts`, `ChainCatalog.swift`, `SendLive.kt` symbol map, desktop `chain_tokens.rs` |
| README: 12 → 13 networks | `README.md` |

## Verification

| Gate | Result |
|---|---|
| Arc admission probe (11 contracts + RIP-7212), mainnet and testnet | ✅ all present, both networks |
| `cargo test -p vela-core --features crux,i18n-all` | ✅ green (incl. the new peg, gas-floor and alias tests) |
| Twelve-chain gas-price parity | ✅ `no_other_chain_gained_a_gas_floor` — every pre-existing chain bit-identical |
| Desktop `cargo test` | ✅ 395 passed |
| Web `vitest` (992 tests) | ✅ except 3 failures that also fail on clean `HEAD` (explore fixtures, i18n FLOW_KEYS scan, signing catalogue) — pre-existing, unrelated |
| Web `npm run check` (svelte-check, 1392 files) | ✅ 0 errors |
| Web `eslint` / `prettier` | ✅ no new findings (the 8 prettier warnings are the pre-existing set) |
| Android `testDebugUnitTest` | ✅ BUILD SUCCESSFUL |
| iOS `xcodebuild build` (simulator) | ✅ BUILD SUCCEEDED, after regenerating the XCFramework |
| Cross-shell parity gates (×4 scripts) | ✅ exit 0 |
| i18n corpus lint | ✅ no new defects |
| `ethereum-data.getvela.app` live checks | ✅ Arc served, corpus intact, health identity unchanged |
| `vela-relay-cf.getvela.app` live checks | ✅ health, Arc lane, EntryPoint, Arc mainnet quote 5/5 at `usdPrice: 1` |
| Relay non-regression sweep (1, 100, 4217, 8453, 42161, 5042002) | ✅ all quote; Gnosis still pegged, Tempo still pathUSD |
| `cargo test -p vela-relay-core` + relay workspace | ✅ 137 + 80 passed |

### The one real bug this found, and how

Arc **mainnet** quotes failed with "in-band gas quote is temporarily unavailable" while Arc **testnet** and all twelve existing chains worked. Two wrong theories were tested and discarded before the right one:

1. *"Stale edge cache"* — plausible (the `_headers` file caches `/chains/*` for an hour, and that path served the SPA shell before the Arc file existed), but refuted: a success would have written the metadata into the Worker's KV for an hour, and failures kept coming.
2. *"Arc's public RPC throttles Workers"* — also plausible, and it did surface something worth fixing (Arc's chain file listed a single endpoint; `arc.drpc.org` and `arc-rpc.publicnode.com` were verified and added), but adding them did not fix it.

`wrangler tail` settled it: `in-band gas quote Multicall request failed: 5042`. The metadata was fine; the **RPC call** was failing — and the relay tries its Alchemy key *before* the directory's public endpoints. Its Alchemy map had `arc-testnet` (5042002) and **no entry for Arc mainnet**, so testnet went through Alchemy and worked while mainnet fell through to a public endpoint the Worker could not reach reliably. One map entry (`arc` → `arc-mainnet.g.alchemy.com`, the subdomain verified live) fixed it: 0/6 before, 5/5 after.

Worth stating plainly: the symptom pointed at chain data, the first two hypotheses were reasonable, and both were wrong. Only the log said what actually failed.

## The browser session — what only a real app could find

Everything above was verified without a browser. Then the feature was driven end to end in **parallel space** (the real app with a fixture passkey) on Arc testnet, funded with 20 USDC from `faucet.circle.com`. Four defects surfaced that no unit test, `curl` check or native build had caught.

### 1. The Cloudflare relay sent no CORS headers — every chain, not just Arc

The wallet is a browser app; every relay call is cross-origin. The docker shell has `CorsLayer::permissive()`; the Cloudflare shell had nothing, so Chrome refused every response before the app saw it:

```
Access to fetch at 'https://vela-relay-cf.getvela.app/5042002' from origin
'http://localhost:5173' has been blocked by CORS policy: No
'Access-Control-Allow-Origin' header is present on the requested resource.
```

`curl`, the Rust tests and the native shells — none of which enforce CORS — all saw a perfectly healthy service. **This would have broken sending on all thirteen networks the moment the built-in bundler moved to the Worker**, which is exactly what this feature does. Fixed in `vela-relay-cf/src/shell.rs`: permissive headers on every response plus a 204 preflight branch. Verified by an actual send afterwards (below).

An earlier line in this document claimed Slice A was verified; that claim was wrong, and it was wrong because the verification never used a browser.

### 2. The Cloudflare relay had no `/v1/treasury` routes — so the wallet could not warn

Raised by the requester while the test was running: *if the relay's treasury is empty on a chain, tell the person instead of leaving them waiting.*

The wallet already does exactly that — `probeTreasury` routes `uncovered` / `low-float` / `covered` / `unknown`, and the send flow raises **"Start this network's relayer"** with the treasury address, a QR, the suggested amount and the non-refundable warning. It stayed silent only because the Worker answered 404 to the probe that asks the question. Ported `GET /v1/treasury` and `GET /v1/treasury/{chain_id}`, with the pure parts (`NATIVE_TREASURY_FLOOR`, `parse_quantity`, `quantity_is_below`) moved into `vela_relay_core::treasury` so both shells report the same floor from one implementation.

The result, verified in the app: on Arc testnet the relay now answers `bootstrapNeeded: true`, and the wallet raises the funding dialog **before** anyone signs. Before the fix, the same send sat on "Submitted to the network · Waiting for blockchain confirmation…" for an operation that could never land — precisely the failure the requester predicted.

`GET /v1/account/{chain}/{safe}` is still missing from the Worker. It is informational (`fetchBundlerAccountInfo` is called under `.catch(() => null)`), so it degrades quietly — but it is a parity gap and belongs on the list.

### 3. The add-token refusal was running against a stale core

The app offered to add `0x3600…0000` as "USDC · Decimals 6 · Arc". The rule was correct; the wasm bundle in `rust/pkg-web` had been built *before* the rule existed. Rebuilt, and the refusal renders as designed: **"Already Yours — This is the network's native coin. Its balance is already shown in your assets."** with the CTA disabled.

### 4. The refusal could re-probe forever

`+page.svelte` treats a probe as settled when it produced a card or a miss. A refusal is neither, so the effect re-dispatched `detect_requested` endlessly. Guard extended to include `native_alias`.

### What the end-to-end run proved

| Check | Result |
|---|---|
| Arc mainnet in the built-in network list | ✅ visible, with its logo from the relocated host |
| Arc testnet through "Add Network" | ✅ **Compatible** — all 12 checks green in the UI |
| Relocated service endpoints in Settings | ✅ `ethereum-data.getvela.app` · `vela-relay-cf.getvela.app` |
| 20 USDC from the Circle faucet | ✅ landed on the fixture Safe |
| One coin, one row | ✅ **one** USDC row at 20 / $20.00 — the ERC-20 mirror reads the same 20 on-chain, so a naive listing would have shown $40 |
| Native price | ✅ $1.00 from the core peg (no DEX, no Chainlink on Arc) |
| Add-token refusal | ✅ refused with its own words |
| Arc fee quote | ✅ 0.166496 USDC for a first send (includes Safe deployment) |
| Empty-treasury warning | ✅ raised before signing, with the address and amount |
| **A real send through the CF relay** | ✅ on Gnosis: 0.01 xDAI moved, fee exactly **0.01 xDAI** — the universal $0.01 floor — and the balance fell by 0.02 on-chain |
| A real send on Arc testnet | ⏳ blocked only on funding the relay's Arc treasury |

## Round two — the fixes, and the Arc send completing

### `/v1/account` ported, with its rules shared

The Cloudflare shell now serves `GET /v1/account/{chain}/{safe}` as well, and both shells read their grammar and verdict from one place: `vela_relay_core::account` (`normalize_address`, `parse_nonce`, `entry_point_nonce_calldata`, `account_status`). Verified side by side — same nonces, same `ACTIVE`, same shape from either deployment.

### The out-of-gas sheet now says WHO can fix it

Raised by the requester: a network might be local, internal, or otherwise one the operator can never hold gas on — so let the person fund it themselves and say plainly that it is non-refundable; and where the developers *could* fix it, point them at reporting it instead.

The distinction the core already had the facts for is **built-in vs. added**, so that is what decides it (`network_admin::is_builtin_chain`, surfaced as `SendTreasuryStatus.operator_served`, filled by the core when it publishes the sheet — the shells report the probe unjudged):

| The network | What the sheet leads with | The funding half |
|---|---|---|
| One Vela ships | "Vela's operator runs this network's relayer… Telling them is the fastest fix — and it stays fixed for everyone", with a **Report this** link | folded away behind "Can't wait? Start it yourself" — it is the operator's bill, and nobody should be nudged into paying it |
| One the person added | "You added this network, so Vela's operator may have no way to hold gas on it — a local or private chain is nobody else's to fund. Starting its relayer is up to you." | open, because it is the only path there is |

The non-refundable warning, the treasury address and the QR are in both. The report link points at the app's existing GitHub issues destination — deliberately, rather than inventing a support URL; one constant (`RELAYER_REPORT_URL`) moves it to Telegram when that handle is confirmed. Copy in all 15 locales; both paths pinned by tests; live-verified on Arc testnet for the added-network case.

### The suggested top-up is 0.01 native coin, not 0.0001

Also raised by the requester. The relay's float floor is `0.0001`, and asking for exactly the shortfall to it buys a relayer that is back under the floor after roughly one operation — the same sheet, again. The suggestion is now `max(shortfall, 0.01)`. The relay's own floor is untouched: that is an operational threshold, not a number to put in front of a person.

### The Arc send, end to end

With the treasury funded (20 USDC, via the faucet's human captcha step), a real send completed on Arc testnet:

- **2.25 USDC** from Parallel One to Parallel Three, fee **0.166496 USDC**
- Parallel One 20 → **17.583503975** — exactly `2.25 + 0.166496`
- Parallel Three 0 → **2.25**
- **The Safe deployed on Arc in the same operation** (`eth_getCode` non-empty afterwards)
- Landed in about **10 seconds**; the wallet showed "Sent 2.25 USDC · Arc Network Testnet", tx `0x8095a9…a8f8a1`

That closes SC-001 for Arc: quote → passkey approval → submission → receipt, through the Cloudflare relay, on a chain whose gas coin is USDC.

**One thing to know about the first attempt.** The 1.5 USDC operation submitted while the treasury was still empty never landed and still reads *Pending*. It exhausted the queue's retries into the dead-letter queue, and because a UserOp hash is a function of its parameters, re-confirming the same send only re-attached to that dead record. The wallet has no story for "the relay gave up on this" — it is the requester's original complaint in its second form, and it is on the list below rather than fixed here.

## Deliberately not done

Two tasks from `tasks.md` were evaluated and left undone on purpose. Both would have been speculative code in a money path.

- **T043 — issuer-blocklist / destructed-account revert copy.** The web shell's revert mapper is a chain of English string matches. Adding a case for Circle's blocklist means guessing the revert text of a condition I cannot reproduce (I have no blocklisted address on Arc). A wrong guess produces a *confidently wrong* explanation, which is worse than the current fallback — which surfaces the chain's own revert reason verbatim. Needs one observed blocklist revert before it is written.
- **T044 — block-number tie-break for equal timestamps.** Arc's sub-second blocks do share timestamps, but the activity feed sorts *stored records* with a stable sort and those records carry no block number at all. Ties therefore keep a deterministic order today; adding block numbers is a storage-schema migration, not a sort change, and does not belong in this feature.

## Still to do

1. **Fund the relay's Arc MAINNET treasury** — `0x3e59292e18417f814112f731e7163534c6d2fe3c` holds `0` on chain 5042. Testnet is funded and proven; mainnet is the same path with real money. Until then, Arc mainnet raises the funding sheet rather than hanging.
2. **Give up loudly.** An operation the relay dead-letters leaves the wallet saying "Waiting for blockchain confirmation…" forever. The relay knows (it has a DLQ); the wallet does not ask. Same shape as the empty-treasury problem, one layer further on.
3. Note for the rollout: the two relays use **different** settlement recipients — docker `0xee2cca98…f0dd`, Cloudflare `0x3e59292e…fe3c` — so moving the built-in bundler moves which treasury must be funded, on every chain.
4. The four-chain non-regression sweep (§7) and the mainnet smoke test (§8). §6 (testnet end-to-end) is done except for the final Arc send.
5. `scripts/verify-i18n-parity.mjs` could not run here — it needs `i18next`, which is not installed in `scripts/`. Pre-existing environment gap; `lint-i18n-corpus.mjs` did run and is clean.

## Environment notes (not code problems)

- Android's Gradle build needs `JAVA_HOME` pointed at a full JDK **and** a daemon restart: a reused daemon picks the VS Code Red Hat extension's trimmed JRE, which has no `jlink`, and `JdkImageTransform` fails before any of this feature's code is compiled.
- `rust/bindings/kotlin` and the iOS XCFramework are generated artifacts, absent or stale in a fresh worktree. Both were regenerated here; without that, any new core export is "unresolved reference" in Kotlin and "cannot find in scope" in Swift.
