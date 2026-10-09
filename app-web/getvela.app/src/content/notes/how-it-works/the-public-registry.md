---
title: "What the public registry stores, and why it exists"
nav: "The public registry"
description: "When you create a wallet, its list of public keys is written to a contract on Gnosis, so any device can find the wallet later, even without Vela's servers."
facts:
  - "Contract | 0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9"
  - "Lives on | Gnosis, with an optional copy on Ethereum"
  - "Can it be edited or deleted | No"
  - "Who pays to write it | Vela's index service"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - specs/080-site-content-accuracy/claim-ledger.md#L26
  - specs/080-site-content-accuracy/claim-ledger.md#L22
  - rust/crates/vela-core/src/registry_backup.rs#L1-L60
  - rust/crates/vela-core/src/registry_proof.rs#L1-L11
  - rust/crates/vela-core/src/registry_metadata.rs#L1-L52
  - rust/crates/vela-core/src/webauthn.rs#L297-L302
  - rust/crates/vela-core/src/app/mod.rs#L578-L600
  - app-web/vela-wallet/src/lib/backup/ethereum-backup.ts#L1-L20
  - rust/crates/vela-core/i18n/locales/en/settingsModals.json#L94-L103
  - app-web/getvela.app/src/content/docs/recovery.md#L15-L25
  - https://github.com/mondaylabsltd/p256-index/blob/main/contracts/src/WebAuthnP256PublicKeyRegistry.sol#L1-L125
  - https://github.com/mondaylabsltd/p256-index/blob/main/README.md#L10-L50
  - https://github.com/mondaylabsltd/p256-index/blob/main/README.md#L159-L162
docs: recovery
related:
  - how-it-works/how-a-new-device-finds-your-wallet
  - security-layers/names-and-labels-are-public
  - security-layers/decide-your-keys-first
order: 11
draft: true
---

A wallet's address is calculated from all the keys you create it with. A device that holds only one
of those keys can't work out the address, because it doesn't know the others.

So when you create a wallet, Vela writes the full list to a public registry contract on Gnosis, a
little like an entry in a public directory that nobody can edit. Any device can read it straight
from the chain.

## What's stored

Creating a wallet publishes to a public registry contract on Gnosis: each key's public key,
credential ID, authenticator model and flags, your wallet name and key labels, the wallet address,
and the signed registration data. It is permanent and readable by anyone.

A public key can check a signature but can't make one. The credential ID is the identifier your
device uses for the passkey. The authenticator model identifies the password manager or security key
that made it, when the device reports one, and the flags record details such as whether the passkey
can sync.

Each entry also notes how the key connects, such as built in or USB, as the browser reported it. No
private key is stored.

## Why it can't be changed

Each key signs its own entry right after it's created. A one-time group key, made on your device for
this purpose, then signs the whole record and is thrown away, so nobody holds a key that could sign
a changed version.

The contract has no owner and no admin functions, and it isn't behind a proxy, so it can't be
upgraded. Its only writes add new records. There is no function to edit or delete one.

## Who writes it

Vela's public-key index service submits the record to Gnosis and pays the gas, the network's charge
for running the transaction. The index also keeps a copy for fast lookups, but the record lives in
the contract, and an app can read it there directly.

## A copy on Ethereum

The registry keeps the exact signed data of every record it accepts. The same contract is deployed
at the same address on Ethereum, set up to accept those same signatures.

In Settings, "Back up public keys to Ethereum" submits your record there as it is. Your keys don't
sign the record again, but you approve one Ethereum transaction from your wallet and pay its fee.
Anyone may submit a copy, and being the sender grants nothing.

The registry contract is Vela's own code and hasn't had a third-party audit.
