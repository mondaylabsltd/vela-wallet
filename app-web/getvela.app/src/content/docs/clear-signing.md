---
title: Clear signing
description: Vela decodes transactions into plain language before you approve them — intent, amounts, addresses, and risk — instead of opaque hex. When it can't decode a call, it warns you instead of pretending.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Clear signing

Many wallets still show raw data for any contract they don't recognise, and
"blind signing" — approving calls you can't actually read — is one of the ways
wallets get drained. Vela's answer is **clear signing**: before you sign, the
transaction is decoded into something you can understand, as far as it can be.

## What you see

Instead of raw calldata, Vela shows:

- **Intent** — what the transaction does: *Send*, *Approve*, *Swap*, and so on.
- **The substance** — the amounts and the addresses involved, with token amounts
  shown in real units and recipients resolved to a name where one exists.
- **The details** — nonce, deadlines, and the raw calldata, available on demand
  rather than shoved in your face.
- **A risk indication**, color-coded so that dangerous actions stand out.

## How it works (ERC-7730)

<Callout type="info" title="Last verified November 2026">
Checked against the code at commit <code>fa6083c87</code> (the development
branch). Where the app and this page disagree, the app is right.
</Callout>

Vela decodes both **contract calls** and **EIP-712 typed data** using
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry)
descriptors — small, shareable definitions of what a contract's functions mean.

Vela looks for a descriptor in this order:

1. **Built into the app** — descriptors for widely used contracts: the Uniswap,
   PancakeSwap and SushiSwap routers, WETH, the Aave v3 pool, 1inch, Lido and
   wstETH, and Seaport.
2. **Fetched from Vela's chain-data server**, which republishes the public
   ERC-7730 registry.
3. **Standard shapes** — ERC-20 tokens, ERC-721 and ERC-1155 NFTs, ERC-4626
   vaults and ERC-2612 permits — so most everyday actions still decode.

**Verified** is reserved for the first source. A transaction is labelled verified
only when the description came from a descriptor built into the app you are
running — or from the chain-data server and is identical to the built-in copy,
which proves nothing was changed in transit. Anything else the server sends is
still decoded and still shown, with a line saying it came from the descriptor
service and nothing authenticated it. That service is not signed, so it is only
as trustworthy as whoever runs it — which is one reason you can
[run your own](/docs/self-hosting#chain-data).

Token amounts are formatted using the token's **real on-chain decimals**. If Vela
can't confirm a token's decimals, it shows the amount as if the token had 18 and
**marks it unverified**, so a wrong number never looks like a checked one.

## Risk levels

Every decoded transaction gets a risk level so the dangerous patterns stand out:

- **Caution** for approvals and permits — you're granting spending power.
- **Danger** for the genuinely risky, like an **unlimited token approval**.
- Lower risk for routine actions like staking or depositing — but only when the
  description is one built into the app. A description fetched from the
  chain-data server, or recovered from a selector database, is never shown as
  lower risk than normal, whatever it says the call does, so a tampered
  description can't paint a drain as safe.

A signature request is not automatically harmless. A typed-data (EIP-712)
message can be a permit, an order or a sign-in, and can move assets once
someone submits it; it gets the same decoding and the same checks.

<Callout type="warning" title="An 'unlimited' approval is shown in red, with a cap on offer">
An on-chain approval for an unlimited amount is one of the most common ways funds
get drained later. When a dApp asks for one (<code>approve</code>,
<code>increaseAllowance</code>, or Permit2's <code>approve</code>) at the
"unlimited" level — 2^128 or more (2^200, or 2^152 for Permit2, in v0.9.8 and
earlier), which is what dApps use for "unlimited" — Vela shows it in red and
offers a cap: a specific amount, your
balance (when it can be read), or — except on <code>increaseAllowance</code> — a
revoke. Unless you pick one, it is sent exactly as the dApp built it: Permit2 is
designed around a standing approval, and a smart-account batch spends the
allowance in the same transaction, so a cap below that spend makes the whole batch
fail. Inside a batch, each unlimited approval gets its own cap the same way (an
amount or a revoke), and keeping one unlimited approval never covers another. An
approval hidden inside a <code>multicall</code>, a Multicall3 call or a Safe
<code>multiSend</code> is found the same way. A last check before submission
reads the raw calldata, so an unlimited approval the approval screen never showed
can't go out. A <strong>large finite approval</strong> is shown with a caution,
and one for a hundred times your balance or more is called out in red. <strong>Signed permits</strong>
(EIP-2612 and Permit2 signatures) can't be capped — the dApp submits its own
copy — so they are signed as asked or rejected: an unlimited one in red, a limited
one with a caution. A request to grant an NFT <code>setApprovalForAll</code> for a
whole collection can't be approved in the apps yet.
</Callout>

## Simulation, and "… anyway"

Before you sign a transaction, Vela simulates it and shows what would move in
and out of your account. The result counts only when **two RPC providers from
different companies agree** on it — or your own node, if you turned on "Only use
my node" for that network. A single node could otherwise tell you "no asset
changes" for a transaction that empties your account.

When Vela could not check something — the simulation didn't come back, the
network doesn't offer one, only one node answered, two nodes disagreed, the
simulation says the transaction would fail, or a call can't be decoded — the
confirm button says **"… anyway"**, is styled as a danger, and has to be **held
for 1.5 seconds** (with a screen reader, press it twice). **Refuse blind
signing**, in Settings → Security, refuses every "… anyway" instead; it is off
by default because on networks with a single provider it would refuse ordinary
transactions.

## When Vela can't decode a call

When no ERC-7730 descriptor exists but
the function appears in a public selector database, Vela decodes the call
generically and labels it **best effort** — decoded, but not verified — under a
caution banner. If even that fails, or Vela can only decode part of a
transaction, it does **not** pretend to understand it.

<Callout type="danger" title="Explicit blind-sign warning">
If a call can't be decoded, Vela shows a clear blind-sign warning instead of a
fake-friendly summary. If it can only resolve some of the fields, it tells you
the view is partial and keeps the risk level elevated. You always know how much
of what you're signing Vela could actually read.
</Callout>

## Why this matters

Self-custody means no one can reverse a bad transaction for you. The defense
isn't a support desk — it's understanding what you approve **before** you approve
it. Clear signing is how Vela tries to show you that, and it has limits: it can
only be as honest as the app showing it, which is why an
[independent check](/docs/clear-signing-self-host) matters. See the
[whitepaper](/docs/whitepaper) for where it fits in Vela's security model.
