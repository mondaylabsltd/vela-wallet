---
title: Networks & fees
description: "The 24 networks built into Vela, how to add another, exactly how a transaction's fee is calculated and who receives it, and what happens when a relay runs out of gas."
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Networks & fees

## Built-in networks

Vela has **24 networks** built in, all mainnets:

| Network | Gas paid in | Network | Gas paid in |
| --- | --- | --- | --- |
| Ethereum | ETH | Arc | USDC (the native coin) |
| BNB Chain | BNB | X Layer | OKB |
| Polygon | POL | Stable | USDT0 (the native coin) |
| Arbitrum | ETH | Soneium | ETH |
| Optimism | ETH | MegaETH | ETH |
| Base | ETH | Robinhood Chain | ETH |
| Avalanche | AVAX | Mantle | MNT |
| Gnosis | xDAI | Kaia | KAIA |
| Unichain | ETH | Celo | CELO |
| Tempo | pathUSD (no native coin) | Ink | ETH |
| Monad | MON | Plume | PLUME |
| World Chain | ETH | XRPL EVM | XRP |

On most of them you can also pay the fee in a USD stablecoin the relay accepts
on that network (see below).

Your wallet has the **same address on every network**, because the address is
computed from your keys, not from the chain.

## Adding another network

You can add any EVM network under **Settings → Networks**, provided it has what
a Vela wallet needs: eleven standard contracts (the ERC-4337 EntryPoint v0.7,
the Safe v1.4.1 contracts, Safe's 4337 and passkey modules, MultiSend,
Multicall3 and two deterministic deployers) and the **RIP-7212** precompile that
verifies passkey signatures at address `0x100`. The wallet checks all of them,
including a real signature check against the precompile, before it lets you
add the network.

The precompile is a hard requirement. Its address is part of how every Vela
address is computed, so there is no fallback verifier and no way to deploy one
later. If a chain has the precompile but is missing some of the contracts,
[chain setup](/chain-setup) shows what is missing and deploys what anyone can
deploy.

## How a transaction is paid for

Vela is an ERC-4337 wallet: you don't broadcast a transaction yourself. The app
builds a **UserOperation**, you sign it with one of your keys, and a **relay**
submits it on-chain and pays the gas up front. (ERC-4337 calls this role a
bundler.) The relay gets paid back **inside your operation**: the payment is a
transfer from your wallet to the relay that sits in the same batch as your
transaction, so it is covered by your signature. There is no paymaster, nobody
sponsors your gas, and nobody can refuse your transaction because of a
sponsorship policy.

### What the fee is

The confirm screen shows one amount, in the fee coin and in your display
currency. It is calculated like this:

- **Gas the wallet reserves.** The wallet simulates the transaction and reserves
  more gas than it expects to use: the verification and execution estimates are
  each raised by half, with minimums (for example, verification is at least
  300,000 gas once the wallet is deployed, and 2,000,000 for the transaction
  that deploys it).
- **Gas price.** The higher of the wallet's own reading of the network's gas
  price and the relay's price for the speed you chose. The default speed is
  *fast*, which the relay prices at about 1.8 × the base fee plus twice the
  priority fee.
- **Fee = 3 × reserved gas × gas price**, with a minimum of about $0.01. On Tempo
  the multiple is 2 and the fee is paid in pathUSD.

Because the reserve is padded and the price includes headroom, **the fee is
usually several times what the transaction actually costs on-chain**. The relay
pays the real cost and keeps the rest; nothing is refunded. On cheap networks
this is cents; on Ethereum mainnet, and for the first transaction that deploys
your wallet, it can be much more. The exact amount is on the confirm screen
before you sign.

<Callout type="info" title="What you see is what you pay">
The fee amount and the address it goes to are part of the operation you sign. A
relay that changed either would invalidate your signature, so you pay exactly
the amount shown — no more, even if gas rises before inclusion. A wallet quote
that is more than three times the wallet's own gas reading is refused.
</Callout>

### What you can pay with

- The network's **native coin**, always.
- A **USD stablecoin** from the relay's list for that network, when the relay can
  price the native coin. Stablecoins you hold none of are hidden.
- On **Tempo**, which has no native coin, **pathUSD** only.

You choose the fee coin, and the speed (*slow*, *standard* or *fast*), on the
confirm screen and in Settings.

### Your first transaction on a network

You can receive on any network before your wallet exists there. The first time
you send from a network, that transaction also deploys your wallet contract (and
one small signer contract for each extra key). The deployment gas is included in
that transaction's fee, so the first send on each network costs more than the
ones after it.

When you send the **maximum** of a native coin, Vela keeps back enough for the
fee.

## Who runs the relay — and who gets the fee

By default every network uses **Vela's relay**, and the fee goes to Vela. You can
point the wallet at a different relay under **Settings → Advanced → Service
Endpoints**; one address serves every built-in network, and a custom network
keeps the relay address it was added with. The relay must be
[vela-relay](https://github.com/mondaylabsltd/vela-relay) — Vela's or one you
run — because the wallet asks for its fee quote with a Vela-specific method that
generic bundlers such as Pimlico or Alchemy don't implement. Whoever runs the
relay you use receives the fee; the [self-hosting guide](/docs/self-hosting#relay)
explains how to run one.

The relay receives an operation that is already signed. It can delay it or
refuse it; it cannot change the recipient, the amount, the fee or anything else.

### When a relay runs out of gas

A relay pays gas from its own **treasury** on each network. If that treasury is
empty, the send screen tells you before you sign:

- On a network Vela's relay serves, the relay's operator (Vela) needs to top it
  up; you can report it. If you can't wait, you can **optionally** send a small
  amount of the native coin to the treasury yourself. That contribution is
  **non-refundable** and does **not** pay for your own transaction.
- On a custom network, funding the relay is up to whoever runs it — which may be
  you.

There is no per-wallet gas account and no activation deposit: an earlier version
of Vela had one, and it no longer exists.

## How Vela reads each network

Vela reads balances and simulates transactions through a **pool of RPC
endpoints** per network — the built-in ones, public fallbacks, and any provider
keys or endpoints you add — and moves to the next one when an endpoint is slow
or down. You can set your own endpoint per network under **Settings →
Networks**.

Next: [how passkeys work](/docs/passkeys).
