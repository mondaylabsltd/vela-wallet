---
title: Send & receive
description: How to receive and send ETH and ERC-20 tokens in Vela, and what to check before you hit send.
---

# Send & receive

Moving tokens in Vela is two flows: share your address to receive, and confirm a
transaction with your passkey to send.

## Receive

1. Open your wallet and tap **Receive**.
2. Share your address — copy it, or let the sender scan the QR code.
3. When the transaction confirms on-chain, the balance appears in your wallet.

Your address is public and safe to share. It's how people pay you.

## Send

1. Tap **Send**.
2. Paste or scan the **recipient address**.
3. Choose the **token** and enter the **amount**.
4. Review the details, then **confirm with your passkey**. The transaction is
   signed by the key in your secure hardware and broadcast to the network.

## Before you hit send

A few habits that save real money:

- **Check the first and last characters of the address.** Address-swapping
  malware is real; confirm both ends match what you expect.
- **Confirm the network.** Sending on the wrong network is the most common
  expensive mistake. See [networks and fees](/docs/networks-and-fees).
- **Start small with new recipients.** A tiny test transfer first is cheap
  insurance.

Transactions are irreversible. There is no support desk that can claw back a
send to the wrong address — that's the nature of self-custody.

## Reading your history

Balances and transaction history are read live from public RPC endpoints. If the
network is slow, your history may take a moment to load. A spinner means "still
fetching," not "funds gone" — Vela is designed so that a flaky node never looks
like a lost balance.
