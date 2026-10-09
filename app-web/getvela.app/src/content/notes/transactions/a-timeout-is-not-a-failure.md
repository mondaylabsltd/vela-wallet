---
title: "Why a timeout never marks your transaction failed"
nav: "A timeout isn't a failure"
description: "Vela marks a transaction as failed only when the chain or the relay says it failed. A timeout leaves it pending, so you don't pay twice."
facts:
  - "Marked failed by | A failed receipt, or the relay refusing it"
  - "Not by | A timeout or lost connection"
  - "Stops checking | After 24 hours"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/tx_tracker.rs#L18-L46
  - specs/082-dapp-browser-mac-ext-ios/spec.md#L24-L28
  - specs/082-dapp-browser-mac-ext-ios/spec.md#L98
docs: send-and-receive
related:
  - networks/arc-gas-floor
featured: true
draft: true
---

A send can lose contact with Vela partway through: the relay stops responding, or the network drops.
The transaction may still land. If the app reported it as failed, you might send it again and pay
twice.

So Vela marks a transaction as failed in only two cases:

- the chain returns a receipt showing that it reverted or was dropped
- the relay reports that it refused the transaction

In every other case the transaction stays pending. The app tells you it may have been sent and asks
you not to send it again.

## How Vela keeps checking

Vela saves the transaction as "may have been sent" before the request leaves your device. It then
checks with the relay and also looks for the transaction on the chain directly. It treats the
transaction as never sent only after the relay has twice reported, more than a minute in, that it
has no record of it, and the chain shows no trace of it either.

After 24 hours Vela stops checking, and the transaction stays marked as pending.

## Why this changed

In a test on 28 September 2026, the relay's responses were deliberately dropped. A small send on
Gnosis landed on chain, but the app showed "Failed" and suggested trying again. The browser
extension, the Mac app and the iPhone app all did the same.
