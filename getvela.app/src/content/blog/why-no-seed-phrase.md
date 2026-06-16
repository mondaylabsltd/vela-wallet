---
title: Why Vela has no seed phrase
description: Seed phrases are the single biggest cause of lost crypto. Here's why we replaced them with passkeys, and what we gave up to do it.
date: 2026-06-09
author: Shelchin
tags: [security, passkeys, design]
---

Almost every self-custodial wallet hands you twelve words and tells you that
they are the most important thing you now own. Lose them and your money is gone.
Let someone see them and your money is gone. We decided Vela would not do that.

## The problem with twelve words

A seed phrase is a great cryptographic primitive and a terrible product. It asks
a normal person to do something they are bad at: store a high-value secret,
offline, perfectly, forever.

The failure modes are well known and they are everywhere:

- People screenshot the phrase, and the screenshot syncs to the cloud.
- People lose the piece of paper, or it's in a drawer in a house they no longer
  live in.
- People get tricked into typing the phrase into a fake "wallet support" page.

None of these are dumb mistakes. They're the predictable result of asking humans
to be password managers.

## Passkeys instead

A passkey is a public/private key pair created and stored by your device's
secure hardware — the same Secure Enclave or equivalent that guards Apple Pay and
your fingerprint. The private key never leaves that hardware. You authorize use
of it with your face or fingerprint.

For Vela, that means:

- There is no secret for you to store, because the secret never exists outside
  your device's secure element.
- There is nothing to phish. A fake support page cannot ask you to "type your
  passkey" — there's nothing to type.
- Signing a transaction feels like unlocking your phone, because it is the same
  gesture.

## What we gave up

Honesty matters more than marketing, so here's the trade-off.

Passkeys are newer than seed phrases, and the recovery story is different. A seed
phrase is portable to any wallet; a passkey is tied to your platform's keychain
(iCloud Keychain or Google Password Manager) and syncs through that. We lean on
that sync for cross-device access, and we publish a public key to a small index
so you can find your account again on a new device.

We think that's the right trade for most people, most of the time. The
[passkeys doc](/docs/passkeys) explains exactly how it works, and the
[recovery doc](/docs/recovery) covers what happens if you lose a device.

That's the bet: that a wallet you can actually use safely beats a wallet that is
theoretically perfect and practically lossy.
