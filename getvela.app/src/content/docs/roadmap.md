---
title: Roadmap
description: Where Vela is headed and why — organized around what matters most in a wallet you actually trust with your money.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Roadmap

<Callout type="info" title="Directions, not deadlines">
Vela is one idea — a wallet you fully own, unlocked by your face, with no seed
phrase. Everything below makes that idea more complete. We build in the open, so
plans are directional and may change. Want something here?
<a href="https://github.com/atshelchin/vela-wallet/issues">Open an issue</a>.
</Callout>

## Recently shipped

- **Clear Signing** — transactions show what they actually do (amount, recipient,
  intent) in plain language instead of raw hex, powered by ERC-7730 descriptors.
- **Payment-first home** — the home screen rebuilt around your activity and balances.
- **13-language app** with instant switching, plus local currency, date and number
  formatting.
- **Pull-to-refresh (VelaRefresh)** and **pending-until-confirmed** send status.
- **DApp Connect maturity** — inline pairing, batched calls (EIP-5792), and silent
  reconnect over WalletPair.

---

## See your money, completely

_The first thing a wallet owes you is an honest balance and history._

- **Catch every incoming transfer.** Today a plain native-coin deposit — and coins
  that arrive through an internal call, like an exchange withdrawal, a router, or a
  multisig — emits no on-chain log, so it can't appear in your activity on most
  networks. We're building a transfer service that traces blocks to surface these,
  so every deposit and send shows up, on every chain. _(Building now.)_

## Use Vela everywhere you are

_The same self-custody wallet, on every device you own._

- **Native iOS & Android apps** in the App Store and Google Play. Vela runs on the
  web today; the mobile builds share the same code and are in real-device testing
  before release. _(Next.)_
- **Everything syncs across your devices.** Your accounts and networks already
  follow you; next, your language, currency and formatting — plus one-tap restore
  of your whole setup on a new device. _(Next.)_
- **A saved address book** so you stop re-pasting addresses. _(Next.)_
- **Connect to dApps from your desktop** without reaching for your phone. _(Exploring.)_

## Read everything before you sign

_Self-custody only means something if you can see what you're approving._

- **Wider clear-signing coverage** — more contracts and chains shown as
  human-readable intent, fewer blind-sign fallbacks. _(Building now.)_
- **An independent security audit** of Vela's Safe + WebAuthn integration, on top
  of the already-audited Safe contracts. _(Exploring.)_

## Work on the chains you use

_Your wallet should reach wherever you do._

- **More EVM networks**, plus a signing path for chains that don't yet ship the
  P-256 (RIP-7212) precompile your passkey relies on. _(Exploring.)_
