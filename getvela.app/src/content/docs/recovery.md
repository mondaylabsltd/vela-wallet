---
title: Recovery & sign-in
description: How Vela lets you recover your wallet on a new device without a seed phrase — and the honest limits of that model.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Recovery & sign-in

The hardest part of a no-seed-phrase wallet is recovery: if there's no twelve
words, how do you get back in on a new phone? Here's exactly how Vela handles it.

## How it works

When you create a wallet, two things are published to Vela's **passkey index**:

- Your passkey's **public key** (never the private key).
- The **name** you chose for the wallet.

The public key is stored on the Gnosis blockchain via a smart contract, so it's
publicly readable and not dependent on Vela's servers staying up.

Your **private** key, meanwhile, is a passkey synced by your platform keychain —
**iCloud Keychain** on Apple devices, **Google Password Manager** on Android.

To sign in on a new device:

1. Sign in to the same iCloud or Google account, with keychain sync enabled.
2. Open Vela and choose to sign in.
3. Authenticate with your passkey. Your platform provides the synced passkey; the
   index provides the matching account. Your wallet is back.

<Callout type="info" title="Why split it this way">
The public key in the on-chain index lets anyone (including a fresh install) find
your account. The private key, synced by your trusted platform keychain, is what
actually authorizes transactions. Neither half alone can move your funds.
</Callout>

## The honest limits

Self-custody means the responsibility is real. Here's what to understand.

<Callout type="warning" title="Your recovery depends on your platform keychain">
Vela's cross-device sign-in relies on your passkey syncing through iCloud
Keychain or Google Password Manager. Keep that account secure and its recovery
options up to date. If you lose access to **both** your devices and your
platform-account keychain, Vela cannot regenerate your private key for you — by
design, we never had it.
</Callout>

Practical guidance:

- **Keep keychain sync on.** It's what carries your passkey between devices.
- **Secure your Apple / Google account** with a strong password and its own
  recovery methods. That account is now part of your wallet's safety.
- **Have more than one device signed in** where you can, so a single lost phone
  is an inconvenience, not a crisis.

## What Vela can and cannot do

- **Can:** help you find your account again via the public index.
- **Cannot:** move your funds, freeze your wallet, or recover a private key. Vela
  never holds it. That's the whole point of self-custody — and the trade you're
  making for it.
