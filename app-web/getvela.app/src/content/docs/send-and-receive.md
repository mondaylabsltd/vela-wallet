---
title: Send & receive
description: "How to receive and send with Vela — one address on every network, sending to one or many people, how recipients are named, what you confirm, and how the relay moves your funds."
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Send & receive

## Receive

1. Open your wallet and tap **Receive**.
2. Share your address — copy it or show the QR code. The web wallet can also
   make a payment request that includes an amount.
3. When the transfer confirms on-chain, it appears in your balance.

- Your address is **the same on every network**, so you give out one address —
  but the sender still has to use a network Vela supports, or one you've added.
- You can **receive before your wallet is deployed** on a network. It deploys
  itself on your first send there.

## Send

1. Tap **Send** and choose the **token**.
2. Enter the **amount** (in the token or in your display currency) and the
   **recipient's address**, by pasting, scanning a QR code or picking a contact.
3. **Review.** Vela shows what will happen, the fee, and the name it found for the
   recipient, if any.
4. **Confirm** with one of your keys — Face ID, a fingerprint, a PIN, or a touch
   and PIN on your security key.

### Sending to many, or sweeping

- **Split** — send one token to several people in a single transaction. You can
  paste a list or import a spreadsheet, and enter amounts in your currency.
- **Sweep** — send several tokens to one address in a single transaction.

Either way you sign once, and the transaction pays one fee.

### Names for addresses

When you enter an address, Vela looks up a name for it: first in its own
registry (the name of another Vela wallet), then in `.bnb`, `.arb`, `.g`,
Basename and ENS reverse records, read directly from each chain. This goes one
way — it names an address you've entered. Typing a name such as `alice.eth` does
not look up an address. Your saved **contacts** show their names too. Treat a name
as a hint, not proof: a reverse record or a Vela wallet name is chosen by whoever
controls that address.

### Fee coin and speed

The confirm screen shows the fee in the fee coin and in your currency. You can
pay in the network's coin or, where the relay accepts it, a USD stablecoin, and
choose a speed (default: fast). When you send the **maximum** of a native coin,
Vela keeps back enough for the fee. [How the fee is calculated](/docs/networks-and-fees).

### What happens when you confirm

1. Vela builds an ERC-4337 **UserOperation** for your Safe, including the fee
   payment to the relay.
2. Your key signs it with a **WebAuthn (P-256)** assertion after verifying you.
3. The signed operation goes to the **relay**, which submits it to the
   EntryPoint; your Safe checks the P-256 signature on-chain and executes.

<Callout type="info" title="The relay can't change your transaction">
The relay receives an operation that is already signed. It cannot change the
recipient, the amount or the fee — any change invalidates your signature. It can
delay or decline it, and it decides when it lands. It is open source, and you can
[run your own](/docs/self-hosting#relay).
</Callout>

Before you sign, Vela decodes what the transaction does and warns you about what
it can't decode; see [clear signing](/docs/clear-signing).

## Before you hit send

- **Check the start and end of the address.** Address-swapping malware is real,
  and so are look-alike addresses planted in your history.
- **Confirm the network.** Sending on the wrong network is a common, expensive
  mistake.
- **Start small with a new recipient.** A tiny test transfer is cheap insurance.

Transactions are irreversible. Nobody can claw back a send to the wrong address —
that is the nature of self-custody.

## Your activity

Your activity combines what you sent from this device with token transfers read
from each chain's logs. A plain native-coin transfer to you that arrives through
another contract (for example, some exchange withdrawals) may not produce a log
on some networks, so it can show in your balance without appearing in activity.
Balances are read live through a pool of RPC endpoints with automatic failover; a
spinner means "still fetching", not "funds gone".

Next: [networks & fees](/docs/networks-and-fees).
