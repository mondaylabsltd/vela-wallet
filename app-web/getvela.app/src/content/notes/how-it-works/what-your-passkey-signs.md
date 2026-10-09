---
title: "What your passkey actually signs"
description: "A 32-byte fingerprint of one operation, for one wallet on one network. Change the amount, recipient, fee or network, and the signature fails."
facts:
  - "Your key signs | A 32-byte hash of the operation"
  - "Hash format | EIP-712, Safe's SafeOp type"
  - "Tied to | One wallet, one network, one nonce"
  - "Time limit | None (validUntil is 0)"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/user_op.rs#L78-L80
  - rust/crates/vela-core/src/user_op.rs#L347-L383
  - rust/crates/vela-core/src/user_op.rs#L393-L409
  - rust/crates/vela-core/src/user_op.rs#L241-L262
  - rust/crates/vela-core/src/user_op.rs#L1548-L1572
  - rust/crates/vela-core/src/webauthn.rs#L243-L251
  - rust/crates/vela-core/src/sign_message.rs#L20-L50
  - rust/crates/vela-core/src/safe.rs#L25-L26
  - app-desktop/vela-wallet/src/executor/user_op.rs#L983-L1005
  - specs/080-site-content-accuracy/claim-ledger.md#L18
  - specs/080-site-content-accuracy/claim-ledger.md#L38-L39
  - https://github.com/safe-global/safe-modules/blob/4337/v0.3.0/modules/4337/contracts/Safe4337Module.sol
docs: passkeys
related:
  - how-it-works/how-the-chain-checks-a-passkey
  - how-it-works/what-happens-when-you-send
  - how-it-works/how-the-fee-is-paid
order: 5
draft: true
---

When you confirm a transaction, Vela reduces the whole operation to a hash: a 32-byte number
calculated from its contents, like a fingerprint. Your passkey signs that hash. If any of those
contents change, even one digit of the fee, the hash changes and the signature no longer matches.

## What goes into the hash

Vela uses EIP-712, the Ethereum standard for hashing structured data, with the `SafeOp` type that
Safe's 4337 module defines. It covers every field of the operation except the signature itself:

- your wallet's address, and its `nonce`, a counter that lets each operation run only once
- `initCode`, which deploys your wallet in your first transaction on a network and is empty after
  that
- `callData`: your calls, plus the transfer that pays the relay its fee
- the gas limits, and two gas-price fields that Vela sets to zero, since that transfer pays the fee
- `paymasterAndData`, empty, because there's no paymaster
- `validAfter` and `validUntil`, both zero, so the operation has no time limit
- the address of the EntryPoint, the ERC-4337 contract that runs operations

The hash also covers a domain: the network's chain ID and the address of Safe's 4337 module. The
same operation on another network has a different hash, so a signature made for one network is
useless on any other.

Because the fee transfer is inside `callData`, the exact fee amount and its recipient are part of
what you sign.

## How the passkey signs it

Vela gives the hash to your key as the WebAuthn challenge. WebAuthn is the web standard behind
passkeys, and the challenge is the value a key is asked to sign. The software asking your key,
usually your browser or operating system, writes the challenge into a short JSON text,
`clientDataJSON`, along with the request type and the origin that asked.

Your key then checks that it's you, with Face ID, fingerprint, device PIN, or a touch and PIN on a
security key. It signs `authenticatorData` followed by the SHA-256 hash of `clientDataJSON`.
`authenticatorData` starts with a hash of `getvela.app` and carries a flag that records whether you
were verified. The signature uses the P-256 curve.

Signing is done on your device. Your passkey's private key never goes to Vela.

The passkey prompt names the site but doesn't show the operation. The confirm screen does, and it
is decoded from the exact transaction that is signed.

## Messages for dApps

When a dApp asks you to sign a message, Vela hashes it as the request defines, with EIP-191 or
EIP-712. It wraps that hash in Safe's `SafeMessage` type, under a domain made of the chain ID and
your wallet's address, and your passkey signs the result. A dApp checks it through EIP-1271, by
calling your wallet's `isValidSignature`, so the check works only where your wallet is deployed.
