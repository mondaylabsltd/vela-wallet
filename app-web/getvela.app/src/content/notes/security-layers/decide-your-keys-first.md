---
title: "Decide your keys before you create the wallet"
nav: "Decide your keys first"
description: "A Vela wallet's keys are chosen when you create it and can't be added, removed or replaced later. Decide which keys you want before you start."
facts:
  - "Keys per wallet | 1 to 7"
  - "Who can sign | Any one key"
  - "Add or remove keys later | Not possible"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - specs/080-site-content-accuracy/claim-ledger.md#L12-L13
  - specs/080-site-content-accuracy/claim-ledger.md#L34
  - rust/crates/vela-core/src/app/create_wallet.rs#L894-L899
docs: signers
related:
  - principles/three-reasons-not-to-use-vela
  - keys/wallet-name-length
order: 1
featured: true
draft: true
---

A wallet can have up to seven keys. You choose them when you create the wallet, and any one of them
can sign on its own. Because the wallet address is calculated from the keys, the set can't change
afterwards. [Signers & security keys](/docs/signers) explains why.

Before you create a wallet, think about these questions.

**What if you lose a device?** A passkey synced through iCloud Keychain, Google Password Manager or
another password manager is still available on your other devices. A key that doesn't sync, such as
a Windows Hello key or a hardware security key, is gone if you lose the device it's on. If your only
key doesn't sync, Vela asks you to add a second one.

**What if you lose your Apple or Google account?** If all your keys sync through one account, losing
that account means losing the keys. A hardware security key stored somewhere safe still works.

**Who else could use each key?** Any one key can sign on its own, so each extra key is another way
into the wallet, including for someone who finds it.

## A common setup

- A synced passkey on your phone for everyday signing.
- A hardware security key with a PIN, kept somewhere you'd still have it if you lost both your phone
  and your cloud account.

If you don't want the wallet to depend on an Apple or Google account, create it with two security
keys and keep them in different places.

## If a key is compromised

Keys can't be removed. If a key might be in someone else's hands, create a new wallet and move your
funds to it. The old address can still be spent by that key on every network, including funds sent
to it later.
