# 100 — Quickstart (how to see it work)

## Core

```sh
cd rust && cargo test -p vela-core --features crux --test app_dapp_add_network_100 \
  && cargo test -p vela-core --features crux --test app_network_admin --test app_dapp_browser
```

## The test page

`app-android/vela-wallet/dev/testdapp/index.html` has a "Spec 100" row of buttons (one per case
below). Serve it on loopback — loopback http is a secure context, so every client offers it the
wallet:

```sh
cd app-android/vela-wallet/dev/testdapp && python3 -m http.server 8137 --bind 127.0.0.1
# Android:  adb reverse tcp:8137 tcp:8137        iOS simulator / desktop: as is
# open http://127.0.0.1:8137 in Vela's browser (or Chrome with the extension), tap Connect
```

For case (b-ok), a chain no catalog knows that IS compatible — a Sepolia fork under a made-up id:

```sh
anvil --fork-url https://ethereum-sepolia-rpc.publicnode.com --chain-id 987654321 --hardfork osaka
# Android: adb reverse tcp:8545 tcp:8545
```

(Measured 2026-10-04: Sepolia and Base Sepolia carry all twelve contracts and answer the P256 probe
with `…01`; Ethereum Classic has 7 of 12; the fork above has 12 of 12 and P256.)

## What each button should show

| Button | Asks for | Sheet | Page hears |
|---|---|---|---|
| **Add Base** | 8453 (built in) | none — a switch | `null`, `chainChanged 0x2105` |
| **(a) Add Sepolia — catalog** | 11155111 with a FAKE name "My Sepolia", symbol "SITE", RPC `https://rpc.invalid` | "127.0.0.1:8137 asks to add a network"; Name **Ethereum Sepolia**, coin **ETH**, RPC host from the catalog (never `rpc.invalid`); "Checking compatibility..." → "Compatible"; **Add Network** | approve → `null`, `chainChanged 0xaa36a7`; Settings › Networks lists Ethereum Sepolia. Cancel → 4001 |
| **(b-ok) Add 987654321 — local fork** | 987654321, RPC `http://127.0.0.1:8545` | Name "Local Sepolia fork", "Not in Vela’s network list — the name and coin are the site’s.", RPC `127.0.0.1:8545` → Compatible → Add Network | approve → `null`, `chainChanged 0x3ade68b1` |
| **(b-wrong) Add 987654321 — RPC on another chain** | 987654321, RPC `https://rpc.gnosischain.com` | "That RPC serves a different network (chain 100, expected 987654321)."; only Done | -32602 |
| **(b-none) Add 987654321 — http RPC** | 987654321, RPC `http://rpc.example.com` (public http) | "The site gave no usable RPC for this network"; only Done | -32602 |
| **(c) Add Ethereum Classic — incompatible** | 61 | Name Ethereum Classic; "Incompatible", the hint, "Open Chain Setup Tool"; only Done | 4902 "Chain 61 is not compatible with Vela Wallet: …" |
| any add twice quickly | — | the first sheet stays | the second: -32002 |

Each ending is a row in the tab's status (`Tab status` › Recent requests) and one log line, e.g.
`dapp tab=… req=… method=wallet_addEthereumChain class=consent outcome=failed code=4902 layer=wallet reason=not_compatible`.

## Public dApps

Any wagmi / RainbowKit dApp whose chain list includes a chain Vela lacks, e.g. the Safe{Wallet} app
or Uniswap's interface on a testnet: pick the network in the dApp's network menu → wagmi sends
`wallet_switchEthereumChain` (4902) → `wallet_addEthereumChain` → the sheet. A chain the dApp
lists that Vela already has switches with no sheet.

## Clients

- **Desktop**: the sheet is the third column (where "Connect to …" shows).
- **Android / iOS**: a non-dismissable bottom sheet over Explore, like the connect sheet.
- **Web (extension)**: the side panel or the request window, like a connect.
