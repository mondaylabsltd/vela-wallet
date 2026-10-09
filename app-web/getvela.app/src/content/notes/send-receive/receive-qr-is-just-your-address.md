---
title: "Why the receive QR code is just your address"
nav: "Why the receive code is your address"
description: "The receive code holds only your address by default, because every wallet can scan that. A switch adds the network, for wallets that can read it."
facts:
  - "Default code | Your address only"
  - "With Include network on | Address and chain ID"
  - "Copying gives | The bare address, always"
  - "Switch turns off | Each time you open Receive"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/payment_request.rs#L12-L17
  - rust/crates/vela-core/src/app/payment_request.rs#L171-L176
  - rust/crates/vela-core/src/app/payment_request.rs#L292-L302
  - rust/crates/vela-core/src/app/payment_request.rs#L406-L421
  - specs/090-receive-network-qr/spec.md#L1-L10
  - specs/090-receive-network-qr/spec.md#L34-L56
  - rust/crates/vela-core/i18n/locales/en/receive.json#L77-L78
  - app-web/vela-wallet/src/lib/flows/screens/ReceiveQr.svelte#L92-L121
  - https://github.com/mondaylabsltd/vela-wallet/issues/208
docs: send-and-receive
draft: true
---

Your Vela address is the same on every network, so the address alone doesn't tell the payer's
wallet which network to send on. ERC-681 is a format that adds it. A code that reads
`ethereum:0x…@100` names Gnosis, and a wallet that understands the format opens a send on Gnosis.
Many wallets can't read that form.

So the code on the Receive screen is your bare address unless you change it, because every wallet
can scan an address. The payer then picks the network themselves.

The logo in the middle of the code is the network's, or the token's on a token's code. The app
draws it over the code, and a scanner doesn't read it.

## The Include network switch

Under the code there's a switch labelled "Include network". When you turn it on:

- The code names the network on screen, as in `ethereum:0x…@100`.
- A token's code, such as USDC on Base, names only the network (`@8453`). It carries no token and
  no amount, so the payer picks what to send.
- A line under the switch reads "Some wallets can't read this code. If theirs can't, switch it off."

Copying the address gives the bare address with the switch on or off, because people paste
addresses. The switch is off again each time you open Receive, so the next code you show is one
any wallet can read.

## Why the switch was added

A bug report on 15 September 2026 (#208) described a BNB Chain receive code from the web wallet,
scanned with a second Vela wallet. The screen named BNB Chain and the code carried its logo, but the
scan opened a send of XDAI on Gnosis, because the code held only the address.

The report was closed on 21 September as not supported and continued as #312. On 2 October 2026
the network became an optional part of the code, off by default (spec 090).
