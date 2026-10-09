---
title: "How a new device finds your wallet"
description: "You sign in with any one of your keys. Vela finds that key in a public registry, reads the wallet's full key set and recalculates the address from it."
facts:
  - "Account list synced | No"
  - "Looked up in | Index, then Gnosis, then Ethereum"
  - "Signatures needed | Usually one"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - specs/080-site-content-accuracy/claim-ledger.md#L25
  - rust/crates/vela-core/src/app/login.rs#L1-L24
  - rust/crates/vela-core/src/app/login.rs#L315-L335
  - rust/crates/vela-core/src/app/login.rs#L396-L466
  - rust/crates/vela-core/src/app/login.rs#L756-L883
  - rust/crates/vela-core/src/webauthn.rs#L253-L292
  - rust/crates/vela-core/src/webauthn.rs#L337-L355
  - rust/crates/vela-core/src/registry_resolve.rs#L1-L36
  - rust/crates/vela-core/src/registry_resolve.rs#L228-L296
  - rust/crates/vela-core/src/registry_resolve.rs#L353-L438
  - rust/crates/vela-core/src/registry_chain.rs#L1-L37
  - rust/crates/vela-core/src/app/create_wallet.rs#L14-L36
docs: recovery
related:
  - how-it-works/the-public-registry
  - how-it-works/how-your-address-is-made
  - keys/four-ways-to-sign-in
order: 10
draft: true
---

Your wallet's address is calculated from all the keys you created it with. When you created the
wallet, that list of keys was written to a public registry on Gnosis. Account lists aren't synced
between devices, so on a new device you sign in with any one key. Vela finds that key in the
registry, reads the whole list and calculates the address again.

## Step by step

1. **You sign in with a key:** a passkey that synced to this device, another phone through a QR
   code, or a security key. If this device already has the wallet saved, Vela opens it without
   looking anything up.

2. **Vela works out the key's public key.** A sign-in returns a signature and a credential ID, the
   name your device gives the passkey, but not the public key, and the registry can't be searched by
   credential ID. One P-256 signature, the kind passkeys make, fits at most two public keys, and
   Vela calculates both.

3. **Vela looks both up.** Only one is real. A record can only be written with a signature from the
   key itself, so only the real key can have one.

4. **Vela reads the wallet's record:** every key it was created with, and the address recorded with
   them.

5. **Vela recalculates the address** from those keys. If it doesn't match the recorded address, or
   the keys don't include the one you signed in with, Vela doesn't open the wallet.

## Who answers the lookup

Vela asks its index service first, a server that keeps a copy of the registry for fast answers. If
the index doesn't answer or knows nothing about the key, Vela reads the registry contract on Gnosis
directly. If Gnosis can't answer either, it reads the copy on Ethereum, if you made one.

The index's answer isn't simply believed. Vela recalculates the record's content hash, a fingerprint
of the key set and its details, and compares it with the hash stored in the contract. If they
differ, Vela discards the index's answer and reads the contract instead. A partial key set gives a
different fingerprint and a different address, so an incomplete set is never offered.

Shortly after a wallet is created, its record may not have reached the chain yet. If the contract
doesn't have it, or can't be reached, Vela uses the index's answer without that comparison. Step 5
still applies.

## If the key can't be found

If no record of the key turns up, or nothing answers at all, Vela offers to rebuild the wallet from
a second signature. Two signatures from the same key pin down one public key.

The rebuild treats that key as the wallet's only key. It's meant for a one-key wallet whose record
never landed. A wallet with several keys can't be rebuilt this way, because one key can't tell Vela
what the others were.
