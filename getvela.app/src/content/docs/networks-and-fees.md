---
title: Networks & fees
description: The 8 networks Vela supports, how account-abstraction gas fees and the relayer work, and how Vela picks RPC endpoints.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Networks & fees

## Supported networks

Vela ships with **8 EVM networks** built in:

| Network | Native fee token |
| ------- | ---------------- |
| Ethereum | ETH |
| BNB Chain | BNB |
| Polygon | POL |
| Arbitrum | ETH |
| Optimism | ETH |
| Base | ETH |
| Avalanche | AVAX |
| Gnosis | xDAI |

Your wallet has the **same address on all of them**, so there's one address to
share everywhere.

You can also **add custom networks** (Settings → Networks). Because Vela is a
smart-account wallet, a network has to provide the contracts Vela relies on —
the ERC-4337 EntryPoint, the Safe contracts, and the **P-256 (RIP-7212)**
signature precompile that verifies your passkey on-chain. Vela checks this
automatically before letting you add a network.

<Callout type="info" title="Why Gnosis shows up a lot">
Beyond being one of the 8 networks, Gnosis Chain hosts Vela's **Passkey Index** —
the contract that stores your public key and account name for cross-device
recovery. That's separate from which network you transact on.
</Callout>

## How fees work (account abstraction)

Vela uses **ERC-4337 account abstraction**, so a transaction isn't broadcast by
you directly — it's a **UserOperation** submitted by a **bundler**. A few things
follow from that:

- **Gas is paid from your own wallet's balance** in the network's native token
  (ETH, BNB, xDAI…). There is **no paymaster** and no third party sponsoring — or
  gating — your transactions.
- The **relayer fee is a transparent markup** over the on-chain gas price, shown
  in full on the confirmation screen before you sign. You can pick a speed tier
  (Slow / Standard / Rapid / Fast).
- The confirm screen breaks the fee down: on-chain gas price, UserOp gas price,
  gas limit, the fee in the native token, and the fee in your display currency.

<Callout type="warning" title="Activating a network's gas account">
Each network uses a dedicated **gas account** (relayer) for your wallet.
Activating it takes a small, **non-refundable** deposit (Vela may sponsor this for
new users). It's designed to top itself up from gas refunds — but that account
can still run down over time, so you may need to **re-activate it again later**
when it's depleted. It isn't strictly a one-time payment. The relayer address can
also change on a service upgrade, which needs a fresh activation. Aside from
activation, each transaction's gas comes out of your own wallet balance.
</Callout>

Because the fee is always paid in the **native** token, you need a small native
balance to send anything — including to move an ERC-20. If a send is blocked for
gas, it means you hold the token but not enough native token to cover the fee.

When you send the **maximum** amount of a native token, Vela automatically
reserves enough for gas so the transaction doesn't fail.

## How Vela talks to each network

Vela reads balances and submits transactions through a **pool of RPC endpoints**,
not a single provider. It gathers endpoints from several sources, scores them by
latency and reliability, and **fails over automatically** when one is slow or
down — temporarily benching bad endpoints — so a single flaky node never takes
the app offline.

Next: [how passkeys work](/docs/passkeys).
