---
title: Networks & fees
description: The 12 networks Vela supports, how account-abstraction gas fees work, who runs the relay and collects the fees, when you self-fund gas-account activation, and how Vela picks RPC endpoints.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Networks & fees

## Supported networks

Vela ships with **12 EVM networks** built in:

| Network     | Native fee token |
| ----------- | ---------------- |
| Ethereum    | ETH              |
| BNB Chain   | BNB              |
| Polygon     | POL              |
| Arbitrum    | ETH              |
| Optimism    | ETH              |
| Base        | ETH              |
| Avalanche   | AVAX             |
| Gnosis      | xDAI             |
| Unichain    | ETH              |
| Tempo       | USD              |
| Monad       | MON              |
| World Chain | ETH              |

Your wallet has the **same address on all of them**, so there's one address to
share everywhere.

You can also **add custom networks** (Settings → Networks). Because Vela is a
smart-account wallet, a network has to provide the contracts Vela relies on —
the ERC-4337 EntryPoint, the Safe contracts, and the **P-256 (RIP-7212)**
signature precompile that verifies your passkey on-chain. Vela checks this
automatically before letting you add a network.

<Callout type="info" title="Why Gnosis shows up a lot">
Beyond being one of the 12 networks, Gnosis Chain hosts Vela's **Passkey Index** —
the contract that stores your public key and account name for cross-device
recovery. That's separate from which network you transact on.
</Callout>

## How fees work (account abstraction)

Vela uses **ERC-4337 account abstraction**, so a transaction isn't broadcast by
you directly — it's a **UserOperation** handed to a **relay**, which submits it
on-chain and is reimbursed for the gas. (The ERC-4337 spec calls that role a
*bundler*. Vela's is called a relay because it does more than bundle: it quotes
fees in band and runs the gas-account protocol below, neither of which is part
of the standard.) A few things follow from that:

- **Gas is paid from your own wallet's balance** — in the network's native token
  (ETH, BNB, xDAI…) by default, or in a supported stablecoin where the relay
  offers one; you pick the fee asset on the confirm screen. Tempo has no native
  coin, so gas there is always settled in USD stablecoins. There's no ERC-4337
  **paymaster** sponsoring — or gating — each transaction. (Vela may sponsor the
  one-time _gas-account activation_ for new users; that's separate, and covered
  below.)
- The **relay quotes the gas price** — it is the single source of truth, and the
  wallet displays that quote and signs exactly what it shows. There is no speed
  picker: every transaction is submitted at high priority.
- The total charge is the **network cost plus the relayer's service fee**, with
  a small minimum charge on very cheap transactions. The relay's quote is the
  price — there is no separate fee schedule to consult. One part goes to the
  chain's validators; the rest pays the relayer that fronts the gas and runs
  the infrastructure.
- The confirm screen shows the **estimated fee** in the fee asset and in your
  display currency before you sign. The quoted amount and its recipient are part
  of what you sign, so the relayer is paid exactly what was shown — a changed
  number would invalidate your signature.

## Who runs the relay — and who gets the fees

Every network points at a relay. By default that's **Vela's own relay**, and
you can replace the endpoint under _Settings → Advanced → Service Endpoints_.
One endpoint applies to every built-in network; a custom network keeps the
relay URL you gave it when you added it.

An honest caveat about compatibility: the app quotes fees through a
Vela-specific RPC method (`vela_getInBandGasQuote`), and the send flow fails
without it. So the endpoint you point at must run
[vela-relay](https://github.com/mondaylabsltd/vela-relay) — Vela's instance or
one you host yourself. A generic ERC-4337 bundler such as **Pimlico** or
**Alchemy** doesn't implement that method, so it won't work end to end in the
current release.

Whoever operates the relay for a network **collects that network's fees** —
the relayer markup on every transaction and the gas-account activation deposit.
Run your own vela-relay and those fees fund your infrastructure instead of
Vela's; Vela takes no cut on traffic you route elsewhere.

<Callout type="warning" title="The gas account is part of the vela-relay protocol">
The **gas-account activation** step funds a dedicated relayer account for your
wallet on each network. If you point the endpoint at a self-hosted vela-relay,
the deposit funds your own relay's account, not Vela's.
</Callout>

### Activating the gas account (Vela Relay)

On Vela's relay, your first transaction on each network **activates a dedicated
gas account**. The app first asks the relay's treasury to fund it for you —
this happens silently inside the send flow, and a sponsored wallet never sees a
funding screen. Only when sponsorship is declined does the app show a top-up
request: you send a small amount of the native token to the gas-account address
it displays, and it tells you why sponsorship wasn't available.

**You pay the activation fee yourself** whenever free sponsorship isn't offered —
namely when:

- **Vela's treasury for that network is empty or low** — the free fund is
  temporarily depleted on that chain.
- **You've used up the free quota** — sponsorship is capped per wallet, so beyond
  the first few it's self-funded.
- **Vela's relay doesn't fund that network at all** — e.g. **custom or test
  networks you added yourself**, which Vela holds no treasury for. (Route these to
  your own relay if you'd rather skip activation entirely.)

The activation deposit is **non-refundable** — it's the relayer's starting balance
and tops itself up from gas refunds over time, though it can still run down and
need **re-activating** later. The relayer address can also change on a service
upgrade, which needs a fresh activation.

The fee comes out of your balance in the **fee asset** you picked — the native
token by default. If a send is blocked for gas, it means your balance in that
fee asset can't cover the fee; where the relay offers stablecoin gas, switching
the fee asset on the confirm screen can unblock it.

When you send the **maximum** amount of a native token, Vela automatically
reserves enough for gas so the transaction doesn't fail.

## How Vela talks to each network

Vela reads balances and submits transactions through a **pool of RPC endpoints**,
not a single provider. It gathers endpoints from several sources, scores them by
latency and reliability, and **fails over automatically** when one is slow or
down — temporarily benching bad endpoints — so a single flaky node never takes
the app offline.

Next: [how passkeys work](/docs/passkeys).
