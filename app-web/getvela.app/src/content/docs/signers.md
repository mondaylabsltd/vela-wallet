---
title: Signers & security keys
description: "A Vela wallet can have up to seven signers — passkeys, a nearby phone, or a YubiKey-class security key — and any one of them signs. They are chosen when the wallet is created, and this page explains why that is not a limitation we forgot to lift."
---

# Signers & security keys

A Vela wallet is a Safe, and a Safe has owners. Yours can have **up to seven**,
with a threshold of **one**: any single signer can authorize a transaction on
its own. That is written `1-of-n`.

## What can be a signer

Three kinds, and you can mix them freely:

| Method | What it is | Typical example |
| --- | --- | --- |
| **Platform** | The authenticator built into the device you are using | Face ID / Touch ID on this phone or laptop, synced by iCloud Keychain or Google Password Manager |
| **Nearby device** | Another device you reach by scanning a code | Your phone signing for your desktop, over the WebAuthn hybrid transport |
| **Security key** | A removable authenticator on USB or NFC | YubiKey and other FIDO2 keys |

All three are WebAuthn credentials on the **P-256** curve. To the Safe they are
indistinguishable: each is an owner whose signature Safe's passkey module
checks on-chain the same way. (The first key is verified by Safe's shared
signer; each additional key by its own small signer contract, created by Safe's
factory the first time the wallet is deployed on a chain.)

Which kinds each app can use:

| App | This device | Nearby phone (QR) | Security key |
| --- | --- | --- | --- |
| Web wallet, browser extension | Yes | Yes | USB or NFC, through the browser |
| Desktop (macOS, Windows, Linux) | macOS and Windows | Yes | USB |
| Android | Yes (with Google Play services) | Yes | USB |
| iOS | Yes | Yes | USB-C or Lightning YubiKey, firmware 5.8 or later |

The desktop app's "this device" option (Touch ID, Windows Hello) and its Windows
support in general are recent and less tested than the other routes; a phone or a
security key is the dependable choice there today.

A security key can be your **first** signer, not only a backup. If you would
rather your wallet never depended on an Apple or Google account at all, create it
with **two** security keys and keep one somewhere safe. (A wallet whose only key
isn't synced anywhere can't be created: the app asks for a second key, because
losing that one device would lose the wallet.)

## Why they are chosen at creation

This is the part that surprises people, so here is the mechanism rather than an
apology.

Your wallet address is **derived** from its owner set. Vela computes it with
`CREATE2` from the Safe setup data — which includes every signer's public key —
before anything is deployed on-chain. That is what lets you receive funds at an
address that does not exist yet.

The consequence is arithmetic, not policy: **a different set of keys is a
different address**. Adding an eighth signer later would not extend your wallet;
it would compute a new wallet, at a new address, with none of your money in it.

So the question "can I add a key later?" has two honest answers:

- **Before you fund it**: yes — the address has not been committed to anything,
  so create the wallet again with the keys you want.
- **After you fund it**: the address is where your money is. Safe itself can
  change owners on a chain where your wallet is already deployed — but on every
  chain where it isn't deployed yet, the same address still stands for the
  original keys, so the owner sets would drift apart from chain to chain. Vela
  doesn't offer owner changes for that reason. Plan the key set at creation.

## What this actually protects you from

**Losing a device.** With more than one signer, a lost phone is an
inconvenience: another key signs. With exactly one signer and OS sync switched
off, a lost phone is a lost wallet — which is why "your passkey syncs
automatically" is a description of a setting you control, not a guarantee we
can make on your behalf.

**A platform account you no longer trust.** If your passkey lives in iCloud
Keychain or Google Password Manager, whoever controls that account can
potentially use it. A security key is held by you and syncs nowhere.

And what it does **not** protect you from, because `1-of-n` cuts both ways:
adding a second key adds a second way *in*, not a second lock. Anyone who
obtains any one of your signers can sign alone. More keys means more resilience
against loss and more surface against theft; that is the trade, and it is yours
to make.

## Recovering, versus adding

These are different things and the docs keep them apart:

- [Recovery & sign-in](/docs/recovery) — getting back to an existing wallet on
  a new device with a key you already have.
- This page — deciding, up front, which keys exist at all.
