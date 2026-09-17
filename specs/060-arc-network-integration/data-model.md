# Phase 1 Data Model — Arc Network Integration

Four entities change. Three are tables the wallet already has and Arc adds a row to; one is a rule that exists today in four copies and becomes one.

## 1. Built-in chain row — `Arc`

The canonical table is `BUILTIN_CHAINS` in [network_admin.rs:212](../../rust/crates/vela-core/src/app/network_admin.rs#L212); each shell mirrors the fields it renders.

| Field | Value | Note |
|---|---|---|
| `id` | `arc` | stable key used by storage and the network picker |
| `display_name` | `Arc` | |
| `chain_id` | `5042` | `0x13b2` |
| `native_symbol` | `USDC` | not `USD` — the coin is USDC and the explorer calls it that |
| `rpc_url` | `https://rpc.mainnet.arc.io` | Circle's public endpoint |
| `explorer_url` | `https://explorer.arc.io` | EIP-3091 |
| `typical_inclusion_s` | `2` | ~0.48 s blocks × relay inclusion depth; the receipt screen's "usually about N seconds" |
| `is_L2` (shells) | `false` | an L1; also excluded from the Arbitrum and OP-stack gas adders |
| `api_network_id` (shells) | `arc-mainnet` | provider-slug key |
| `icon_label` / colours | `USDC` / Circle blue | fallback when the logo asset is unavailable |

**Provider slugs** ([network_admin.rs:340](../../rust/crates/vela-core/src/app/network_admin.rs#L340)): Alchemy `arc-mainnet`, dRPC `arc`, Ankr none. Verified in research R5; an unverified slug is omitted rather than guessed, because a wrong slug poisons the RPC pool at the highest tier.

**Not added**: Arc testnet `5042002`. It is reachable through the custom-network flow and is the verification vehicle for this feature.

## 2. Native price peg — one rule, was four

**Today**: each shell hard-codes `symbol == "USD" ⇒ $1` — [price-service.ts:116](../../app-web/vela-wallet/src/lib/services/price-service.ts#L116), [Prices.swift:136](../../app-ios/VelaWallet/VelaWallet/Core/Prices.swift#L136), [ChainlinkPrices.kt:87](../../app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/wallet/core/ChainlinkPrices.kt#L87), [chainlink.rs:144](../../app-desktop/vela-wallet/src/executor/chainlink.rs#L144).

**After**: one table in `app::balance_dashboard`, beside `choose_native_price`, exported through both binding crates.

| Native symbol | USD price | Chain |
|---|---|---|
| `USD` | 1.0 | Tempo (existing behaviour, unchanged) |
| `USDC` | 1.0 | Arc |
| anything else | none — fall through to Chainlink/DEX as today | |

Comparison is case-insensitive, matching the shells' current `uppercased()` behaviour. The peg is consulted at the same point in the ladder the `"USD"` literal occupies today, so no other rung's precedence changes.

**Why this matters beyond Arc**: the peg is what gives a chain's native coin a price, and a priced native coin is what makes the universal `$0.01` fee floor apply. Arc gets the same floor as Gnosis and Ethereum through this table alone — no fee code knows Arc exists.

## 3. Per-chain minimum gas price

New data in [fee_policy.rs](../../rust/crates/vela-core/src/app/fee_policy.rs), alongside the existing per-chain gas tables (`ARBITRUM_CHAIN_IDS`, `OP_STACK_CHAIN_IDS`, `TEMPO_CHAIN_IDS`).

| Chain | Minimum gas price | Source |
|---|---|---|
| `5042`, `5042002` (Arc) | 20 Gwei (`20_000_000_000` wei) | chain rejects anything lower, silently |
| every other chain | `0` | unchanged behaviour |

**Applied where**: to the gas price the fee machine derives — both the measured path and the `FALLBACK_GAS_PRICE_WEI` path — *before* the ×2 bundler margin, so the floor holds even when `eth_gasPrice` cannot be read. The floor raises a price; it never lowers one.

**Invariant**: for a chain with floor `0`, every derived value is bit-identical to today's. This is what the twelve-chain parity corpus pins.

## 4. Chain-data entry

Served at `chains/eip155-<id>.json`; consumed by the wallet (RPC list, stablecoins, DEX, wrapped native) and by the relay (RPC list). Full contract in [contracts/chain-data-arc.md](./contracts/chain-data-arc.md).

Arc's shape is deliberately sparse:

| Field | Arc value | Why |
|---|---|---|
| `nativeCurrency` | `USDC`, 18 decimals | protocol accounting is 18-decimal |
| `stables` | `[]` | membership in this list *means* "≈ $1.00"; Arc's dollar is the native coin, and EURC/USYC are not dollars (research R9) |
| `wrappedNativeToken` | absent | Arc has no wrapped native |
| `dex` | absent | no DEX to quote against; the peg supplies the price |
| `features` | `[{ "name": "RIP7212" }]` | verified live |
| `contracts.safeProxyFactory` | canonical address | matches every other chain |

## 5. Native-coin alias

`0x3600000000000000000000000000000000000000` on chain `5042` is **not a token**. It is the ERC-20 interface onto the native balance, 6 decimals against the coin's 18.

| Surface | Required behaviour |
|---|---|
| chain-data `stables` | absent (see above) — so it is never discovered |
| add-a-token | refused on chain `5042` with "this is Arc's native coin" |
| token list / portfolio total | one USDC row, counted once |
| Max / reserve arithmetic | transfer asset and fee asset are the same asset, as for any native send |

The alias is per-chain data (`chain 5042 → 0x3600…0000`), not a global address denylist: the same address on another chain has no special meaning.
