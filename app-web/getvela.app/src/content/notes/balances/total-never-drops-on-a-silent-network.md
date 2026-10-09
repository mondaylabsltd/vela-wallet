---
title: "Why your total doesn't drop when a network doesn't answer"
nav: "When a network doesn't answer"
description: "When a network can't be read, Vela shows the last complete total instead of a smaller sum, and lists every network it couldn't reach."
facts:
  - "Wait for each network | 18 seconds"
  - "Last complete total kept | 24 hours"
  - "Unreachable networks listed | All of them"
  - "Unreachable from mainland China | About 5 of 24"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/balance_dashboard.rs#L16-L22
  - rust/crates/vela-core/src/app/balance_dashboard.rs#L76-L80
  - rust/crates/vela-core/src/app/balance_dashboard.rs#L108-L126
  - rust/crates/vela-core/src/app/balance_dashboard.rs#L1110-L1118
  - rust/crates/vela-core/src/app/balance_dashboard.rs#L1366-L1379
  - rust/crates/vela-core/src/app/balance_dashboard.rs#L1533-L1561
  - specs/092-unreachable-network-notice/spec.md#L8-L34
  - rust/crates/vela-core/i18n/locales/en/assets.json#L6-L13
  - rust/crates/vela-core/i18n/locales/en/home.json#L5
docs: networks-and-fees
related:
  - balances/prices-from-onchain-pools
draft: true
---

Vela reads your balances from each network separately. It waits up to 18 seconds for each one, then
carries on without it. A network can miss that window because its public RPC is down, slow or
blocked where you are.

If the total were only the sum of the networks that answered, it would fall each time one didn't.
So Vela also keeps the last total from a round in which every network answered, saved on the device
for 24 hours. While any network is missing, the home shows the larger of the two. Until every
network answers again, the total can't fall below that saved figure, even if you've spent since.

While a refresh is running, the total can only rise as networks answer (issue #188). It can fall
only when the refresh finishes. Before anything is known, the home shows a placeholder rather than
$0.00.

A token with no price is a different case, because its network did answer. The total leaves it out,
and the home says "Some tokens couldn't be priced."

## Which networks are missing

A line under the balance names the network, or counts them, as in "Can't reach 3 networks right
now". Tapping it lists every unreachable network, with what Vela last read there:

- "Last seen" and an amount
- "Held tokens when last read", when none of them had a price
- "Held nothing when last read"
- "Not read yet"

Networks where you seemed to hold nothing stay on the list, because while a network can't be read
nobody knows what it holds now. While the list is open, Vela reads the networks again every 10
seconds. A network that is only rate-limiting requests isn't listed, because that clears on its own.

Before 2 October 2026 (spec 092), the line said the networks' RPCs were unavailable, and tapping it
opened a fix for only one of them. In mainland China, about 5 of the 24 networks' public RPCs are
always unreachable, so the line never went away.
