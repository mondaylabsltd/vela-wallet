---
title: "Where the wallet uses orange"
nav: "Where orange is used"
description: "In the Vela wallet, orange is used for actions that move money, such as Send. Errors, sign-out and switches use other colours."
facts:
  - "Orange | Actions that move money"
  - "Sign out, Erase | Red"
  - "Switches | Text colour when on"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - docs/DESIGN-REVIEW-2026-07.md#L9
  - app-web/vela-wallet/src/lib/ui/Button.svelte#L5-L10
  - app-web/vela-wallet/src/lib/ui/Switch.svelte#L1-L9
  - app-web/vela-wallet/src/lib/settings/ui/DangerCard.svelte#L1-L6
  - app-web/vela-wallet/src/lib/tokens/contrast.test.ts#L1-L17
related:
  - principles/three-reasons-not-to-use-vela
draft: true
---

Vela's accent colour is orange (#E8572A). In the wallet it's used for actions that move money or
submit something, such as the Send button. Other controls use neutral colours.

A design review on 4 July 2026 found orange on many things that didn't move money: an "RPC offline"
notice, the sign-out button, filter chips, the theme picker and "Save image". Some error states also
used orange instead of the error red. These were changed:

- Sign out and Erase are red, because they delete data rather than move money.
- Switches use the text colour when they're on.
- Links in notices are plain text links.

In Settings, the card for clearing your data is the only one with a border, because it's the only
action there that can't be undone. With orange kept for one purpose, seeing it means money is about
to move.

White text on this orange has a contrast ratio of about 3.6:1, below the 4.5:1 that WCAG AA asks for
at this text size. The app's contrast tests record it as a known exception and check that it doesn't
drop below 3:1.
