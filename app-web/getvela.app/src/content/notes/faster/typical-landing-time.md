---
title: "How long a send usually takes on each network"
nav: "How long a send takes"
description: "The receipt shows each network's usual time to land, from 2 seconds on Arc, Stable, Robinhood Chain and Plume to 24 seconds on Ethereum."
facts:
  - "Fastest | 2 s"
  - "Ethereum | 24 s"
  - "Taking longer than usual | After twice the usual time"
  - "Networks you add | No figure"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/network_admin.rs#L236-L241
  - rust/crates/vela-core/src/app/network_admin.rs#L272-L505
  - rust/crates/vela-core/src/app/network_admin.rs#L511-L522
  - rust/crates/vela-core/src/app/tx_tracker.rs#L152-L224
  - rust/crates/vela-core/i18n/locales/en/send.json#L58-L64
  - app-web/vela-wallet/src/lib/flows/live-send.ts#L1370-L1395
  - app-web/vela-wallet/src/lib/flows/screens/SendReceipt.svelte#L78-L93
  - app-web/vela-wallet/src/lib/flows/ui/StatusHero.svelte#L43-L47
docs: networks-and-fees
related:
  - transactions/a-timeout-is-not-a-failure
  - faster/one-transaction-at-a-time
draft: true
---

Once the relay has sent your transaction to the network, the receipt shows a line such as "Ethereum
typically confirms in ~24s" and counts down from that figure. Until then it says "Vela's relay is
sending it to the network…", and nothing is counted.

Each built-in network has one fixed figure: its block time multiplied by the number of blocks the
relay usually needs to get a transaction in. The figure is the same whichever speed you choose.

| Usual time | Networks |
| --- | --- |
| 2 s | Arc, Stable, Robinhood Chain, Plume |
| 3 s | Unichain, Monad, X Layer, MegaETH, Kaia, Celo, Ink |
| 4 s | Arbitrum, Optimism, Base, Avalanche, World Chain |
| 5 s | Tempo |
| 6 s | Polygon, Soneium, Mantle |
| 9 s | BNB Chain |
| 15 s | Gnosis |
| 18 s | XRPL EVM |
| 24 s | Ethereum |

## When it takes longer

Within the usual time, the line counts down the seconds left. After it, the line counts the seconds
that have passed. After twice the usual time, it says "Taking longer than usual, please wait...".
The transaction stays pending throughout. Only a failed receipt or a refusal from the relay marks
it as failed.

## Networks you add

A network you add yourself has no figure, because Vela has one only for the 24 networks it ships.
Its receipt says only "Waiting for blockchain confirmation...", with no countdown, and the progress
ring spins instead of filling until the transaction lands.
