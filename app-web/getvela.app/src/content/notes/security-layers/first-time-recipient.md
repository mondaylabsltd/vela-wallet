---
title: "What Vela flags about a recipient before you send"
nav: "What Vela flags about a recipient"
description: "Vela tags an address you've never sent to from this device, and warns when the address is a token's own contract. Neither flag stops the send."
facts:
  - "Never sent to | First time sending here"
  - "A token's own contract | Warning on form and confirm"
  - "Split sends | Rows not checked"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - app-web/vela-wallet/src/lib/services/recipient-risk.ts#L1-L16
  - app-web/vela-wallet/src/lib/services/recipient-risk.ts#L59-L72
  - app-desktop/vela-wallet/src/executor/send.rs#L480-L499
  - rust/crates/vela-core/src/app/send.rs#L2136-L2143
  - rust/crates/vela-core/src/app/send.rs#L2649-L2675
  - app-web/vela-wallet/src/lib/flows/live-send.ts#L1069-L1093
  - rust/crates/vela-core/i18n/locales/en/componentsUi.json#L111
  - rust/crates/vela-core/i18n/locales/en/send.json#L170
docs: send-and-receive
related:
  - send-receive/names-must-resolve-back
draft: true
---

Address poisoning puts a look-alike address in your history. An attacker sends you a small or
worthless transfer from an address whose first and last characters match one you use, and hopes you
copy it the next time you pay.

On the confirm screen, Vela tags the recipient "First time sending here" when this device has no
record of a send or dApp transaction from your wallet to that address. Incoming transfers don't
count, so a look-alike that has only ever sent to you still gets the tag.

The send form and the confirm screen also warn when the recipient is a token's own contract on the
network you're sending on. That covers the token you're sending and every token in your list on that
network. A token contract usually has no way to send back what it receives, so the warning reads
"This is a token contract address. Assets sent here usually can't be recovered."

## What the flags don't cover

- Both checks run for a send to one address. The rows of a send split between several recipients
  aren't checked.
- Each device keeps its own history. On a new device, every address gets the tag until you've sent
  to it from there.
- A token that isn't in your list isn't recognised, so sending to its contract shows no warning.
- Saving an address as a contact, or starring it, doesn't change either flag.
