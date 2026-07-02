---
title: Send & receive
description: How to receive and send tokens in Vela — one address across networks, clear-signed transactions, and how account abstraction actually moves your funds.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Send & receive

## Receive

1. Open your wallet and tap **Receive**.
2. Share your address — copy it, or let the sender scan the QR code.
3. When the transfer confirms on-chain, the balance appears in your wallet.

Two things worth knowing:

- Your address is **the same on every supported network**, so you share one
  address everywhere — just make sure the sender uses the right network.
- You can **receive before your wallet is deployed**. Vela accounts are
  counterfactual smart accounts, so funds can arrive at your address before the
  contract exists on a given chain; it deploys itself on your first send there.

## Send

1. Tap **Send** and pick the **token**.
2. Enter the **amount** (you can toggle between the token and your display
   currency) and the **recipient**. Vela resolves known recipients to a name
   where it can — a Vela account, an ENS name, a Basename, and so on.
3. **Review and confirm.** Vela shows the transfer, then asks for your passkey
   (Face ID / Touch ID / fingerprint).

### What happens when you confirm

Vela doesn't just "broadcast" a transaction. Under the hood:

1. It builds an ERC-4337 **UserOperation** for your Safe account.
2. Your device signs it with a **WebAuthn (P-256)** assertion after your
   biometric check.
3. The signed operation goes to the **bundler**, which submits it to the
   EntryPoint; your Safe verifies the P-256 signature **on-chain** and executes.

<Callout type="info" title="The relayer can't tamper with your transaction">
The bundler receives an <strong>already-signed</strong> UserOperation. It can
delay or decline to relay, but it cannot change the recipient, amount, or any
other field — any change invalidates your signature. It's a liveness helper, not
a custodian, and it's open source so you can run your own.
</Callout>

### Clear signing — no blind approvals

Before you sign, Vela decodes the transaction using **ERC-7730** descriptors and
shows the **intent** (Send, Approve, Swap…), the **amounts and addresses**, and a
risk indication — not opaque hex. When it can't fully decode a call, it shows an
explicit **blind-sign warning** instead of pretending to understand it. An
unlimited token approval, for example, is flagged.

## Before you hit send

- **Check the first and last characters of the address.** Address-swapping
  malware is real.
- **Confirm the network.** Sending on the wrong network is the most common
  expensive mistake. See [networks & fees](/docs/networks-and-fees).
- **Start small with new recipients.** A tiny test transfer first is cheap
  insurance.

Transactions are irreversible. There's no support desk that can claw back a send
to the wrong address — that's the nature of self-custody.

## Reading your history

Balances and history are read live from a pool of public RPC endpoints with
automatic failover. If the network is slow, history may take a moment — a spinner
means "still fetching," not "funds gone."
