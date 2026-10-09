---
title: "How your wallet address is calculated"
description: "Your address is a fingerprint of the wallet's recipe: Safe's factory, Safe's code and a setup that lists every key. The same recipe gives the same address."
facts:
  - "Method | CREATE2 (EIP-1014)"
  - "Deployed by | Safe Proxy Factory v1.4.1"
  - "Your part of the input | All your keys' public keys"
  - "Calculated | On your device, before deployment"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/safe.rs#L23-L53
  - rust/crates/vela-core/src/safe.rs#L134-L158
  - rust/crates/vela-core/src/safe.rs#L172-L281
  - rust/crates/vela-core/src/safe.rs#L283-L372
  - rust/crates/vela-core/src/primitives.rs#L111-L127
  - rust/crates/vela-core/src/app/create_wallet.rs#L978-L985
  - specs/080-site-content-accuracy/claim-ledger.md#L11
docs: account-contract
related:
  - how-it-works/same-address-every-network
  - how-it-works/receive-before-it-exists
  - security-layers/decide-your-keys-first
order: 1
featured: true
draft: true
---

Think of your wallet address as the fingerprint of a recipe. The recipe says which factory builds
the wallet, which code it runs and how it's set up, including every key you create it with. Vela
works out the fingerprint on your device before anything exists on a blockchain.

Your address is derived from all the keys you create the wallet with. A different key, or one more
or one fewer, gives a different address.

## The formula

Safe's proxy factory, at `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67`, builds the wallet with
CREATE2, an Ethereum instruction that deploys a contract at an address computed in advance. Vela
runs the same calculation offline:

```text
address   = last 20 bytes of keccak256(0xff ‖ factory ‖ salt ‖ keccak256(initCode))
initCode  = Safe proxy creation code ‖ Safe singleton address
salt      = keccak256(keccak256(setupData) ‖ saltNonce)
saltNonce = keccak256(x1 ‖ y1 ‖ x2 ‖ y2 ‖ … ‖ xn ‖ yn)
```

`keccak256` is Ethereum's hash function, and `‖` joins bytes end to end. Each key is a P-256
public key, the kind passkeys use, and `x` and `y` are its two 32-byte coordinates. The factory
derives the salt itself from the setup data and the salt nonce, and Vela makes the salt nonce from
your keys.

## What the setup data contains

`setupData` is the call to Safe's `setup` function, which runs once when the wallet is created.
Vela's call sets:

- `owners`: Safe's shared WebAuthn signer for the first key, then one signer contract for each
  other key. Each of those addresses is itself a CREATE2 result from Safe's passkey signer factory,
  computed from the key's `x`, `y` and the verifier address `0x100`.
- `threshold`: 1, so any one key can sign.
- `fallbackHandler`: Safe's 4337 module.
- `to` and `data`: a batch that runs during setup. It enables the 4337 module, records the first
  key's `x`, `y` and `0x100` as the shared signer's configuration for this wallet, and creates the
  signer contract for each other key.

`0x100` is the address of the P-256 precompile, the built-in contract that checks passkey
signatures. Every byte of the setup data goes into the salt, so changing any key, contract address
or verifier changes the wallet address.

## Does the order of keys matter?

The first key in your list stays first. Keys two to seven are sorted by their public keys before
anything is hashed, so the order you add them in doesn't change the address. Putting a different
key first does.

Because every key is part of the recipe, the keys can't change after the wallet is created. See
[Decide your keys first](/notes/security-layers/decide-your-keys-first).
