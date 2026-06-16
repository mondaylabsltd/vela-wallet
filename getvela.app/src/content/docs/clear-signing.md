---
title: Clear signing
description: Vela decodes transactions into plain language before you approve them — intent, amounts, addresses, and risk — instead of opaque hex. No blind signing.
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

- When a **contract-specific descriptor** exists, the transaction is marked
  **verified** and labeled with the contract's name.
- When it doesn't, Vela falls back to **standard descriptors** for common shapes
  — ERC-20 tokens, ERC-721 NFTs, ERC-4626 vaults, and ERC-2612 permits — so most
  everyday actions still decode.

Token amounts are formatted using the token's **real on-chain decimals**. Vela
never just assumes 18; if it can't confirm the decimals, it shows the value but
**flags it as unverified** rather than guessing.

## Risk levels

Every decoded transaction gets a risk level so the dangerous patterns stand out:

- **Caution** for approvals and permits — you're granting spending power.
- **Danger** for the genuinely risky, like an **unlimited token approval**.
- Lower risk for routine actions like staking or depositing.

<Callout type="warning" title="Unlimited approvals are flagged">
An "approve" that grants an unlimited allowance is one of the most common ways
funds get drained later. Vela marks these explicitly so you can choose a specific
amount instead.
</Callout>

## When Vela can't decode a call

Honesty matters more than a clean screen. If no descriptor matches — or Vela can
only decode part of a transaction — it does **not** pretend to understand it.

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
