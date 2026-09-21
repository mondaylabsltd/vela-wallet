---
title: Clear signing
description: Vela decodes transactions into plain language before you approve them — intent, amounts, addresses, and risk — instead of opaque hex. When it can't decode a call, it warns you instead of pretending.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Clear signing

Most wallets ask you to approve a wall of hexadecimal and hope for the best.
"Blind signing" — approving calls you can't actually read — is behind a large
share of drained wallets. Vela's answer is **clear signing**: before you sign,
the transaction is decoded into something you can understand.

## What you see

Instead of raw calldata, Vela shows:

- **Intent** — what the transaction does: *Send*, *Approve*, *Swap*, and so on.
- **The substance** — the amounts and the addresses involved, with token amounts
  shown in real units and recipients resolved to a name where one exists.
- **The details** — nonce, deadlines, and the raw calldata, available on demand
  rather than shoved in your face.
- **A risk indication**, color-coded, so the scary stuff looks scary.

## How it works (ERC-7730)

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

When a descriptor written for that specific contract matches, the transaction is
labelled **verified** with the contract's name. "Verified" means *a descriptor
for this contract was found*, not that it was cryptographically checked:
descriptors fetched from the chain-data server are not signed, so they are only
as trustworthy as that server — which is one reason you can
[run your own](/docs/self-hosting#chain-data).

Token amounts are formatted using the token's **real on-chain decimals**. If Vela
can't confirm a token's decimals, it shows the amount as if the token had 18 and
**marks it unverified**, so a wrong number never looks like a checked one.

## Risk levels

Every decoded transaction gets a risk level so the dangerous patterns stand out:

- **Caution** for approvals and permits — you're granting spending power.
- **Danger** for the genuinely risky, like an **unlimited token approval**.
- Lower risk for routine actions like staking or depositing.

<Callout type="warning" title="Unlimited on-chain approvals can't be submitted">
An on-chain approval for an unlimited amount is one of the most common ways
funds get drained later. When a dApp asks for one (<code>approve</code>,
<code>increaseAllowance</code>, or Permit2's <code>approve</code>), Vela won't
submit it until you change it to a specific amount, your balance, or a revoke —
and a last check before submission refuses any approval that would still be
unlimited, reading the raw calldata so it works with or without a descriptor.
Two things it does not cap: <strong>signed permits</strong> (EIP-2612 and
Permit2 signatures), which are shown with a caution and signed as requested,
and NFT <code>setApprovalForAll</code>, which asks you to confirm the grant
deliberately.
</Callout>

## When Vela can't decode a call

Honesty matters more than a clean screen. When no ERC-7730 descriptor exists but
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
it. Clear signing turns "trust this opaque blob" into "here's exactly what this
does." See the [whitepaper](/docs/whitepaper) for where it fits in Vela's overall
security model.
