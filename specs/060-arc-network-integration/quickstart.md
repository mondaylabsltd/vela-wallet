# Quickstart — verifying Arc integration

Run in the order the project's coding rules prescribe: **typecheck → lint → unit → integration → e2e → build**. Everything below is runnable; nothing here depends on production funds except the final mainnet smoke test, which is explicitly marked.

## 0. Prerequisites

- Arc testnet USDC from `https://faucet.circle.com` (chain `5042002`).
- `wrangler` authenticated against the Cloudflare account that owns `getvela.app`, on a **Workers paid plan** (both services need it: ethereum-data for its ~29k static assets, vela-relay-cf for CPU time).
- The three repositories checked out at the paths recorded in [plan.md](./plan.md).

## 1. The chain answers (no build required)

```bash
RPC=https://rpc.mainnet.arc.io
curl -s -X POST $RPC -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}'         # => 0x13b2
curl -s -X POST $RPC -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"eth_gasPrice","params":[]}'        # => ~0x4a817c800 (20 Gwei)
```

Admission bar — every required contract plus the P-256 precompile (the same probe the in-app compatibility checker runs):

```bash
for a in 0x4e59b44847b379578588920cA78FbF26c0B4956C \
         0x914d7Fec6aaC8cd542e72Bca78B30650d45643d7 \
         0xcA11bde05977b3631167028862bE2a173976CA11 \
         0x0000000071727De22E5E9d8BAf0edAc6f37da032 \
         0x29fcB43b46531BcA003ddC8FCB67FFE91900C762 \
         0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67 \
         0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226 \
         0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47 \
         0x94a4F6affBd8975951142c3999aEAB7ecee555c2 \
         0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99 \
         0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526; do
  printf '%s ' "$a"
  curl -s -X POST $RPC -H 'content-type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"eth_getCode\",\"params\":[\"$a\",\"latest\"]}" \
    | grep -qo '"result":"0x[0-9a-f]' && echo present || echo MISSING
done
```

Expected: eleven × `present`. Repeat against `https://rpc.testnet.arc.io`.

## 2. Chain data

```bash
cd /Volumes/data/production/shelchin-workspace/data/ethereum-data
bun run build                      # regenerates index/ and erc7730/
jq '.chainId, .nativeCurrency, .stables' chains/eip155-5042.json
# => 5042 / {"name":"USDC","symbol":"USDC","decimals":18} / []
```

After deploy:

```bash
curl -s https://ethereum-data.getvela.app/chains/eip155-5042.json | jq .chainId   # => 5042
curl -s https://ethereum-data.getvela.app/api/health | jq .service                # => "ethereum-data"
curl -sI https://ethereum-data.getvela.app/chainlogos/eip155-5042.png | head -1   # => 200
curl -s https://ethereum-data.getvela.app/chains/eip155-1.json | jq .chainId      # => 1  (the corpus still serves)
```

## 3. Core rules

```bash
cd /Volumes/data/production/agent-3/vela-wallet/rust
cargo test -p vela-core                      # peg, gas floor, native alias, and the 12-chain parity corpus
cargo test -p vela-core fee_policy            # the money kernels specifically
```

What must be true:

- `pegged_native_usd("USDC") == Some(1.0)`, `pegged_native_usd("USD") == Some(1.0)`, `pegged_native_usd("ETH") == None`.
- `min_gas_price_wei(5042) == 20_000_000_000`; `min_gas_price_wei(1) == 0`.
- **The twelve existing chains produce bit-identical gas prices and fee amounts to the pre-change baseline.** This is the regression that matters: a floor that leaks into another chain silently changes what people pay.
- An Arc fee quote with a working gas-price read and one with a failed read both yield `maxFeePerGas >= 20 Gwei`.
- An Arc fee quote is never below `$0.01`, and matches Gnosis's floor for equivalent gas.

## 4. Shells

```bash
# web
cd app-web/vela-wallet && npm run check && npm run lint && npm run test:unit -- --run

# desktop
cd ../../app-desktop/vela-wallet && cargo test

# android
cd ../../app-android/vela-wallet && ./gradlew test

# ios
cd ../../app-ios/VelaWallet && xcodebuild test -scheme VelaWallet -destination 'platform=iOS Simulator,name=iPhone 16'

# cross-shell parity gates
cd ../.. && node scripts/check-ios-android-parity.mjs && node scripts/check-android-event-parity.mjs
```

Each shell must show Arc in the network list with the USDC symbol, and each shell's native-price resolver must return `1.0` for `USDC` **via the core**, not via a local literal.

## 5. Relay

```bash
cd /Volumes/data/production/vela-relay
cargo test --workspace --exclude vela-relay-cf
cd vela-relay-cf && wrangler deploy
curl -s https://vela-relay-cf.getvela.app/api/health | jq .service    # => "vela-relay"
curl -s https://vela-relay-cf.getvela.app/5042/health                  # Arc lane reachable
```

Arc-specific: the relay must resolve Arc RPCs from the relocated directory, and must **not** route chain `5042` down the Tempo `0x76` path. Its Arc relayer needs native USDC — an unfunded relayer must produce an explainable refusal, never a hang.

## 6. End to end — testnet

1. Add Arc testnet (`5042002`, `https://rpc.testnet.arc.io`) through Settings → Networks → Add network. The compatibility check must pass all eleven contracts plus P-256.
2. Fund the wallet from `https://faucet.circle.com`.
3. Confirm **one** USDC row whose amount equals the explorer's balance.
4. Paste `0x3600000000000000000000000000000000000000` into "add a token" — must be refused as the native coin.
5. Send USDC to a second address. The fee must read as at least $0.01; approve with a passkey; the receipt must resolve within a few seconds and the transfer must appear on `https://explorer.testnet.arc.io`.
6. Repeat with the RPC's `eth_gasPrice` fault-injected to fail — the send must still land.
7. Send to a deliberately invalid destination (the zero address) — the failure must be explained, not generic.

## 7. Non-regression — the twelve existing chains

With the new endpoints active and **no** stored override, on at least Ethereum, Base, Gnosis and Tempo:

- balances match what the pre-change build showed;
- a fee quote matches the pre-change quote for the same send;
- deposit detection still notices an incoming transfer;
- Settings → Endpoints shows every service healthy with its expected identity.

## 8. Mainnet smoke test *(real funds — do last, do once)*

One minimal USDC send on Arc mainnet from a wallet holding a few dollars. Confirm: fee at or above $0.01, inclusion within a few seconds, explorer shows the transfer, balance decremented by amount + fee exactly once.
