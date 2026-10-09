---
title: "Why transactions on one network land one at a time"
nav: "One at a time on each network"
description: "Each operation your wallet signs carries the next number in its sequence on that network, so operations land in order. To do several things at once, batch them."
facts:
  - "Order | By nonce, one after another"
  - "Same nonce twice | Only one runs"
  - "Desktop app waits | Up to 2 minutes"
  - "Batch from a dApp | wallet_sendCalls (EIP-5792)"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - https://github.com/eth-infinitism/account-abstraction/blob/v0.7.0/contracts/core/NonceManager.sol
  - https://github.com/mondaylabsltd/vela-relay/blob/main/vela-relay-core/src/execution.rs#L2430-L2458
  - app-ios/VelaWallet/VelaWallet/Core/RelayClient.swift#L820-L835
  - app-ios/VelaWallet/VelaWallet/Core/UserOpSpine.swift#L519-L523
  - app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/send/core/RelayClient.kt#L693-L699
  - app-web/vela-wallet/src/lib/services/safe-transaction.ts#L2951-L2999
  - app-desktop/vela-wallet/src/executor/user_op.rs#L1185-L1225
  - rust/crates/vela-core/src/app/tx_tracker.rs#L121
  - rust/crates/vela-core/src/app/dapp_rpc.rs#L509-L511
docs: send-and-receive
related:
  - paying-less/pay-many-people-one-fee
  - transactions/a-timeout-is-not-a-failure
draft: true
---

That number is the operation's nonce. The EntryPoint contract runs an operation only when its nonce
is the one it expects next for your wallet, and then raises the expected nonce by one. So a second
operation can't land before the first, and two operations with the same nonce can't both land.

## Starting a second one before the first lands

If the second operation has the next nonce, the relay holds it until the first has landed, then
sends it. If it has the same nonce, only one of the two runs. The other never runs and pays no fee,
because the fee is a transfer inside the operation.

Which nonce it gets depends on the app:

- The iPhone and Android apps read the nonce from the chain for every operation. While the first is
  pending, the chain still reports the old number, so the second gets the same one.
- The web wallet and the browser extension use the next nonce for 10 seconds after the relay accepts
  an operation, then read it from the chain again.
- The desktop app waits up to two minutes for its previous operation on that network to land before
  it prepares the next one. If the previous one hasn't landed by then, the new request fails before
  you're asked to sign.

## Doing several things at once

Put them in one operation. On the send screen, a split or a sweep carries many transfers with one
signature and one fee. A dApp can send several calls with `wallet_sendCalls` (EIP-5792). Vela sends
them as one operation, so they all land or none do.
