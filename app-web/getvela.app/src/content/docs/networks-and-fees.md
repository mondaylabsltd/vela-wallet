---
title: Networks & fees
description: "The 20 networks Vela shows by default, how to add another, exactly how a transaction's fee is calculated and who receives it, and what happens when a relay runs out of gas."
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Networks & fees

<Callout type="info" title="Last verified November 2026">
Checked against the code at commit <code>fa6083c87</code>. The fee rule is
<code>fee_policy.rs</code> in Vela's shared core; where this page and the code
disagree, the code is right.
</Callout>

## Built-in networks

Vela shows **20 networks** by default, all mainnets:

| Network | Gas paid in | Network | Gas paid in |
| --- | --- | --- | --- |
| Ethereum | ETH | X Layer | OKB |
| BNB Chain | BNB | Mantle | MNT |
| Base | ETH | Gnosis | xDAI |
| Polygon | POL | Tempo | pathUSD (no native coin) |
| Arbitrum | ETH | World Chain | ETH |
| Avalanche | AVAX | Celo | CELO |
| Optimism | ETH | Unichain | ETH |
| Monad | MON | Ink | ETH |
| Robinhood Chain | ETH | Soneium | ETH |
| Arc | USDC (the native coin) | Kaia | KAIA |

Stable, MegaETH, Plume and XRPL EVM are still supported, hidden by default:
add them back in **Settings → Add network**.

On most of them you can also pay the fee in a USD stablecoin the relay accepts
on that network (see below).

Your wallet has the **same address on every network**, because the address is
computed from your keys, not from the chain.

## Adding another network

You can add any EVM network under **Settings → Networks**, provided it has what
a Vela wallet needs: twelve standard contracts (the ERC-4337 EntryPoint v0.7,
the Safe v1.4.1 contracts, Safe's 4337 and passkey modules, MultiSend,
Multicall3 and two deterministic deployers) and the **EIP-7951 / RIP-7212** precompile that
verifies passkey signatures at address `0x100`. The wallet checks all of them,
including a real signature check against the precompile, before it lets you
add the network. The precompile has two names: EIP-7951 on Ethereum, live since
the Fusaka upgrade (December 2025), and RIP-7212 on rollups. The interface is the
same, and the wallet accepts either.

The precompile is a hard requirement. Its address is part of how every Vela
address is computed, so there is no fallback verifier and no way to deploy one
later. If a chain has the precompile but is missing some of the contracts,
[chain setup](/chain-setup) shows what is missing and deploys what anyone can
deploy. Two of the twelve contracts it checks — Safe's passkey signer factory
and the signer code that factory deploys — matter only to a wallet holding more
than one key, and the check says so per contract: without them a one-key wallet
works normally, while a wallet whose address came from two to seven keys cannot
be deployed on that network at all.

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

<span id="fee"></span>

The confirm screen shows one amount, in the fee coin and in your display
currency. How it is calculated depends on what the relay publishes.

