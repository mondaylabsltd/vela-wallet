---
title: "Your wallet name and key labels are public"
nav: "Wallet names and key labels are public"
description: "The wallet name you type and each key's label go into a public registry on Gnosis when you create the wallet, and can't be changed or removed later."
facts:
  - "Where | Registry contract on Gnosis"
  - "Who can read it | Anyone"
  - "Can it be edited or deleted | No"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - specs/080-site-content-accuracy/claim-ledger.md#L26
  - rust/crates/vela-core/src/app/create_wallet.rs#L54-L78
  - app-web/vela-wallet/src/lib/ui/onboarding/v2/NameScreen.svelte#L73-L88
  - rust/crates/vela-core/i18n/locales/en/onboarding.json#L138
  - rust/crates/vela-core/src/app/create_wallet.rs#L809-L819
  - rust/crates/vela-core/src/app/create_wallet.rs#L1409-L1420
  - rust/crates/vela-core/src/registry_metadata.rs#L35-L52
  - rust/crates/vela-core/src/registry_lookup.rs#L1-L35
  - rust/crates/vela-core/src/registry_lookup.rs#L58-L66
  - rust/crates/vela-core/src/app/send.rs#L2589-L2645
  - rust/crates/vela-core/src/registry_backup.rs#L1-L13
  - rust/crates/vela-core/i18n/locales/en/settingsModals.json#L95-L99
  - app-web/getvela.app/src/content/docs/create-wallet.md#L56-L75
docs: create-wallet
related:
  - keys/wallet-name-length
  - send-receive/names-must-resolve-back
draft: true
---

Creating a wallet publishes to a public registry contract on Gnosis: each key's public key,
credential ID, authenticator model and flags, your wallet name and key labels, the wallet address,
and the signed registration data. It is permanent and readable by anyone.

The record is what lets any one of your keys find the wallet again on a new device. Before you can
continue, the app asks you to tick "My public key and wallet name are written into the on-chain
contract."

## What the labels say

The first key's label is the wallet name. The apps label each key you add after it "Key 2",
"Key 3" and so on, and publish those labels as they are. So the text you choose is the wallet name.

The authenticator model names the password manager or security key that made each key, and the
flags say whether the key syncs. Both are readable too.

## Where the name shows up

Vela's apps can find the name from your address alone. This works once the wallet contract exists
on Gnosis, Ethereum, Base, BNB Chain, Arbitrum, Optimism or Polygon, which happens with your first
transaction there. After that, when someone sends to you from Vela, the send screen shows your
wallet name next to your address, tagged "Vela User".

If you use "Back up public keys to Ethereum" in Settings, the registration is copied to Ethereum as
it was written on Gnosis, with the name and labels included.

Don't put anything in the name that you wouldn't want a stranger to read, such as your full name or
where you keep a backup security key.
