---
title: How passkeys work
description: The security model behind Vela — what a passkey is, where your key lives, and why there's nothing to phish.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# How passkeys work

Vela's whole security model rests on one idea: the key that controls your wallet
is a **passkey**, created by your device and held by your OS — no app, including
Vela, can read it — and used only with your face or fingerprint.

## What a passkey actually is

A passkey is a public/private key pair created by your device. The **private
key** is held by your OS passkey provider — iCloud Keychain on Apple, Google
Password Manager on Android — stored end-to-end encrypted, so no app can read or
copy it. Apps don't get the key; they get to *ask your device to sign something*
after you authenticate.

This is the same technology that protects Apple Pay and your biometric unlock.

<Callout type="info" title="Key point">
An app — including Vela — can request a signature, but it never sees your private
key. Your face or fingerprint authorizes your device to sign; the key itself
stays with your OS, end-to-end encrypted.
</Callout>

## Why there's nothing to phish

Phishing works by getting you to hand over a secret. With a seed phrase, that
secret is twelve words you can type into a fake page. With a passkey, **there is
no secret you can type**. A scam site cannot ask you to "enter your passkey,"
because a passkey isn't enterable — it's a hardware operation gated by your
biometrics.

That removes the single most common way people lose self-custodied funds.

## How signing a transaction feels

1. You confirm a transaction in Vela.
2. Your device prompts for Face ID / Touch ID.
3. Your device signs the transaction with your passkey.
4. Vela broadcasts the signed transaction to the network.

Same gesture as unlocking your phone — because it's the same passkey mechanism
your device already uses everywhere else.

<Callout type="warning" title="Device security still matters">
A passkey protects against remote attacks and phishing extremely well. It does
not protect against someone who has your unlocked device and can pass your
biometric check. Keep a device passcode set and don't hand an unlocked phone to
someone you don't trust.
</Callout>

## Where the rest lives

Your passkey's **public** key is published to a small on-chain index so your
wallet can be recovered on a new device. That's the subject of the next page:
[recovery & sign-in](/docs/recovery).