**Where the relay bills the gas used** (Ethereum, Gnosis, Polygon, BNB Chain and
their testnets, with Vela's relay):

- **Gas billed (`settlementGas`).** With its quote the relay says how much gas it
  will bill for this operation: what its simulation measured plus 15% and
  30,000, and never more than the operation's own gas limits.
- **Price per gas (`inBandFeePerGas`).** The relay publishes one per speed:
  1.1 × (1.5 × the next block's base fee + a tip) for *slow* and *standard*, with
  1.75 × the base fee for *fast*, the tips being the 25th, 50th and 70th
  percentile of recent blocks' rewards. The default speed is *standard*.
- **Fee = `settlementGas × inBandFeePerGas`**, with a minimum of about $0.01.

The wallet does not take either figure on trust. It refuses a quote whose
`settlementGas` is above the smaller of the gas limits the operation is signed
with and 125% of the wallet's own gas model plus 50,000, and a price per gas more
than three times the same formula applied to the wallet's own reading of the
network. Nothing is signed on a refused quote.

**Against a relay that publishes neither figure** (other networks, or an older
relay), the fee is **3 × the operation's gas limits × the higher of the wallet's
own gas-price reading and the relay's price for the chosen speed**, with the same
minimum. On Tempo the multiple is 2 and the fee is paid in pathUSD.

Either way the fee is more than the transaction costs on-chain — more again for
your first transaction on a network, which also deploys your wallet. The relay
pays the real cost and keeps the rest; nothing is refunded. On cheap networks
this is cents; on Ethereum mainnet it can be a real amount. You never have to
guess: the exact amount is on the confirm screen before you sign, it is rounded
up, never down, and from the moment you confirm, the amount and fee you pressed
are the ones signed — if a new quote arrives in between, nothing is signed and
the screen says the fee moved.

**Who gets it.** The fee goes to whoever runs the relay the wallet is set to —
Vela's unless you change it. Any vela-relay deployment works, including
[one you run yourself](/docs/self-hosting#relay), and the wallet uses the same
formula whichever relay you pick.

<Callout type="info" title="What you see is what you pay">
The fee amount and the address it goes to are part of the operation you sign. A
relay that changed either would invalidate your signature, so you pay exactly
the amount shown — no more, even if gas rises before inclusion. A relay's
price per gas more than three times the wallet's own reading is refused, and so
is a bill for more gas than the operation can use.
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

The relay receives an operation that is already signed. It cannot change the
recipient, the amount, the fee or anything else. It can delay or refuse it, and it
chooses when it lands — so for a swap it could, in principle, trade ahead of you
within your slippage. Every operation is signed with a **30-minute** validity
window: after that the chain refuses it, so a relay cannot hold a signed
operation and land it later. If one expires, the wallet says nothing was sent,
and you sign again.

### When a relay runs out of gas

A relay pays gas from its own **treasury** on each network. If that treasury is
empty, the send stops at **Continue** — before anything is signed — and shows
the treasury's address with a button to copy it (and, in the web wallet, a QR
code), how much it has against how much it needs, and in which coin:

- On a network Vela's relay serves, the relay's operator (Vela) needs to top it
  up; you can report it. If you can't wait, you can **optionally** send a small
  amount of the native coin to the treasury yourself. That contribution is
  **non-refundable** and does **not** pay for your own transaction.
- On a network you added, funding the relay is up to whoever runs it — which may
  be you.

The wallet checks again every few seconds while that screen is open. Once the
treasury is funded it carries on by itself; on the confirm screen it only closes
the notice, because signing is always yours to start.

There is no per-wallet gas account and no activation deposit: an earlier version
of Vela had one, and it no longer exists.

### When a relay can't reach a network

A relay can only serve a network it can read. When it can't, the send stops at
**Continue** and says so — funding would not help.

- On a network Vela ships, that is the operator's to fix; you can report it.
- On a network you added, the relay reads it through the RPC address you set for
  it, so that address has to be a public `https` one. A node on your own computer
  or local network (`localhost`, `192.168.…`) can only be served by a relay
  running beside it — see [running your own relay](/docs/self-hosting#relay).

## How Vela reads each network

Vela reads balances and simulates transactions through a **pool of RPC
endpoints** per network — the built-in ones, public fallbacks, and any provider
keys or endpoints you add — and moves to the next one when an endpoint is slow
or down. You can set your own endpoint per network under **Settings →
Networks**, in every app.

A balance is one node's answer. A **signing check** is not: a simulation, a
multisig's safety checks and an ownership check count only when two endpoints
from **different providers** agree, each having first reported the right chain.
One node answering, or two disagreeing, turns the confirm into "… anyway", held
for 1.5 seconds. With **Only use my node** on, your own node is trusted alone,
and the confirm screen says so. Providers are told apart by their domain name,
so two brands that share one backend would count as two.

The relay is sent the RPC address the wallet uses for a network — including any
API key in it — with every request, so it can reach the network you use. The
wallet says so where you set an address or a provider key; see the
[privacy policy](/privacy).

Next: [how passkeys work](/docs/passkeys).
