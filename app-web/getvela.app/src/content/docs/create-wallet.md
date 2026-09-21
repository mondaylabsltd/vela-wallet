---
title: Create your wallet
description: "Create a Vela wallet with one to seven keys — what each step does, why the keys are fixed at creation, what becomes public, and what your wallet actually is."
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Create your wallet

Creating a wallet takes a minute or two. Open the web wallet at
[wallet.getvela.app](https://wallet.getvela.app/) — or the extension, desktop or
phone app — and choose **Create a wallet**.

## Steps

1. **Name your wallet.** The name helps you recognise it, and it is written to a
   public registry with your keys — treat it as public, and don't put anything
   private in it.
2. **Confirm what happens.** You tick that your public keys and wallet name are
   written on-chain, that your private keys stay in your devices or security
   keys, and that you agree to the [terms](/terms) and
   [privacy policy](/privacy).
3. **Create your first key.** Choose how: **this device** (Face ID, Touch ID,
   fingerprint, Windows Hello), **a phone or tablet** (scan a QR code and create
   it there, where the app offers it), or a **USB security key**. Your device creates the passkey and then
   signs once with it, so the app knows the key really works before going on.
4. **Add more keys, if you want.** Up to seven in total, of any kind. Any one of
   them will be able to sign on its own. If your only key isn't synced anywhere —
   a security key, or Windows Hello — the app asks for a second one, because a
   single unsynced key is one lost device away from a lost wallet.
5. **Create.** The app computes your wallet's address from the full set of keys
   and publishes the set to the public registry on Gnosis Chain. When that record
   is on-chain, your wallet opens.

<Callout type="warning" title="Choose your keys now">
Your address is computed from the keys you finish with, so keys can't be added,
removed or replaced later. [Signers & security keys](/docs/signers) explains
why, and how to choose.
</Callout>

## What your wallet is

Your wallet is a **Safe smart account** — a contract, not a plain account with one
private key. Your keys are its owners, and any one of them can authorize a
transaction. [The account contract](/docs/account-contract) lists every contract
involved.

The address is **the same on every network**, and it is **counterfactual**: it is
computed before anything is deployed, so you can receive funds on any network
right away. The contract deploys itself the first time you send from a network,
and that first transaction's fee includes the deployment. Creating the wallet
costs you nothing.

## What is public

<span id="what-is-public"></span>

Creating a wallet writes a permanent record to a public registry contract on
Gnosis Chain, readable by anyone and impossible to edit or delete:

- each key's **public key** (never the private key) and its **credential ID**;
- each key's **authenticator model** (which password manager or security key
  made it) and flags saying whether you were verified and whether the key is
  synced;
- the **wallet name** and a **label for each key**;
- the **wallet address** and when it was created;
- the **signed registration data** itself.

Vela's public-key index submits the record and pays its gas, so it sees the record
first. Nothing in it can move your funds; it is what lets any one of your keys
find the wallet again on a new device ([recovery](/docs/recovery)). The
[privacy policy](/privacy) has the complete list, and the [registry page](/registry)
shows every record.

## Next steps

- [Receive your first tokens](/docs/send-and-receive)
- [Understand networks and fees](/docs/networks-and-fees)
- [What to do if you lose a device](/docs/recovery)
