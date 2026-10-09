---
title: "Pay up to 60 people with one fee"
description: "A split, a sweep or an imported list of payees goes out as one operation on one network. You sign it once and pay one fee."
facts:
  - "Recipients per split | Up to 60"
  - "Signatures | One"
  - "Fees | One"
  - "Networks | One per batch"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/send.rs#L83-L86
  - rust/crates/vela-core/src/app/send.rs#L321-L358
  - rust/crates/vela-core/src/app/send.rs#L4049-L4086
  - rust/crates/vela-core/src/app/batch_import.rs#L1-L9
  - rust/crates/vela-core/src/app/batch_import.rs#L85-L88
  - rust/crates/vela-core/i18n/locales/en/send.json#L101-L156
  - rust/crates/vela-core/src/app/dapp_rpc.rs#L509-L511
  - specs/080-site-content-accuracy/claim-ledger.md#L18
  - app-web/getvela.app/src/content/docs/send-and-receive.md#L34-L40
docs: send-and-receive
related:
  - paying-less/stablecoin-fees-dont-save
  - faster/one-transaction-at-a-time
draft: true
---

The send screen can put many transfers into a single operation:

- A split sends one token to several addresses. You add each one with "Add recipient".
- An import fills a split from a list. Under "Import list" you can paste rows or import a CSV, TSV,
  TXT or Excel file, with amounts written in the token or in a currency at a rate you can edit.
- A sweep sends several tokens to one address.

The operation carries every transfer and one payment to the relay for the fee, and its transfers
land together or not at all.

## Why it costs less

Some of an operation's gas goes to work that happens once per operation, whatever it carries, such
as checking your passkey signature on chain. Each fee also has a $0.01 minimum. Separate sends pay
for both every time. A batch pays for them once, plus the gas for each transfer it makes.

## Limits

- A split takes up to 60 recipients. An import that would take it past 60 keeps the first rows
  that fit and tells you how many will be sent.
- An import can't be applied if its total is more than your balance of that token.
- An import skips a row whose address appears earlier in the same list.
- A batch stays on one network. It can send one token to many addresses, or many tokens to one
  address, but not both at once.
