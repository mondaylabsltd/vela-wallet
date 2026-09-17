# Phase 0 Research — Arc Network Integration

All probes below were run against live endpoints on **2026-09-17** from this machine. Where a claim is Circle's rather than measured, it is marked *(docs)*.

## R1 — Does Arc clear the wallet's own admission bar?

**Decision**: Yes. No change to the signing, account or recovery paths.

**Evidence**: every address in `REQUIRED_CONTRACTS` ([network_admin.rs:95](../../rust/crates/vela-core/src/app/network_admin.rs#L95)) returned non-empty code on **both** Arc mainnet (`5042`) and Arc testnet (`5042002`):

| Contract | Address | mainnet | testnet |
|---|---|---|---|
| Deterministic Deployment Proxy | `0x4e59b448…4956C` | ✅ 69 B | ✅ |
| Safe Singleton Factory | `0x914d7Fec…643d7` | ✅ 69 B | ✅ 69 B |
| Multicall3 | `0xcA11bde0…6CA11` | ✅ 3.8 kB | ✅ 3.8 kB |
| EntryPoint v0.7 | `0x00000000…a032` | ✅ 16 kB | ✅ 16 kB |
| Safe L2 | `0x29fcB43b…0C762` | ✅ 24 kB | ✅ 24 kB |
| Safe Proxy Factory | `0x4e1DCf7A…0ec67` | ✅ 3 kB | ✅ 3 kB |
| Safe 4337 Module | `0x75cf1146…5c226` | ✅ 8.3 kB | ✅ 8.3 kB |
| Safe Module Setup | `0x2dd68b00…5b47` | ✅ | — |
| WebAuthn Signer | `0x94a4F6af…555c2` | ✅ 3 kB | ✅ 3 kB |
| Fallback Handler | `0xfd0732Dc…9Ec99` | ✅ | — |
| MultiSend | `0x38869bf6…3B526` | ✅ | — |

RIP-7212: `eth_call` to `0x…0100` with `VALID_P256_CALL` ([network_admin.rs:137](../../rust/crates/vela-core/src/app/network_admin.rs#L137)) returned `0x…01` on both networks. `eth_getCode` at that address is `0x`, as expected for a native precompile — the checker's two-strategy probe already handles this.

**Consequence**: counterfactual Safe addresses on Arc equal those on every other chain (same CREATE2 factory + same singletons), so cross-device recovery and the address-is-a-function-of-the-keys property hold unchanged.

## R2 — What the native coin actually is

**Decision**: Treat Arc's native coin as USDC at a fixed $1, and treat `0x3600…0000` as *the same balance*, never a token.

**Evidence**:
- `eth_chainId` → `0x13b2` (5042); `web3_clientVersion` → `arc/v1`.
- `0x3600000000000000000000000000000000000000` has code; `decimals()` → `6`; `symbol()` → `USDC`.
- Protocol accounting is 18 decimals *(docs)*; the ERC-20 view divides by 10¹². Circle's own integration note: "treat native USDC and ERC-20 USDC as a single balance rather than separate rows".
- `eth_getLogs` over 21 consecutive blocks returned 730 `Transfer` events from `0x3600…0000` with 6-decimal values — so USDC movement is observable through ordinary ERC-20 log watching.

**Alternatives considered**:
- *List `0x3600…0000` as a stablecoin in the chain data* — rejected: it is the mechanism that would double every Arc balance.
- *Name Arc's native symbol `USD` so the existing Tempo peg fires unchanged* — rejected: it is a lie on screen (the coin is USDC, the explorer calls it USDC) and it buys only the avoidance of one small core rule.

## R3 — Fee model

**Decision**: Arc uses the ordinary native-gas path with two pieces of new per-chain data: a `$1` peg for the native symbol, and a 20 Gwei minimum gas price.

**Evidence**:
- `eth_gasPrice` → `0x4a817d261` = 20.000000353 Gwei. `baseFeePerGas` = exactly `0x4a817c800` = 20 Gwei across sampled blocks; `extraData` = `0x00000004a817c800` — the next block's base fee, as documented.
- *(docs)* Minimum base fee 20 Gwei, maximum 20,000 Gwei, EWMA smoothing, 30M gas/block, ~$0.001 target per ERC-20 transfer, **"transactions with `maxFeePerGas` lower than 20 Gwei are silently dropped"**.
- `eth_feeHistory` works and returns real reward percentiles, so the existing fee-signal plumbing needs no new RPC.

**The hazard this creates**: [fee_policy.rs:123](../../rust/crates/vela-core/src/app/fee_policy.rs#L123) falls back to `FALLBACK_GAS_PRICE_WEI = 5 Gwei` when `eth_gasPrice` cannot be read. With the ×2 bundler margin that is a 10 Gwei `maxFeePerGas` — below Arc's floor, and therefore an operation that is accepted by the wallet, shown as submitted, and **never mined, with no error anywhere**. A per-chain floor is not a nicety here; it is the difference between a loud failure and a lost payment.

**The $0.01 question (raised by the requester)**: the floor is already universal, including Gnosis. [`calculate_in_band_fee_amount`](../../rust/crates/vela-core/src/app/fee_policy.rs#L593) computes `native_minimum` as "$0.01 worth of the native coin, but never below the 0.00001-coin relay admission floor" for **every** chain; only when the native coin has *no price* does it degrade to a blind 0.001-coin floor. Gnosis is priced (chain-local DAI/USD feed `0x678df341…`, plus the `XDAI → DAI` alias in [price-service.ts:44](../../app-web/vela-wallet/src/lib/services/price-service.ts#L44)), so Gnosis gets a true $0.01 floor today. **Arc inherits the same $0.01 floor for free the moment its native coin is priced** — which is exactly what the peg does. No Arc-specific fee code, no new floor constant.

**Alternatives considered**:
- *Reuse Tempo's `gasModel: 'tempo'`* — rejected. Tempo has **no** native coin and settles gas as a batched TIP-20 reimbursement through a `0x76` transaction. Arc has a native coin and pays gas natively. Sharing the model would drag in pathUSD, the fee-collector staleness gate and the `0x76` signing path for no reason.
- *Hard-code a 20 Gwei branch in the shells* — rejected: four copies of a money rule is precisely the failure mode the Rust core exists to end.

## R4 — Where the $1 peg lives

**Decision**: one rule in `vela-core`, consulted by all four shells.

**Evidence of the problem**: the `"USD" ⇒ $1` peg is currently written four times —
[price-service.ts:116](../../app-web/vela-wallet/src/lib/services/price-service.ts#L116) (web), [Prices.swift:136](../../app-ios/VelaWallet/VelaWallet/Core/Prices.swift#L136) (iOS), [ChainlinkPrices.kt:87](../../app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/wallet/core/ChainlinkPrices.kt#L87) (Android), [chainlink.rs:144](../../app-desktop/vela-wallet/src/executor/chainlink.rs#L144) (desktop). Adding `USDC` to four hand-maintained copies is how a pricing rule silently diverges.

**Design**: export `pegged_native_usd(symbol) -> Option<f64>` from the core (`{USD, USDC} ⇒ 1.0`), and have each shell's `resolve*Price` consult it in place of its literal comparison. Four small edits, one rule, and a parity test that reads the core's answer.

## R5 — RPC provider slugs

**Decision**: Alchemy `arc-mainnet`, dRPC `arc`. Ankr: none.

**Evidence**: `arc-mainnet.g.alchemy.com` answers (HTTP 429 on the shared demo key) while a fabricated subdomain fails to connect at all — the host exists. dRPC returns `"Your token is invalid or expired"` for `network=arc` and `network=arc-testnet` but `"Couldn't parse request"` for a fabricated network name, so `arc` is a network it knows. Ankr's supported list does not include Arc, consistent with it also lacking Unichain, World Chain, Monad and Tempo. *(docs)* also name Blockdaemon and QuickNode, which the wallet does not integrate.

## R6 — Chain data and the service move

**Decision**: author `eip155-5042.json` and `eip155-5042002.json` in the ethereum-data repo; deploy it and the relay to Cloudflare Workers at the two `getvela.app` hostnames.

**Evidence / constraints**:
- `https://ethereum-data.awesometools.dev/chains/eip155-5042.json` currently 404s (serves the SPA shell), which is why the wallet has no Arc data today.
- The chain-file schema is set by its neighbours: [`eip155-100.json`](file:///Volumes/data/production/shelchin-workspace/data/ethereum-data/chains/eip155-100.json) carries `stables[]`, `wrappedNativeToken`, `dex`, `features[{name:RIP7212}]`, `contracts.safeProxyFactory`. Tempo's file is the precedent for a chain with stables but **no** `dex` and **no** `wrappedNativeToken`.
- ethereum-data deploys as Workers Static Assets + a WASM worker for `/api/*`; ~29k static files means the **Workers paid plan** is required (documented in its `wrangler.toml`), and the compiled worker is committed so no Rust toolchain is needed at deploy time. A custom domain must be bound for `ethereum-data.getvela.app`.
- Chain logos are served from `/chainlogos/eip155-<id>.png` — note the files are actually WebP bytes under a `.png` name. A missing logo is survivable (the shells fall back to `iconLabel`/`iconColor`), but Arc should ship one.
- The relay already has a Cloudflare shell, `vela-relay-cf`, with Durable Objects, Queues and KV declared in its `wrangler.jsonc`, and it discovers chain RPCs dynamically from the chain-data service — [`market.rs:9`](file:///Volumes/data/production/vela-relay/vela-relay-cf/src/arms/market.rs) and [`rpc.rs:15`](file:///Volumes/data/production/vela-relay/src/utils/rpc.rs) both hard-code the old `awesometools.dev` host and must move with it.

**Consequence**: once Arc's chain file exists at the new host, the relay resolves Arc RPCs without a per-chain code change. What the relay *does* need is (a) the hostname change, (b) confirmation that Arc is not routed down the Tempo `0x76` path, and (c) a funded Arc treasury — native USDC, not ETH.

## R7 — Ordering, timestamps, and other EVM deltas

**Decision**: no behavioural change needed beyond ordering hygiene; record the rest.

*(docs, spot-checked)*: `PREVRANDAO` = 0, `BLOBHASH` = 0, `BLOBBASEFEE` = 1, type-3 transactions rejected by the mempool, withdrawals always empty, `parentBeaconBlockRoot` = parent execution block hash with no beacon-roots contract, EIP-7702 / CREATE2 / EIP-2935 supported, base fee paid to the block beneficiary rather than burned. None of these is read by the wallet.

Two that do matter:
1. **Timestamps are non-decreasing, not strictly increasing** — sub-second blocks share a timestamp, so any ordering that uses `block.timestamp` alone will shuffle rows. Tie-break by block number.
2. **Transfers can revert for issuer reasons** — blocklisted sender or recipient, destination that has self-destructed, transfers to the zero address. This is a new *category* of revert for the wallet, and it deserves a sentence a person can act on rather than a generic estimation failure.

## R8 — Blast radius of the endpoint move

**Decision**: change the default in the core and the four shells; never touch a stored override.

**Inventory** (source trees only; fixtures and docs listed so they stay consistent):

| What | Where |
|---|---|
| `DEFAULT_ETHEREUM_DATA_URL` | [network_admin.rs:157](../../rust/crates/vela-core/src/app/network_admin.rs#L157) |
| `BUNDLER_BASE`, `DEFAULT_BUNDLER_SERVICE_URL` | [network_admin.rs:154](../../rust/crates/vela-core/src/app/network_admin.rs#L154) |
| web defaults | `services/endpoints.ts`, `services/networks.ts` (logo base + bundler base), `settings/fixtures.ts`, `services/chain-tokens.ts` (comment) |
| iOS | `Features/Settings/NetworkAdminExecutor.swift`, `Features/Settings/SettingsFixtures.swift`, `VelaWalletTests/NetworkAdminLiveTests.swift` |
| Android | `VelaWalletApplication.kt`, `feature/settings/SettingsFixtures.kt` |
| desktop | `settings/fixtures.rs`, `executor/relay.rs` (`BUILTIN_BASE`), `executor/pool.rs` |
| relay | `vela-relay-cf/src/arms/market.rs`, `src/utils/rpc.rs`, `src/app/rpc/handlers/README.md` |

`SERVICE_IDENTITY` ([network_admin.rs:166](../../rust/crates/vela-core/src/app/network_admin.rs#L166)) is unchanged: the relocated services keep reporting `ethereum-data` and `vela-relay`, so the Settings health check passes without edits.

## R9 — Arc's other assets are not "stables"

**Decision**: Arc's chain-data entry carries an **empty** stablecoin list. EURC and USYC are deliberately left out.

**Evidence**: a token that appears in a chain's curated stablecoin list is priced at exactly `1.0` — not as a fallback, but as the definition of membership ([wallet-api.ts:641](../../app-web/vela-wallet/src/lib/services/wallet-api.ts#L641), whose comment spells out that `stable` is a membership verdict and ≈$1 is what membership means, with the same table core-owned for records and clear-signing). EURC is a euro token and USYC is yield-bearing; both would be valued wrongly, silently, in the portfolio total and in transaction records.

Arc's dollar *is* the native coin, so the curated list has nothing legitimate to hold. Both assets stay reachable through "add a token", where they are priced by quote or left unpriced rather than asserted to be a dollar.

**Alternatives considered**: *list EURC with a euro peg* — rejected; the list has no currency dimension, and inventing one is a separate feature with its own FX correctness questions.

## R10 — The relay's Alchemy map had Arc testnet but not Arc mainnet

**Decision**: add `arc` → `https://arc-mainnet.g.alchemy.com/v2/` (chain 5042) to `vela_relay_core::alchemy::ALCHEMY_NETWORKS`, and peg Arc's native coin at $1 in `settlement::pegged_native_usd_price`.

**Evidence**: found live, not by reading. `vela_getInBandGasQuote` answered "temporarily unavailable" for chain 5042 while 5042002 and every existing chain answered normally. `wrangler tail` showed `in-band gas quote Multicall request failed: 5042` — the RPC call, not the metadata. The relay's `rpc::call` tries its Alchemy key before the directory's public endpoints, and the map carried `arc-testnet` (5042002) with no mainnet entry, so testnet went through Alchemy and mainnet fell through to a public endpoint the Worker could not reach reliably. 0/6 quotes before the entry, 5/5 after.

**The peg, separately**: the relay priced Arc's USDC from Binance's `USDCUSDT` ticker (it answered `1.00065`). Accurate, but the client's fee floor is "$0.01 worth of the native coin" and it reads that price **out of the relay's quote** — so a market outage would have silently degraded Arc's floor to the blind 0.001-coin fallback, a tenth of a cent on a dollar coin. Arc now joins Gnosis in the pegged set, and both relay shells consult that one predicate rather than asking "is this Gnosis".

**Also found**: Arc's chain file listed one RPC endpoint. `https://arc.drpc.org` and `https://arc-rpc.publicnode.com` were verified (chain id, and the exact `aggregate3` the relay sends) and added, so the fallback tier is not a single point of failure. This did not fix the bug — it is worth having anyway.
