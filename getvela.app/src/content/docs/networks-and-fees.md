---
title: Networks & fees
description: The 8 networks Vela supports, how account-abstraction gas fees work, who runs the bundler and collects the fees, when you self-fund gas-account activation, and how Vela picks RPC endpoints.
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
you directly — it's a **UserOperation** handed to a **bundler**, which submits it
on-chain and is reimbursed for the gas. A few things follow from that:

- **Gas is paid from your own wallet's balance** in the network's native token
  (ETH, BNB, xDAI…). There's no ERC-4337 **paymaster** sponsoring — or gating —
  each transaction. (Vela may sponsor the one-time *gas-account activation* for
  new users; that's separate, and covered below.)
- The **bundler quotes the gas price** — it is the single source of truth, and the
  wallet uses that quote rather than marking the price up on its own. You can pick a
  speed tier (Slow / Standard / Fast); the tier changes how fast your transaction is
  included, not the fee policy.
- Vela's **relayer fee is set to roughly the network fee itself** — so you pay about
  **twice the raw on-chain cost**: one part to the chain's validators, one to the
  relayer that runs the infrastructure.
- The confirm screen **breaks the fee down honestly**: the on-chain *network fee*,
  the *Vela relayer fee*, and what *you pay* in total — plus the gas limit and the
  amount in your display currency. As a safety check, the wallet **refuses any quote
  above ~3× the network rate**, so a misbehaving or third-party bundler can't
  overcharge you.

## Who runs the bundler — and who gets the fees

Every network points at a bundler. By default that's **Vela's own bundler**, but
you can point any network at a third-party ERC-4337 bundler — **Pimlico**,
**Alchemy**, or your own self-hosted one — under *Settings → Networks → (network)
→ BUNDLER*. (Vela's backend is open source, so you can run the whole bundler
service yourself too.)

Whoever operates the bundler for a network **collects that network's fees** — both
the relayer markup on every transaction and, for Vela's bundler, the gas-account
activation deposit. So the choice decides where your fees go:

- **Stay on Vela's bundler** → activation deposit and relayer markup fund Vela's
  service.
- **Point a network at Pimlico / Alchemy / your own** → you transact under *their*
  pricing and pay *them* directly (typically via your own API key in the bundler
  URL). Vela takes no cut on networks you route elsewhere.

<Callout type="warning" title="Only Vela's bundler uses a gas account">
The **gas-account activation** step is specific to Vela's bundler — it funds a
dedicated relayer account for your wallet on each network. **Third-party bundlers
(Pimlico, Alchemy) don't use it**: they meter and bill gas their own way, so on a
network routed to them you never see the activation screen.
</Callout>

### Activating the gas account (Vela bundler)

On Vela's bundler, your first transaction on each network **activates a dedicated
gas account**. The app offers **Free Activation** — *sponsored by Vela for new
users* — and falls back to **Self Activate**, where you send a small amount of the
native token to the gas-account address shown in the app.

**You pay the activation fee yourself** whenever free sponsorship isn't offered —
namely when:

- **Vela's treasury for that network is empty or low** — the free fund is
  temporarily depleted on that chain.
- **You've used up the free quota** — sponsorship is capped per wallet, so beyond
  the first few it's self-funded.
- **Vela's bundler doesn't fund that network at all** — e.g. **custom or test
  networks you added yourself**, which Vela holds no treasury for. (Route these to
  your own or a third-party bundler if you'd rather skip activation entirely.)

The activation deposit is **non-refundable** — it's the relayer's starting balance
and tops itself up from gas refunds over time, though it can still run down and
need **re-activating** later. The relayer address can also change on a service
upgrade, which needs a fresh activation.

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
