---
title: "What “Erase This Device” deletes, and what it can't"
nav: "What “Erase This Device” deletes"
description: "It deletes everything Vela stored on the device but one pending record. It can't delete your passkeys, the relay's or index's records, or the registry."
facts:
  - "Left on the device | One record, on purpose"
  - "Relay keeps operations | Up to 14 days"
  - "Key index keeps records | Up to 30 days"
  - "On-chain registry | Permanent"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/storage_catalog.rs#L1-L13
  - rust/crates/vela-core/src/storage_catalog.rs#L120-L168
  - app-web/vela-wallet/src/lib/services/erase-device.ts#L12-L32
  - app-web/vela-wallet/src/lib/services/erase-device.ts#L165-L188
  - rust/crates/vela-core/i18n/locales/en.json#L109
  - rust/crates/vela-core/i18n/locales/en.json#L114-L123
  - app-web/getvela.app/src/routes/privacy/+page.svelte#L101-L114
  - specs/080-site-content-accuracy/claim-ledger.md#L25-L26
docs: recovery
related:
  - settings/sign-out-remove-or-erase
  - settings/connected-sites
draft: true
---

"Erase This Device" is in Settings. It removes transaction history, contacts and groups, browsing
history, dApp permissions, custom tokens and networks, endpoint settings, RPC provider keys and
every preference. It also signs every wallet out of the device.

## How does it find what to delete?

Vela stores its data under keys that start with `vela.`, plus one older cache. The erase lists every
such key on the device and deletes it, so data added in a later version is covered too. An earlier
version deleted from a hand-kept list of eleven keys. It missed contacts, contact groups, dApp
permissions, caches and every preference.

After deleting, Vela reads the storage again. If anything is left, it says "The erase did not
finish" and you stay signed in, so you can try again.

## The one record it keeps

The erase leaves `vela.pendingUploads` in place. It holds a passkey public key that the public-key
index hasn't confirmed yet. Vela retries the upload on the next launch, even with no wallet signed
in. Without the record, the upload could never be retried.

## What it can't delete

- **Your passkeys.** They belong to your passkey provider or security key, not to Vela. Sign in with
  any of your keys and the wallet is rebuilt from the public registry.
- **Records kept by Vela's services.** The relay keeps your operations for up to 14 days so it can
  retry them. The public-key index keeps its submission records for up to 30 days.
- **The registry on Gnosis.** Creating a wallet publishes each key's public key, credential ID,
  authenticator model and flags, your wallet name and key labels, the wallet address, and the signed
  registration data. It is permanent and readable by anyone.
- **Your transactions,** which are on chain.
