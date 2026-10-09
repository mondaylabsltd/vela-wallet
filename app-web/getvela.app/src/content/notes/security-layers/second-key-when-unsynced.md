---
title: "Why Vela asks for a second key when your only key doesn't sync"
nav: "When Vela asks for a second key"
description: "A wallet whose only key isn't backed up to a sync service can't be created. Any second key clears this, even one that stays on its own device."
facts:
  - "Blocked | One key, not synced"
  - "Cleared by | Any second key"
  - "Checked | Once, when you create the wallet"
  - "Sync state unreadable | Treated as synced"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/create_wallet.rs#L475-L483
  - rust/crates/vela-core/src/app/create_wallet.rs#L873-L900
  - rust/crates/vela-core/src/passkey.rs#L385-L431
  - app-web/getvela.app/src/content/docs/passkeys.md#L26-L30
  - rust/crates/vela-core/src/wallet_keys.rs#L232-L241
  - rust/crates/vela-core/i18n/locales/en/onboarding.json#L147-L186
  - app-web/vela-wallet/src/lib/ui/onboarding/v2/KeysScreen.svelte#L64-L92
  - app-web/vela-wallet/src/lib/ui/onboarding/v2/KeysScreen.svelte#L194-L197
docs: create-wallet
related:
  - security-layers/decide-your-keys-first
  - security-layers/names-and-labels-are-public
draft: true
---

When an authenticator creates a passkey, it reports whether the key is backed up to a sync service
such as iCloud Keychain, Google Password Manager or your password manager. This is the backup state
(BS) flag in the WebAuthn authenticator data. A hardware security key never sets it, and a Windows
Hello key stays on the PC.

If a wallet's only key doesn't have this flag, losing that one device or security key would leave
no way to sign for the wallet. So Vela won't create it. The key list is headed "One more key before
you can create" and shows "This key isn't backed up anywhere. Lose it and the wallet can't be
opened. Add a second one." The create button reads "Add a second key first" and stays disabled.

Any second key clears the check, including a second security key or a passkey that stays on its
device, because losing one key still leaves the other. A single key that does sync never triggers
it.

## What the check doesn't cover

Vela reads the flag from the key's registration, once, while you create the wallet. It doesn't
check again. If you later turn off sync or delete the passkey from your password manager, nothing
warns you. The key badges in Settings also show the state recorded at creation.

Some authenticators leave out the data that carries the flag. When Vela can't read it, it lets you
create the wallet rather than block a key that may well sync. The key then shows no sync badge,
rather than a "Cloud-synced" that nobody confirmed.
