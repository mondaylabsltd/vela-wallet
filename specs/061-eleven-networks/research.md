# Research — the eleven, probed on 2026-09-17

## Admission (all live)

| Chain | ID | 11 contracts | RIP-7212 | Block time | Gas coin | Client |
|---|---|---|---|---|---|---|
| X Layer | 196 | 11/11 | ✅ | 1.0 s | OKB | geth (drpc) |
| Stable | 988 | 11/11 | ✅ | 0.7 s | USDT0 | custom |
| Soneium | 1868 | 11/11 | ✅ | 2.0 s | ETH | op-geth |
| MegaETH | 4326 | 11/11 | ✅ | 1.0 s | ETH | geth |
| Robinhood Chain | 4663 | 11/11 | ✅ | 0.1 s | ETH | nitro |
| Mantle | 5000 | 11/11 | ✅ | 2.0 s | MNT | geth 1.16 |
| Kaia | 8217 | 11/11 | ✅ | 1.0 s | KAIA | Klaytn v2.2 |
| Celo | 42220 | 11/11 | ✅ | 1.0 s | CELO | op-geth |
| Ink | 57073 | 11/11 | ✅ | 1.0 s | ETH | op-geth |
| Plume | 98866 | 11/11 | ✅ | 0.45 s | PLUME | nitro |
| XRPL EVM | 1440000 | 11/11 | ✅ | 5.8 s | XRP | Cosmos-EVM |

Controls: `eth_getCode(0xdead…beef)` → `0x`, and a five-word invalid P-256 signature → not `1`, on all eleven.

## Stable (988) has Arc's mirror

`nativeCurrency` is USDT0 with 18 decimals; the directory listed a `USDT0` "bridge" token at `0x779Ded0c9e1022225f8E0630b35a9b54bE713736` with 6 decimals. For every sampled funded address, `eth_getBalance ÷ 10¹²` equalled that token's `balanceOf` exactly (e.g. `2000000000001` wei ↔ `2`). It is the native balance through a 6-decimal window — the same construction as Arc's `0x3600…0000`. Listing it doubles every Stable balance. **Removed from the directory; refused in add-a-token; the coin pegged.**

## Provider slugs (verified, not guessed)

Alchemy answered (HTTP 429 on the shared demo key — host exists) for `xlayer|stable|soneium|megaeth|robinhood|mantle|kaia|celo|ink|plume-mainnet.g.alchemy.com`; `xrpl-evm-mainnet` does not resolve. dRPC recognised `xlayer stable soneium megaeth robinhood mantle kaia celo ink plume` ("token invalid") and not `xrplevm` ("couldn't parse"). X Layer's Alchemy slug had sat in the wallet's map as dead data since before Arc; it becomes live now.

## Fee shape

Robinhood and Plume are Arbitrum Orbit (`nitro`) → `ARBITRUM_CHAIN_IDS`. Soneium, Ink, Celo (an OP-stack L2 since 2025) and Mantle (OP-derived) → `OP_STACK_CHAIN_IDS`. Kaia, Stable and XRPL EVM are L1s. X Layer is a Polygon CDK rollup with no adder of its own; left as is. MegaETH's data-fee model is not OP-shaped; left as is. Mantle's token-ratio gas metering is worth a live send before it ships to people — noted, not done.

## MegaETH's explorer

The directory has none. `megaeth.blockscout.com` answers 200 and is EIP-3091; used.

## Relay

`alchemy.rs` already carried nine of the eleven; Plume added (82 networks). `pegged_native_usd_price` now covers Gnosis, Arc, Arc testnet and Stable — the set of chains whose gas coin is a dollar by construction, which is what keeps the client's $0.01 floor from degrading when Binance is unreachable.
