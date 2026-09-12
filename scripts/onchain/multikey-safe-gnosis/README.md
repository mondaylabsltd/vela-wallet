# multikey-safe-gnosis — on-chain e2e for the multi-passkey Safe

Live-network integration harness for `compute_safe_address_multi`
(rust/crates/vela-core/src/safe.rs): generate a set of P-256 keys, compute
the counterfactual multi-owner Safe with the vela-core wasm bindings, fund
it, and deploy it on Gnosis through the production relay
(https://vela-relay.getvela.app) with in-band gas settlement.

The deploy step signs with a **non-first key by default** (`SIGNER_INDEX=1`),
which exercises the load-bearing design decision: extra keys' signer proxies
are deployed *inside* the setup MultiSend (`createSigner` CALL sub-txs), so
any key can sign the very first user operation — the proxy that validates
the signature only comes into existence in that same transaction.

## Usage

```sh
cd scripts/onchain/multikey-safe-gnosis
bun run generate   # new keyset → state/keys.json + prints the Safe address
# … fund the printed address with xDAI (≥ the in-band fee, 1 xDAI is plenty) …
bun run status     # balance / Safe code / signer-proxy code
bun run deploy     # quote → estimate → sign SafeOp → submit → poll receipt
bun run send       # post-deploy: normal tx from the Safe, any SIGNER_INDEX
bun run regress    # single-key backward-compat: golden fixture addresses +
                   # production conformance vectors through the shipped wasm
```

Knobs (env): `KEY_COUNT` (generate, default 3), `SIGNER_INDEX` (deploy
default 1, send default 0; 0 = shared-signer path, ≥1 = signer-proxy path),
`TO` / `VALUE_XDAI` (send, default 0.0001 self-transfer), `RELAY_URL`,
`GNOSIS_RPC`.

`state/` is gitignored — the keys are throwaway test keys, but they control
whatever the Safe holds, so treat the file accordingly. `generate` refuses
to overwrite an existing keyset without `--force`.

## Prerequisites

- `npm run build:wasm` at the repo root whenever the Rust wasm surface
  changed — the scripts load `rust/pkg-web` + the fingerprinted
  `assets/wasm/vela_core_bg.<hash>.wasm` and will miss new exports otherwise.
- bun (loads the pkg-web ESM glue directly; no bundler).

## Relay contract (mirrors src/services/safe-transaction.ts)

- Endpoint `POST {RELAY_URL}/{chainId}`, standard bundler JSON-RPC plus
  `vela_getInBandGasQuote`; no auth; v0.7 unpacked userOp with
  `factory`/`factoryData` split and **deny_unknown_fields** (send no extras).
- In-band gas: `maxFeePerGas = maxPriorityFeePerGas = 0x0`; callData MUST be
  `executeUserOp(MULTI_SEND, 0, multiSend(...), 1)` containing a native
  CALL leg of ≥ 0.00001 xDAI to the quote's `recipient` (fee sizing:
  3× estimated gas cost, floored).
- Signature = SafeOp EIP-712 hash (verifyingContract = the 4337 module,
  validAfter/validUntil = 0) signed by a synthetic WebAuthn assertion
  (flags must carry UV 0x04), wrapped in the 12-byte-prefixed ERC-1271
  contract-signature envelope. Owner word = shared signer for key 0, the
  per-key `SafeWebAuthnSignerProxy` otherwise.

## First successful run (2026-08-14)

Safe `0x509eaA52ff83FCfa74148549EFb397B6B1dAe74a` (3 keys, threshold 1) —
all three owner paths proven on-chain:

- deploy, signed by **key 1** (factory signer `0x8BcD…6f69`, deployed inside
  the same userOp's setup):
  https://gnosisscan.io/tx/0x98c75792873ede79bdfa710a45aaaf42e52f62997fb896ce6a29165d96b68cb5
- send, signed by **key 0** (shared signer):
  https://gnosisscan.io/tx/0xce37ba707c0c23974206c093358c8ce0962cffaf352184205b75a12155b3d732
- send, signed by **key 2** (factory signer `0x253a…19a8`):
  https://gnosisscan.io/tx/0x207a431c1c56fc2728768b156fb304a311998e74f803c631f7283c1db333815a

Each op paid its 0.00001 xDAI fee in-band from the Safe's own balance.

## Key-count limits (measured 2026-08-14, `bun run limits`)

Per extra key: ~119k deploy gas, ~220 bytes of setupData — linear 3→100.
Binding constraints, loosest to tightest: relay body limit 1 MiB (never
binds); Gnosis block gas 17M → ~140 keys; **the production relay's
~10M simulation cap → ~80 keys practical max** (N=75 estimates OK, N=90
rejected). Byte-level limits never bind (EIP-3860 does not apply to setup —
it runs as a CALL, not initcode).

13-key proof run: Safe `0x19E5BF2278b2cB5E302f50a656b004D5333646BF`, funded
by sweeping the 3-key wallet (`STATE_FILE=state/keys-3key-…json bun run
send`), deployed in one userOp **signed by key 12** — 13 owners + 12 signer
proxies, 1,843,566 gas actual (simulation predicted 1.81M + 4337 overhead):
https://gnosisscan.io/tx/0xfdcf04438d3aacaf3c66bccbecbb33351798d437fbced059e9c83e981dc2dc54

Deployment verificationGasLimit is sized from an on-chain `eth_estimateGas`
of the factory call (×1.2 + 300k), computed BEFORE the relay estimate — the
relay both simulates with the limits you pass (a low provisional OOGs the
estimate itself for large wallets) and bottoms its verification number out
at a floor, so its estimate is never used for verification.

vela-core enforces `MAX_MULTI_KEYS = 7` (~1.2M deploy gas, comfortably
under every measured ceiling); over-cap key counts are rejected in core.
Headroom proof run, recorded when the cap was 21: Safe
`0x898cFE71FBB4A7fb03513Ca71d30beE838B60e9f`, 21 owners + 20 signer
proxies in one userOp **signed by key 20**, 2,749,568 gas actual
(simulation predicted 2.75M):
https://gnosisscan.io/tx/0x1d6217e296f1530f91f5c7e59be165090db518063092dd7088c7802f6ddd3928
