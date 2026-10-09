---
title: "Why Arc transactions have a 20 gwei floor"
nav: "Arc's 20 gwei floor"
description: "Arc drops transactions priced below 20 gwei without returning an error, so Vela never quotes a transaction on Arc below that price."
facts:
  - "Arc's minimum | 20 gwei"
  - "Below the minimum | Dropped, with no error"
  - "Other networks | No floor"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/fee_policy.rs#L130-L159
  - rust/crates/vela-core/src/app/fee_policy.rs#L2140-L2150
  - specs/060-arc-network-integration/research.md#L43-L52
docs: networks-and-fees
related:
  - transactions/a-timeout-is-not-a-failure
draft: true
---

On most chains, a transaction with a gas price that's too low waits in the mempool until it's
replaced or prices come down. Arc discards it instead. Its documentation says that "transactions with
maxFeePerGas lower than 20 Gwei are silently dropped". The sender gets no error and nothing appears on
chain, so the payment looks submitted but never happens.

Arc's base fee usually sits at exactly 20 gwei, so normal quotes aren't affected. The risk came from a
fallback: when the wallet couldn't read a network's gas price, it used to assume 5 gwei, and the
relay's ×2 margin turned that into 10 gwei, which is below Arc's minimum.

Vela now handles this in two ways:

- The shared core has a 20 gwei minimum for Arc, and no quote on Arc goes below it.
- Since 26 September 2026, a failed gas-price read on any network gives no quote at all. The fee row
  says the quote is unavailable, next to its refresh button. The old fallback had also overcharged:
  during an RPC outage it once priced an Ethereum transfer at $23, about 80 times the real cost.

The minimum is built into the app. If Arc lowers its floor, Vela needs a new release to follow.
