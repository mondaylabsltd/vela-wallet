# Feature Specification: Eleven more built-in networks

**Feature Branch**: `060-arc-network-integration` (continuation) · **Created**: 2026-09-17 · **Status**: Implemented

**Input**: "内置网络希望添加这些：X Layer (196), Celo (42220), Robinhood Chain (4663), Kaia (8217), Soneium (1868), MegaETH (4326), Stable (988), XRPL EVM Sidechain (1440000), Plume (98866), Mantle (5000), Ink (57073)。先确定 Vela 可以无缝支持吗；需要部署合约的说明哪些；完全无法支持的也说清楚。"

## The answer to the question asked

Every one of the eleven clears the wallet's admission bar **today**: all eleven required contracts are deployed at the canonical addresses and the RIP-7212 P-256 precompile answers a known-good signature. **Nothing needs deploying, and none is unsupportable.** The probe was controlled — an empty address returned `0x` and an invalid signature did not return `1` on every chain — so the uniform result is real, not an RPC that says yes to everything.

What "seamless" then turns on is not contracts but the same three things Arc turned on: what the gas coin is worth, whether the fee estimate knows the chain's shape, and whether the relay can reach and pay on it.

## Requirements

- **FR-001** The eleven are built-in networks on all four shells, in the core's order, with verified provider slugs; a provider is listed only where its endpoint was seen to answer.
- **FR-002** Stable's native coin (`USDT0`) is pegged at $1 by the core, and its ERC-20 mirror `0x779Ded…3736` is neither listed as a token nor addable as one. (Verified address by address: `native ÷ 10¹² == erc20`.)
- **FR-003** The Arbitrum-Orbit pair (Robinhood, Plume) and the OP-stack four (Soneium, Ink, Celo, Mantle) join the static L1-data-fee adders, so a failed estimate does not under-price.
- **FR-004** The relay resolves Plume through Alchemy (the map had every other chain), and pegs Stable's native coin alongside Gnosis's and Arc's.
- **FR-005** No existing chain's fee, balance or provider behaviour changes; the twelve-plus-Arc parity tests stay green.

## Out of scope, and why

- **Relay treasuries.** Each new chain needs its relayer funded in that chain's coin. The wallet's out-of-gas sheet already says who can fix it; funding is an operation.
- **Native prices for OKB, CELO, KAIA, MNT, PLUME, XRP.** None has a Chainlink feed in the wallet's set; they price through each chain's DEX as listed in the chain directory. Pegging is reserved for coins that are dollars by construction.
- **XRPL EVM providers.** Neither Alchemy nor dRPC serves it; the wallet uses the chain's public endpoints.
