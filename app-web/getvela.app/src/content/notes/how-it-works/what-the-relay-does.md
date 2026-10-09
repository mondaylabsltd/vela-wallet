---
title: "What the relay does, and what it can't do"
nav: "What the relay can and can't do"
description: "Your wallet can't send its own transactions, so a relay sends them and pays the gas. It's paid back by a transfer inside the same operation, which you sign."
facts:
  - "Change your operation | Can't"
  - "Delay or refuse it | Can"
  - "Paymaster | None"
  - "Who can run one | Anyone; vela-relay is open source"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - specs/080-site-content-accuracy/claim-ledger.md#L18-L21
  - specs/080-site-content-accuracy/claim-ledger.md#L36
  - app-web/getvela.app/src/content/docs/whitepaper.md#L196-L214
  - app-web/getvela.app/src/content/docs/whitepaper.md#L288-L298
  - app-web/getvela.app/src/content/docs/networks-and-fees.md#L59-L68
  - app-web/getvela.app/src/content/docs/networks-and-fees.md#L129-L144
  - rust/crates/vela-core/src/user_op.rs#L1548-L1572
  - rust/crates/vela-core/src/app/fee_policy.rs#L548-L556
  - rust/crates/vela-core/src/app/tx_tracker.rs#L26-L29
  - rust/crates/vela-core/i18n/locales/en/send.json#L70
  - rust/crates/vela-core/i18n/locales/en.json#L28
  - rust/crates/vela-core/i18n/locales/en.json#L81
  - https://github.com/mondaylabsltd/vela-relay
  - https://github.com/mondaylabsltd/vela-relay/blob/main/docs/fees.md
docs: networks-and-fees
related:
  - how-it-works/how-the-fee-is-paid
  - how-it-works/what-happens-when-you-send
  - principles/three-reasons-not-to-use-vela
order: 8
draft: true
---

Every transaction on an EVM network comes from an ordinary account, which signs it with its own
key and pays the network's gas. Your wallet is a Safe, a smart contract, so it can't start a
transaction. The relay runs such accounts. It takes the operation you signed, places it in a
transaction to the EntryPoint, the ERC-4337 contract that runs operations, and pays the gas.
ERC-4337 calls this role a bundler. Vela's relay is
[vela-relay](https://github.com/mondaylabsltd/vela-relay), and it's open source.

## How it gets paid

Your operation sets its gas-price fields to zero, so it pays the EntryPoint nothing, and there's no
paymaster: nobody sponsors your gas, and nobody can gate your transactions with a sponsorship
policy. The relay is paid by the last call in your operation, a transfer from your wallet to the
relay. The exact amount is shown before you sign and is part of what you sign. [How the fee is
paid](/notes/how-it-works/how-the-fee-is-paid) has the details.

## What it can't do

The relay receives an already-signed operation. It can't change the recipient, the amount, the fee
or anything else, because any change makes your signature invalid. It holds no key and no role on
your Safe, so it can't move funds you haven't signed for.

## What it can do

- Delay your operation, or refuse it.
- Choose when it lands. For a swap, it could trade ahead of you within your slippage.
- Affect the fee you're quoted, because its price for your chosen speed is one of the prices the
  fee is based on. The wallet refuses a price far above its own reading of the network's gas price.
- See your address, the operations you submit and the RPC endpoint your app uses, including any
  API key in its URL.

If gas prices rise after you sign and your fee no longer covers the cost, the relay holds the
operation and sends it once fees settle. The app shows "Network fees rose above the amount you
approved." By default, vela-relay gives up and refuses the operation after about 35 minutes.

## Using a different relay

The fee goes to the relay the wallet is set to: Vela's by default. You can point the wallet at
another vela-relay deployment, or one you run, under Settings → Advanced → Service Endpoints →
Vela Relay. It must be vela-relay, because the wallet asks for its fee quote with
`vela_getInBandGasQuote`, and generic ERC-4337 bundlers don't implement Vela's fee quote.
[Self-hosting](/docs/self-hosting#relay) explains how to run one.
