---
title: "Why typing “4,5” means 4.5 on every keypad"
nav: "Typing a decimal comma"
description: "A phone's keypad shows a decimal comma or point depending on the device's region. Vela reads a single typed comma as a decimal point."
facts:
  - "You type | 4,5"
  - "Vela reads | 4.5"
  - "Before spec 073 | 4, or 45"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/l10n/amount_text.rs#L1-L51
  - specs/073-amount-text/spec.md#L12-L28
docs: send-and-receive
related:
  - interface/orange-means-money-moves
draft: true
---

A phone's number keypad follows the device's region settings. In much of Europe, and in Brazil,
Russia, Turkey, Vietnam and Indonesia, the decimal key types a comma.

Before September 2026, the apps didn't all handle that comma the same way. On the send screen in
fiat mode, "4,5" was read as 4. In the custom limit for a dApp approval, commas were removed, so
"4,5" became 45.

Every app now runs the same rule from the shared core before an amount is used, and shows the
cleaned value in the field:

- A single comma typed into a number that has no decimal point yet is the decimal point, whatever
  number format the app is set to.
- A pasted number that uses both separators, such as "1,234.56" or "1.234,56", is read as 1234.56.
- A pasted value that can't be read in only one way is rejected, and the field keeps its previous
  value. For example, "1.5e-7" is rejected rather than turned into 1.57.

If you paste "1,5" while the app uses a decimal point, the field rejects it, because it could mean
1.5 or 15. Type it instead.
