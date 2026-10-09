---
title: "Your wallet is a Safe contract, not a Vela account"
nav: "Your wallet is a Safe"
description: "Your money sits in a Safe smart contract on the blockchain that only your keys control. Vela has no account on its side that holds it."
facts:
  - "Account | Safe v1.4.1, unmodified"
  - "Runs through | ERC-4337 EntryPoint v0.7"
  - "Vela's role on your Safe | None"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - specs/080-site-content-accuracy/claim-ledger.md#L15
  - specs/080-site-content-accuracy/claim-ledger.md#L23-L24
  - specs/080-site-content-accuracy/claim-ledger.md#L35
  - rust/crates/vela-core/src/safe.rs#L23-L36
  - rust/crates/vela-core/src/safe.rs#L70-L84
  - rust/crates/vela-core/src/safe.rs#L172-L281
  - app-web/getvela.app/src/content/docs/account-contract.md#L12-L39
docs: account-contract
related:
  - how-it-works/how-the-chain-checks-a-passkey
  - how-it-works/what-the-relay-does
  - principles/three-reasons-not-to-use-vela
order: 4
draft: true
---

A Vela wallet is a smart contract on each network, a program that holds your coins and tokens and
moves them only when one of your keys signs. Vela's servers don't hold your balance.

Your wallet is an unmodified Safe v1.4.1, run through ERC-4337 (EntryPoint v0.7) with Safe's 4337
module v0.3.0, and verified by Safe's passkey module v0.2.1 (the shared signer for the first key, a
signer created by Safe's factory for each additional key). No contract in the funds path was
written by Vela.

Safe is a widely used smart-account contract. ERC-4337 is a standard for such accounts: a shared
contract, the EntryPoint, has the account check the signature on an operation and then runs it.

## What's at your address

- A Safe proxy, a small contract that forwards every call to Safe's shared `SafeL2` v1.4.1 code at
  `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762`.
- Owners, one per key. Each is a signer contract that checks passkey signatures. The threshold is
  1, so any one key can sign.
- One module, Safe's 4337 module, which lets the EntryPoint run operations your keys signed. It's
  also the fallback handler.

No Vela address appears among the owners, the modules or the fallback handler.

## What Vela holds

Vela holds no key and no role on your Safe, so it can't move or freeze funds by itself; it does
write and serve the software that asks your keys to sign. Vela's own contracts, such as the public
registry a new device uses to find your wallet, hold no funds and have no role in your Safe.

## Can other Safe tools use it?

Any Safe tool can read your account and build transactions for it; signing needs software that can
ask your key for a `getvela.app` signature. Vela's passkeys are bound to `getvela.app`, so Safe's
own web app, served from another domain, can't sign for it.
[If getvela.app disappears](/docs/self-hosting#if-getvela-app-disappears) lists what can.
