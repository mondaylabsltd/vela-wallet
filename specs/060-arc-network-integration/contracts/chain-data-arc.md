# Contract — Arc chain-data files

Served by the chain-data service at `https://ethereum-data.getvela.app/chains/eip155-<id>.json`. Consumed by the wallet (`chain-tokens.ts` and its three shell twins) and by the relay (`src/utils/rpc.rs`, `vela-relay-cf/src/arms/market.rs`).

Field semantics are fixed by the existing corpus — see `eip155-100.json` (Gnosis, the full shape) and `eip155-4217.json` (Tempo, the precedent for a chain with no DEX and no wrapped native).

## `chains/eip155-5042.json` — Arc mainnet *(authored here)*

Styled to match the corpus's upstream conventions (`name`/`chain`/`icon` as chainid.network spells them) so a future upstream sync does not fight it. Every endpoint below was probed live: `rpc.mainnet.arc.io` answers `0x13b2` over both HTTPS and WSS, and no `*.arc.network` mainnet host answers at all.

```json
{
  "name": "Arc Network",
  "chain": "Arc Network",
  "icon": "arcnetwork",
  "rpc": ["https://rpc.mainnet.arc.io", "wss://rpc.mainnet.arc.io"],
  "faucets": [],
  "nativeCurrency": { "name": "USDC", "symbol": "USDC", "decimals": 18 },
  "infoURL": "https://arc.network",
  "shortName": "arc",
  "chainId": 5042,
  "networkId": 5042,
  "explorers": [
    { "name": "Arc Explorer", "url": "https://explorer.arc.io", "standard": "EIP3091" }
  ],
  "stables": [],
  "features": [{ "name": "RIP7212" }],
  "contracts": { "safeProxyFactory": "0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67" }
}
```

## `chains/eip155-5042002.json` — Arc testnet *(already existed — do not rewrite)*

The corpus **already carries** a curated Arc testnet entry, complete with `contracts.safeProxyFactory`, the RIP-7212 feature marker and the Circle faucet. It names the chain `Arc Network Testnet`, uses the `arc.network` RPC domain (which answers: `rpc.testnet.arc.network` returns `0x4cef52`) and the `testnet.arcscan.app` explorer.

It needs no change. An earlier draft of this contract proposed replacing it with `arc.io` spellings; that was reverted — both domains work, and rewriting an upstream-synced file to a second valid spelling buys nothing and loses the QuickNode and Blockdaemon endpoints it lists.

`stables` is simply absent there, which every consumer reads as empty. Absence and `[]` are equivalent; the mainnet file states it explicitly because its emptiness is a deliberate decision (rule 1 below) rather than a gap.

## Logos *(already present)*

`chainlogos/eip155-5042.png` and `chainlogos/eip155-5042002.png` already exist in the working tree as 400×400 PNGs of Arc's arch mark, untracked and awaiting commit. Do not regenerate them.

## Rules this contract enforces

1. **`stables` is empty, and that is deliberate.** A token listed here is priced at exactly `1.0` by every shell — membership *is* the claim "worth about a dollar" ([wallet-api.ts:641](../../../app-web/vela-wallet/src/lib/services/wallet-api.ts#L641)). Arc's dollar is its native coin. EURC (`0xbEf5f6d51CB62b58e6A8f77868681825C6fe21c1`) is a euro token and USYC (`0x8a5D989Bbb96929F689B0200f435f53dA42bF490`) is yield-bearing; neither may appear here. Both remain addable as ordinary tokens.
2. **The ERC-20 USDC interface `0x3600000000000000000000000000000000000000` never appears in this file**, under any key. It is the native balance seen through a 6-decimal window; listing it doubles every Arc balance in the wallet.
3. **No `dex` and no `wrappedNativeToken`.** Arc has neither. Their absence is what makes the shells skip DEX quoting entirely and rely on the peg.
4. **`nativeCurrency.decimals` is 18**, matching protocol accounting — not the 6 the ERC-20 interface reports.
5. **`features` must carry `RIP7212`**, which the live probe confirms; the compatibility checker reads the precompile itself, but this keeps the corpus honest.

## Verification

```bash
curl -s https://ethereum-data.getvela.app/chains/eip155-5042.json | jq '.chainId, .nativeCurrency, .stables, .dex'
# => 5042, {"name":"USDC","symbol":"USDC","decimals":18}, [], null
```
