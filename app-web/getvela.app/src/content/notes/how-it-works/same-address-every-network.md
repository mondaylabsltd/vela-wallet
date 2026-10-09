---
title: "Why your address is the same on every network"
nav: "Same address on every network"
description: "Nothing your address is calculated from names a network, and the contracts it relies on sit at the same addresses on every network Vela supports."
facts:
  - "Chain id in the address | None"
  - "Contracts checked per network | 12, plus the precompile at 0x100"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/safe.rs#L1-L36
  - rust/crates/vela-core/src/safe.rs#L346-L372
  - rust/crates/vela-core/src/app/network_admin.rs#L95-L174
  - rust/crates/vela-core/src/app/network_admin.rs#L2335-L2366
  - rust/crates/vela-core/i18n/locales/en/settingsModals.json#L79
  - app-web/getvela.app/src/lib/chain-setup/required-contracts.ts#L53-L155
  - rust/crates/vela-core/src/user_op.rs#L78-L79
  - specs/080-site-content-accuracy/claim-ledger.md#L11
  - specs/080-site-content-accuracy/claim-ledger.md#L16
docs: networks-and-fees
related:
  - how-it-works/how-your-address-is-made
  - networks/no-precompile-no-vela
  - how-it-works/receive-before-it-exists
order: 2
draft: true
---

Your address is derived from all the keys you create the wallet with, and is the same on every
network. The calculation uses your keys' public keys, the addresses of Safe's contracts and the
verifier address `0x100`, where the P-256 precompile that checks passkey signatures lives. None of
these depends on the chain, and there's no chain id anywhere in it.
[How your wallet address is calculated](/notes/how-it-works/how-your-address-is-made) lists each
input.

## Why the contracts have the same addresses

The calculation refers to contracts by address: Safe's proxy factory, the Safe singleton, Safe's
4337 module and its setup contract, Safe's WebAuthn signer and MultiSend. A wallet with more than
one key also uses Safe's passkey signer factory and its singleton.

These contracts are deployed with CREATE2, an Ethereum instruction that deploys a contract at an
address computed in advance. They go through one of two factories, the Safe Singleton Factory or
the Deterministic Deployment Proxy, and each factory has the same address on every network that
has it.

A CREATE2 address depends on the factory, a salt and the code, so each contract lands at the same
address on every network it's deployed to. Your wallet is placed the same way.

## What Vela checks when you add a network

Your address also exists on networks where the wallet can't run. If a network lacks Safe's proxy
factory, Safe's other contracts, the EntryPoint or the precompile at `0x100`, Vela can't deploy the
wallet there or sign from it, so it can't move funds sent to your address on that network.

Before adding a network, Vela asks it for the code at twelve contract addresses and sends the
precompile a signature it knows is valid:

- If any of the ten contracts Vela requires for every wallet, or the precompile, is missing, the
  network is marked incompatible.
- If only Safe's passkey signer factory and its singleton are missing, the network is added, with a
  note that a wallet with more than one passkey can't be created there. Keys two to seven are signer
  contracts that factory creates.

Signatures are tied to one network. What your key signs includes the network's chain id, so a
signature made for one network isn't valid on another.
