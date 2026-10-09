---
title: "Sign out, remove a wallet, or erase the device?"
nav: "Sign out, remove or erase"
description: "Sign Out and Remove from this device only take wallets off this device's list. Erase This Device deletes everything Vela stored here. None touches your keys."
facts:
  - "Sign Out | Every wallet's row on this device"
  - "Remove from this device | One wallet's row"
  - "Erase This Device | All but unconfirmed public keys"
  - "Your keys and funds | Not touched by any of them"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/session.rs#L38-L89
  - rust/crates/vela-core/src/app/session.rs#L149-L183
  - rust/crates/vela-core/src/app/session.rs#L564-L623
  - rust/crates/vela-core/src/storage_catalog.rs#L120-L168
  - app-web/vela-wallet/src/lib/session/ui/SignOutSheet.svelte#L1-L24
  - app-web/vela-wallet/src/lib/onboarding/core/storage.ts#L226-L233
  - app-web/vela-wallet/src/lib/services/erase-device.ts#L1-L45
  - rust/crates/vela-core/i18n/locales/en.json#L32-L36
  - rust/crates/vela-core/i18n/locales/en.json#L104-L123
  - specs/080-site-content-accuracy/claim-ledger.md#L11
  - specs/080-site-content-accuracy/claim-ledger.md#L25-L26
docs: recovery
related:
  - keys/four-ways-to-sign-in
  - settings/hide-balance
draft: true
---

All three actions take wallets off the device you're on. Account lists aren't synced between
devices, so none of them affects another device.

## Sign Out

Sign Out signs every wallet on this device out. It removes the list of wallets and which one was
open, and nothing else. Contacts, transaction history, custom tokens and networks, dApp permissions
and your settings stay on the device.

They can stay because your address is derived from all the keys you create the wallet with. Sign in
again with any of your keys and the same address returns, with its history and settings.

If a public key from this device hasn't been confirmed by Vela's public-key index yet, the sheet warns
you and the button reads "Sign Out Anyway". Until that key is confirmed, you may not be able to sign
in with it on another device. Vela retries at the next launch.

## Remove from this device

In the wallet list, "Remove from this device" takes one wallet off the list and leaves the others
signed in. It deletes nothing else, and signing in again brings the wallet back. Removing the last
wallet on the device is the same as signing out.

## Erase This Device

Erase deletes everything Vela stored on this device, including history, contacts, dApp permissions,
custom tokens and networks, endpoint settings and every preference. It also signs you out, and it
can't be undone.

Erase keeps one thing: any public key the index hasn't confirmed yet, so Vela can go on retrying the
upload. If that record were deleted, the upload could never be retried.

## What none of them touches

Vela holds none of your keys, and your funds are on chain. The wallet's record on the public registry
is permanent. After any of the three, you can sign in again on this device or another with any of
your keys.
