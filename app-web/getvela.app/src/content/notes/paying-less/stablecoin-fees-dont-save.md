---
title: "Paying the fee in a stablecoin doesn't save money"
nav: "Stablecoin fees cost the same"
description: "A fee paid in a stablecoin is the fee in the network's coin, converted to dollars at the relay's price. It's never the cheaper choice."
facts:
  - "Stablecoin counted at | $1"
  - "Converted at | The relay's price for the network's coin"
  - "Minimum | $0.01"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/fee_policy.rs#L18-L23
  - rust/crates/vela-core/src/app/fee_policy.rs#L923-L935
  - rust/crates/vela-core/src/app/fee_policy.rs#L1016-L1035
  - rust/crates/vela-core/src/app/fee_policy.rs#L2827-L2883
  - https://github.com/mondaylabsltd/vela-relay/blob/main/docs/fees.md
  - https://github.com/mondaylabsltd/vela-relay/blob/main/vela-relay-core/src/quote.rs#L122-L135
  - https://github.com/mondaylabsltd/vela-relay/blob/main/vela-relay-core/src/settlement.rs#L443-L471
docs: networks-and-fees
related:
  - paying-less/pay-many-people-one-fee
draft: true
---

Except on Tempo, Vela prices the fee in the network's own coin. For a stablecoin, it values that
amount at the relay's price for the coin, counts the stablecoin as exactly $1 and rounds up, with a
minimum of $0.01. The relay checks a stablecoin payment with the same conversion.

The operation also gets slightly heavier. A fee in the network's coin is a plain transfer to the
relay. A fee in a stablecoin calls the token contract, which adds 68 bytes of calldata and a write
to the token's storage, so the operation uses a little more gas.

A stablecoin fee is still useful when you hold none of the network's coin, because you can send
without buying any. It needs a price, though. If the relay has no price for the network's coin,
there is nothing to convert at, so a stablecoin can't pay the fee on that network. On Tempo, which
has no native coin, every fee is paid in pathUSD.

## Which coin Vela picks

You can choose the coin under "Fee token". If you don't, Vela picks one that covers the fee on top
of what the transaction itself moves, in this order:

1. A coin the transaction isn't sending, so the fee doesn't come out of the amount and a "Max"
   send stays whole.
2. A stablecoin before the network's coin, because a fee in dollars shows plainly what it costs.
3. The coin with the larger balance in dollars.

For a dApp call that hasn't been simulated yet, the network's coin comes first, because it's the one
coin a contract can't take more of than the call states.
