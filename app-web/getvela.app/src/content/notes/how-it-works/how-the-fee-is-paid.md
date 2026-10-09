---
title: "How the fee is paid inside your transaction"
nav: "How the fee is paid"
description: "The fee is one more payment inside the transaction you sign, from your wallet to the relay. The amount you see is the amount you sign."
facts:
  - "Paid from | Your wallet, in the same transaction"
  - "Paid to | The relay your wallet is set to"
  - "Paid in | Network coin or a supported stablecoin"
  - "Paymaster | None"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/fee_policy.rs#L1-L22
  - rust/crates/vela-core/src/user_op.rs#L241-L262
  - rust/crates/vela-core/src/user_op.rs#L1515-L1574
  - rust/crates/vela-core/src/user_op.rs#L204-L225
  - rust/crates/vela-core/src/app/send.rs#L1-L31
  - rust/crates/vela-core/src/app/send.rs#L4995-L5012
  - specs/080-site-content-accuracy/claim-ledger.md#L18-L21
  - https://github.com/mondaylabsltd/vela-relay/blob/main/docs/fees.md
  - https://github.com/safe-global/safe-modules/blob/4337/v0.3.0/modules/4337/contracts/Safe4337Module.sol#L155-L176
  - https://github.com/safe-fndn/safe-smart-account/blob/v1.4.1/contracts/libraries/MultiSend.sol
docs: networks-and-fees
related:
  - how-it-works/what-the-relay-does
  - how-it-works/what-happens-when-you-send
  - paying-less/stablecoin-fees-dont-save
order: 9
draft: true
---

When you send from Vela, your transaction holds the calls you asked for, such as a transfer or a
swap, plus one extra call that pays the fee. You sign them all at once. Because the fee is part of
what you sign, the fee on the confirm screen is exactly the fee in your signature, and nobody can
change it afterwards.

## Step by step

1. Before you sign, Vela asks the relay for a quote and works out the fee. The relay is the service
   that puts your signed transaction on chain.

2. Vela adds the fee as the last call in your transaction: a transfer of the network's coin to the
   relay, or a stablecoin `transfer` to it. On Tempo the fee is paid in pathUSD.

3. You sign the whole transaction with one of your keys.

4. The relay submits it to the network and pays the gas, the network's charge for running it, from
   its own funds.

5. The network runs your calls and the fee payment together, and the fee payment repays the relay.

## Why nothing is paid to the EntryPoint

A Vela wallet is an ERC-4337 smart account. Such an account normally pays gas money up front to the
EntryPoint, the shared contract that runs these transactions, which then pays whoever submitted
them.

Vela sets the operation's gas price fields, `maxFeePerGas` and `maxPriorityFeePerGas`, to zero. The
wallet then owes the EntryPoint nothing and sends it nothing. The relay is repaid only by the fee
payment inside your transaction.

## What this means for you

- Your calls and the fee payment run as one batch, which either runs in full or not at all. If the
  transaction fails on chain, no fee leaves your wallet.
- There's no paymaster, a contract that pays gas on someone else's behalf. Nobody sponsors your gas,
  and nobody can gate your transactions with a sponsorship policy.
- There is no per-network deposit or gas account.
- The relay can delay or refuse a signed transaction, but it can't change the fee or anything else
  in it.
- The fee is fixed before anyone knows exactly how much gas the transaction will use, so it's set
  above the expected on-chain cost. The relay keeps the difference.

When you send, Vela checks the relay's own gas balance on that network before you sign. If it has
run dry, Vela tells you instead of asking for your signature. The fee goes to the relay your wallet
is set to: Vela's by default, or another vela-relay deployment, including one you run yourself.
