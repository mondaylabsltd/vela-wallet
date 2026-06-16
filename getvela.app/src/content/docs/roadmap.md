---
title: Roadmap
description: Where Vela is headed — recently shipped, in progress, and what's next. Directions, not deadlines.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Roadmap

<Callout type="info" title="Directions, not deadlines">
Vela is in alpha and shipping constantly. Plans here are directional and may
change — we build in the open, and every change lands in the
<a href="https://github.com/atshelchin/vela-wallet">public repo</a> first. Want
something on this list? <a href="https://github.com/atshelchin/vela-wallet/issues">Open an issue</a>.
</Callout>

## ✅ Recently shipped

- **Clear Signing** — transactions show what they actually do (amount, recipient,
  intent) in plain language instead of raw hex, powered by ERC-7730 descriptors.
- **Payment-first home** — the home screen rebuilt around your activity and balances.
- **13-language app** — instant language switching, with Russian and Italian most
  recently added.
- **Local currency & formatting** — choose your fiat currency and a configurable
  rates source; amounts, dates, and numbers follow your locale.
- **Pull-to-refresh (VelaRefresh)** — a branded, gesture-driven refresh across the app.
- **Clearer sends** — transactions show a pending state until they are confirmed
  on-chain.
- **DApp Connect maturity** — inline pairing, batched calls (EIP-5792), and silent
  reconnect over WalletPair.
- **One-tap feedback** — a Feedback entry in Settings opens a prefilled GitHub report.

## 🔨 Now — building

- **Wider clear-signing coverage** — more contracts and chains rendered in plain
  language, fewer blind-sign fallbacks.
- **Home & activity polish** — continued refinement of the payment-first layout.

## ⏭️ Next — planned

- **More recovery options** — additional ways to recover an account beyond
  cloud-synced passkeys.
- **DApp Connect on web** — desktop pairing currently relies on mobile Bluetooth;
  bringing it to the web build.

## 🔭 Later — exploring

- **Independent security audit** — a third-party review of Vela's Safe + WebAuthn
  integration. The Safe contracts themselves are already audited and battle-tested.
- **More networks** — additional EVM chains based on demand.
