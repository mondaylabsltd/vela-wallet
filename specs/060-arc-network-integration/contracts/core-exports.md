# Contract — new core surface

Two rules move into `vela-core` and become the single implementation every shell consults. Both are pure functions over data: no I/O, no clock, no network — the core's standing constraint.

## 1. `pegged_native_usd`

**Rust** — `vela_core::app::balance_dashboard`:

```rust
/// The USD price of a native gas coin that is a dollar stablecoin by
/// construction, where no DEX or Chainlink feed can price it.
/// Case-insensitive. `None` means "not pegged" — the caller falls through
/// to the existing Chainlink/DEX ladder unchanged.
pub fn pegged_native_usd(symbol: &str) -> Option<f64>
```

| Input | Output |
|---|---|
| `"USD"`, `"usd"` | `Some(1.0)` — Tempo, existing behaviour |
| `"USDC"`, `"usdc"` | `Some(1.0)` — Arc |
| `"ETH"`, `"XDAI"`, `""`, anything else | `None` |

**Bindings** (both already export `choose_native_price` from this module, so this joins it):

| Shell | Surface |
|---|---|
| web | `vela-core-wasm`, `#[wasm_bindgen(js_name = peggedNativeUsd)]` → `peggedNativeUsd(symbol: string): number \| undefined` |
| iOS / Android | `vela-core-uniffi`, `pegged_native_usd(symbol: String) -> Double?` |
| desktop | direct crate dependency |

**Call sites replaced** — each shell's native-price resolver consults the core instead of its own literal:

- `app-web/.../services/price-service.ts` → `resolveChainlinkPrice`
- `app-ios/.../Core/Prices.swift` → `resolve`
- `app-android/.../feature/wallet/core/ChainlinkPrices.kt` → `resolve`
- `app-desktop/.../executor/chainlink.rs` → `resolve`

The peg is consulted at exactly the rung the `"USD"` literal occupies today: before the symbol lookup, after case normalisation. No other precedence changes.

## 2. Per-chain minimum gas price

**Rust** — `vela_core::app::fee_policy`:

```rust
/// The lowest gas price a chain will accept. Arc discards anything below
/// its 20 Gwei floor *silently*, so a price under the floor is not a cheap
/// transaction — it is a transaction that never happens and never errors.
/// `0` for chains without a floor: every existing chain.
pub fn min_gas_price_wei(chain_id: u32) -> u128
```

| Chain id | Result |
|---|---|
| `5042`, `5042002` | `20_000_000_000` |
| anything else | `0` |

**Applied inside** `derive_chain_gas_price` and the static-fallback path, by raising the derived `gas_price` (and the `base_fee` it is built from) to the floor before any margin is applied:

```text
gas_price = max(derive(signals), min_gas_price_wei(chain_id))
```

**Invariant the parity corpus pins**: for every chain with floor `0`, the output is bit-identical to today's. The floor may only raise a price.

**Why it lives here and not in a shell**: the fallback that triggers the hazard (`FALLBACK_GAS_PRICE_WEI = 5 Gwei`) is in this module; a floor applied anywhere else would miss it.

## 3. Native-coin alias (add-token refusal)

**Rust** — `vela_core::app::manage_tokens` (or the address-validation kernel it already uses):

```rust
/// The ERC-20 interface onto a chain's NATIVE balance, where one exists.
/// Adding it as a token would show the same money twice.
pub fn native_alias_token(chain_id: u32) -> Option<&'static str>
```

| Chain id | Result |
|---|---|
| `5042`, `5042002` | `Some("0x3600000000000000000000000000000000000000")` |
| anything else | `None` |

The add-token path refuses a match with a message naming the native coin rather than a generic validation error. Comparison is lowercase-normalised, as every address comparison in the core already is.
