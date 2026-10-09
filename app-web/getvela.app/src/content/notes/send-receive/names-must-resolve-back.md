---
title: "Why a name shown for an address must resolve back to it"
nav: "Why a name must resolve back"
description: "Vela shows a name service's name for an address only if looking that name up gives the same address back. Otherwise it shows the address alone."
facts:
  - "Name shown when | It resolves back to the address"
  - "Lookup fails or times out | Address shown alone"
  - "Vela User names | Chosen by the wallet's owner"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/name_verify.rs#L1-L51
  - rust/crates/vela-core/src/app/name_verify.rs#L75-L99
  - rust/crates/vela-core/src/app/name_verify.rs#L155-L172
  - app-web/vela-wallet/src/lib/services/recipient-identity.ts#L21-L31
  - app-web/vela-wallet/src/lib/services/recipient-identity.ts#L250-L270
  - app-web/vela-wallet/src/lib/services/recipient-identity.ts#L341-L353
  - rust/crates/vela-core/src/registry_lookup.rs#L32-L34
  - rust/crates/vela-core/src/app/send.rs#L2128-L2134
  - app-web/vela-wallet/src/lib/flows/live-send.ts#L536-L572
  - rust/crates/vela-core/i18n/locales/en/send.json#L17
  - specs/081-audit-product-gaps/spec.md#L132
docs: send-and-receive
related:
  - security-layers/names-and-labels-are-public
  - security-layers/first-time-recipient
draft: true
---

A reverse record maps an address to a name, such as an ENS name. The owner of the address sets it,
and nothing stops them from choosing a name that isn't theirs. Anyone can take a new address and
point its reverse record at `vitalik.eth`, an exchange's name, or the name of someone you're about
to pay.

So Vela checks every name it gets this way. It looks the name up forward in the same name service
and compares the address that comes back with the one it started from. The name appears next to the
address only when the two match.

The rule covers every name service Vela reads: ENS, Basenames, .bnb, .arb and .g. It lives in the
shared core, so every Vela app applies it. The check was added on 22 September 2026. Before that,
every Vela app showed the reverse name as it was.

## When the check can't finish

If the network doesn't answer or the lookup times out, Vela shows the address and asks again next
time. Showing the unchecked name instead would let an attacker pick a moment when the check fails.

Vela also shows no name for:

- a name served through an offchain gateway (CCIP-read), because Vela doesn't follow the gateway
- a name containing spaces, control characters or invisible formatting characters, such as a
  right-to-left override

## Names tagged Vela User

A name tagged "Vela User" doesn't come from a name service. It's the wallet name that wallet's owner
typed when creating it, read from Vela's public registry. Only the owner can set it, but they can
choose any name, and there is no forward record to check. On the confirm screen, the short address
is always shown under the name.
