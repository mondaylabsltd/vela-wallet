---
title: How passkeys work
description: "What a passkey is, where the private key lives for each kind of key, why there is no secret to phish, and what a passkey does not protect you from."
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# How passkeys work

The keys that control a Vela wallet are **passkeys**: WebAuthn credentials on
the P-256 curve. Your device or security key creates each one, keeps the private
key, and uses it only after you confirm with Face ID, a fingerprint, your device
PIN, or a touch and PIN on a security key. Vela never receives the private key;
if a password manager syncs it, the manager holds it, encrypted, on your behalf.

## What a passkey is

A passkey is a public/private key pair made for one website — for Vela,
`getvela.app`. An app never receives the private key; it can only ask the
authenticator to sign something, and the authenticator asks you first.

Where the private key lives depends on the kind of key:

| Kind of key | Where the private key lives | Synced to other devices? |
| --- | --- | --- |
| **This device** — Face ID, Touch ID, fingerprint, Windows Hello | Your platform's password manager (iCloud Keychain, Google Password Manager) or a password manager such as 1Password | Usually, end-to-end encrypted, if sync is on. Windows Hello keys stay on the PC |
| **Another phone**, reached by scanning a QR code | That phone's password manager | As above |
| **A hardware security key** (YubiKey and other FIDO2 keys, over USB or NFC) | Inside the security key | Never |

A Vela wallet can use up to seven keys of any mix, chosen when you create it;
[signers & security keys](/docs/signers) covers that choice.

## No secret to phish

Phishing works by getting you to hand over a secret. A seed phrase is twelve
words you can be talked into typing somewhere. A passkey has **no secret you can
type**: there is nothing to reveal, nothing to paste, and a fake site cannot ask
for it. And because a passkey is made for one website, your browser offers a
`getvela.app` passkey only to pages on getvela.app and its subdomains.

That removes a whole class of loss — the stolen recovery phrase — which is
common in self-custody.

## What a passkey does not protect you from

<Callout type="warning" title="A passkey signs whatever you approve">
The prompt from your phone or browser says <em>which</em> key is being used, not
<em>what</em> is being signed. A passkey will sign a harmful transaction as
readily as a good one if you approve it. That is why Vela decodes every
transaction before you sign (<a href="/docs/clear-signing">clear signing</a>),
and why the page showing it matters (<a href="/docs/bybit-attack">the Bybit
attack</a>).
</Callout>

It also does not protect against someone who has your unlocked phone and can
pass its check, or who controls the account your passkey syncs through. Keep a
device passcode set, secure your Apple or Google account, and consider a
hardware security key that syncs nowhere.

## What signing feels like

1. You confirm a transaction in Vela, after reading what it does.
2. Your device or security key asks for Face ID, a fingerprint, your PIN, or a
   touch plus PIN.
3. It signs, and only the signature comes back to the app.
4. The app hands the signed operation to the relay, which submits it; your
   wallet contract checks the passkey signature on-chain before doing anything.

## Where the public key goes

Your keys' **public** halves are recorded in a public registry on Gnosis Chain,
so a new device can find your wallet. That is the subject of
[recovery & sign-in](/docs/recovery).

Next: [signers & security keys](/docs/signers).
