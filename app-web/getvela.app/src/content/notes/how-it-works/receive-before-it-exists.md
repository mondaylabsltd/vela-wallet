---
title: "How you can receive money before your wallet exists"
nav: "Receiving before the wallet exists"
description: "Your address is known before the wallet contract exists, so anyone can send to it. Your first send on a network creates the contract in the same transaction."
facts:
  - "Cost to receive | Nothing"
  - "Contract created by | Your first send on each network"
  - "Who can sign that send | Any one of your keys"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - specs/080-site-content-accuracy/claim-ledger.md#L17
  - rust/crates/vela-core/src/user_op.rs#L265-L308
  - rust/crates/vela-core/src/user_op.rs#L361-L383
  - rust/crates/vela-core/src/user_op.rs#L614-L652
  - rust/crates/vela-core-uniffi/src/lib.rs#L1511-L1530
  - app-web/vela-wallet/src/lib/services/safe-transaction.ts#L2901-L2949
  - rust/crates/vela-core/src/safe.rs#L172-L186
docs: send-and-receive
related:
  - how-it-works/how-your-address-is-made
  - how-it-works/what-happens-when-you-send
  - how-it-works/how-the-fee-is-paid
order: 3
draft: true
---

Vela calculates your address in advance, from the wallet's recipe. Balances are recorded against an
address whether or not a contract exists there yet, so anyone can send coins or tokens to yours as
soon as Vela shows it. The contract that controls them is created the first time you send from that
network.

An address like this is called counterfactual: it's known, and can receive, before the contract is
deployed.

## What your first send does

ERC-4337 is the standard Vela's wallet uses: you sign an operation, and a shared contract called
the EntryPoint runs it.

1. Vela asks the network for the code at your address. If there is none, the wallet isn't deployed
   there yet.
2. The operation then carries deployment data in its `factory` and `factoryData` fields: Safe's
   proxy factory and the call `createProxyWithNonce(singleton, setupData, saltNonce)`.
3. The EntryPoint calls the factory first. The factory deploys your Safe at your address and runs
   its setup.
4. The EntryPoint then has the new Safe check your key's signature, and runs what you signed.

The deployment data is part of what your key signs, so the relay can't change it.

## Can any of my keys do it?

Yes. On a wallet with more than one key, setup also creates the signer contracts for keys two to
seven. That happens before the signature is checked, so any one of your keys can sign the first
send, not only the first key.

## What it costs

Receiving costs you nothing, because the sender pays the network fee for their transaction. Your
first transaction on a network deploys the wallet, and that transaction's fee covers the
deployment, so it costs more than later ones. The fee is shown before you sign.

Each network has its own copy of the contract. Deploying it on one network doesn't deploy it on
another, so each network's first send includes the deployment.
