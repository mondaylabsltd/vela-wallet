---
title: Recovery & sign-in
description: "How you get back into your wallet on a new device with any one of your keys, where the wallet is looked up, and the honest limits of recovery without a seed phrase."
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Recovery & sign-in

Without a seed phrase, recovery rests on two things: **a key you still have**,
and **a public record of which keys belong to your wallet**.

## What's recorded when you create a wallet

Your wallet's address is computed from all the keys you create it with. So that
any one of those keys can find the wallet again later, creating a wallet writes
a record to a public **registry contract** on Gnosis Chain: each key's public
key, the wallet's address, its name, and the signed registration data. The
registry has no owner and cannot be edited or deleted. (The full list of what is
public is on [create your wallet](/docs/create-wallet#what-is-public).)

Vela's public-key index service submits that record and pays its gas; the
record itself lives on-chain, and any app can read it directly.

## Signing in on a new device

1. Open Vela and choose to sign in.
2. Use **any one** of your keys: a passkey that synced to this device, a phone
   nearby (scan the QR code), or your security key.
3. Vela works out that key's public key from the signature, looks it up — first
   in Vela's index, then, if the index doesn't answer, in the registry contract on
   Gnosis and then on Ethereum — and rebuilds the wallet. It checks that the keys
   it found really compute to the recorded address before showing it to you.

Account lists aren't synced between devices; signing in rebuilds them.

<Callout type="info" title="If no index or registry can answer">
A wallet with a <strong>single key</strong> can be rebuilt on the device with no
server involved: two signatures from that key are enough to recover its public
key and recompute the address. A wallet with several keys needs the registry
record, because one key can't tell the app what the others were.
</Callout>

## Copies of the record

The registry on Gnosis is the one the apps read first. From **Settings**, you can
also copy your wallet's record to the same registry contract on **Ethereum**,
paying the gas yourself, so the record exists on a second chain. Anyone can make
such a copy; it contains nothing that can move funds.

## The honest limits

<Callout type="warning" title="A lost key is lost">
If every key you created the wallet with is gone — the synced passkeys, the
phones, the security keys — nobody can recover the wallet: not Vela, not Apple or
Google, not anyone. There is no seed phrase, no support reset and no back door.
</Callout>

What makes that unlikely is having more than one way in:

- **Keep passkey sync on** if you use this device's passkey. It's what carries
  the key to a new phone or computer.
- **Secure the account behind it.** Whoever controls your Apple or Google account
  may be able to use a synced passkey; give it a strong password and its own
  recovery options.
- **Create the wallet with more than one key**, for example your phone's passkey
  and a hardware security key kept somewhere safe. Keys can only be added when
  you create the wallet ([why](/docs/signers)). Remember that any single key can
  sign on its own.

## What Vela can and cannot do

- **Can:** keep the index running, so your wallet is found quickly on a new device.
- **Cannot:** move your funds, freeze your wallet, add or remove keys, or recover
  a key you have lost. Vela never holds your keys.

Next: [clear signing](/docs/clear-signing).
