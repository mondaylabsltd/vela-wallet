---
title: "How the blockchain checks a passkey signature"
nav: "How the chain checks a passkey"
description: "Passkeys sign with the P-256 curve. Networks that run Vela have a built-in P-256 checker at address 0x100, and Safe's passkey contracts call it."
facts:
  - "Curve | P-256 (secp256r1)"
  - "Checked by | Safe's passkey module v0.2.1"
  - "Verifier | The P-256 precompile at 0x100"
  - "Fallback verifier | None"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - specs/080-site-content-accuracy/claim-ledger.md#L15-L16
  - rust/crates/vela-core/src/safe.rs#L23-L53
  - rust/crates/vela-core/src/safe.rs#L172-L231
  - rust/crates/vela-core/src/webauthn.rs#L16-L17
  - rust/crates/vela-core/src/webauthn.rs#L194-L251
  - rust/crates/vela-core/src/user_op.rs#L415-L439
  - rust/crates/vela-core/src/user_op.rs#L482-L519
  - rust/crates/vela-core/src/user_op.rs#L1589-L1620
  - https://github.com/safe-global/safe-modules/blob/passkey/v0.2.1/modules/passkey/contracts/libraries/WebAuthn.sol
  - https://github.com/safe-global/safe-modules/blob/passkey/v0.2.1/modules/passkey/contracts/libraries/P256.sol
  - https://github.com/safe-global/safe-modules/blob/passkey/v0.2.1/modules/passkey/contracts/4337/SafeWebAuthnSharedSigner.sol
  - https://github.com/safe-global/safe-modules/blob/4337/v0.3.0/modules/4337/contracts/Safe4337Module.sol
docs: account-contract
related:
  - how-it-works/what-your-passkey-signs
  - networks/no-precompile-no-vela
  - how-it-works/your-wallet-is-a-safe
order: 6
draft: true
---

Ordinary Ethereum accounts sign with a curve called secp256k1, and every EVM network can check
those signatures. Passkeys sign with a different curve, P-256, also called secp256r1. Vela's
wallets check them with a precompile: a function built into the network itself, at a fixed
address. The P-256 precompile lives at `0x100`. It's EIP-7951 on Ethereum, live since the Fusaka
upgrade in December 2025, and RIP-7212 on rollups, with the same interface.

Your Safe doesn't call it directly. The signer contracts from Safe's passkey module v0.2.1 do.

## What happens on chain

1. The EntryPoint, the ERC-4337 contract that runs operations, asks your Safe to validate the
   operation. Safe's 4337 module computes the operation's `SafeOp` hash, the same hash your passkey
   signed.

2. The signature names the signer contract that should check it. Your first key is checked by
   Safe's shared WebAuthn signer, which reads that key from your Safe's own storage. Each extra key
   has its own signer contract, created by Safe's factory when your wallet is deployed. The Safe asks
   that contract whether the signature is valid.

3. The signer rebuilds `clientDataJSON`, the JSON text whose hash your key signed. It writes the
   request type, `webauthn.get`, and the challenge, which is the hash from step 1 in base64url.
   Then it appends the remaining fields Vela sent with the signature, such as the origin.

4. It hashes `clientDataJSON` with SHA-256, joins that to `authenticatorData`, and hashes the
   result. It also checks that `authenticatorData` has the user-verification flag set.

5. It passes that hash, the signature and your public key to `0x100`. The precompile returns 1 if
   the signature is valid.

The signature Vela submits doesn't contain the challenge. The chain computes it from the
operation, so a signature made for any other operation fails at step 5.

## Why some passkey providers don't work

Step 3 only rebuilds the right text if the provider wrote `clientDataJSON` in the standard order,
with the type first and the challenge second, and ended it right after the last field. A provider
that writes it another way, or doesn't set the user-verification flag, produces a signature the
Safe can't accept. Vela checks both before it submits, and shows "Your device's identity provider
is not compatible with Vela Wallet."

## Details

- Authenticators return the signature in DER format. Vela converts it to the raw 32-byte `r` and
  `s` and uses the low form of `s`, though Safe's library doesn't require it.
- There's no fallback verifier. Your wallet's setup data names `0x100` as the verifier for each
  key, and your address is calculated from that setup data, so a network without the precompile
  can't run Vela. [Why a network without the P-256 precompile can't run
  Vela](/notes/networks/no-precompile-no-vela) explains how Vela checks for it.
