---
title: Networks & fees
description: The networks Vela supports, how gas fees work, and how Vela picks RPC endpoints.
---

# Networks & fees

## Supported networks

Vela launched on **Gnosis Chain** and is built to add more. Starting with one
network keeps the surface area small while the fundamentals settle. The network
picker in the app is where you'll switch as more are added.

## Gas fees

Every transaction on an EVM network costs **gas**, paid in the network's native
token. Gas covers the cost of the validators that process your transaction.

- On Gnosis Chain, fees are paid in **xDAI** and are typically a fraction of a
  cent.
- The fee depends on how busy the network is and how complex your transaction is.
- You need a small native-token balance to send anything — including to move
  other tokens — because the fee is always paid in the native token.

If a send fails for "insufficient funds for gas," it means you have the token
you're sending but not enough native token to pay the fee.

## How Vela talks to the network

Vela reads balances and broadcasts transactions through public **RPC endpoints**.
Rather than trusting a single provider, Vela keeps a list and **fails over**
automatically when one is slow or down, so a single bad node doesn't take the app
offline.

The endpoint list is sourced from a public, regularly-updated registry and falls
back to a built-in list if that registry is unreachable. You can think of it as
the app always having a backup way to reach the chain.

Next: [how passkeys work](/docs/passkeys).
