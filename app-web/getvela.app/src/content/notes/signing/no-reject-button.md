---
title: "Why there's no Reject button next to the slide"
nav: "No Reject button"
description: "On the signing sheet you approve by sliding and reject by closing the sheet with ✕. There is no separate Reject button."
facts:
  - "Approve | Slide across the track"
  - "Reject | Close the sheet with ✕"
  - "Keyboard | Enter or Space"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - app-web/vela-wallet/src/lib/signing/ui/SlideToConfirm.svelte#L5-L16
  - app-web/vela-wallet/src/lib/wallet/ui/BottomSheet.svelte#L35-L42
  - specs/079-android-dapp-browser-stability/spec.md#L20-L27
docs: clear-signing
related:
  - browser/lock-without-a-label
draft: true
---

When a dApp asks Vela to sign something, the signing sheet has one control for approving: a slide.
To reject the request, close the sheet with the ✕ in its corner.

Approve and Reject buttons side by side are easy to mix up on a small screen. The slide is hard to
complete by accident, because the knob has to travel 88% of the track. With a keyboard or a screen
reader, press Enter or Space on the focused slide instead.

## Closing the sheet

Closing the sheet tells the dApp that you rejected the request, and the dApp has to send it again.
Early versions also closed the sheet when you swiped down or tapped outside it. After a stray touch
rejected a request during testing on Android in September 2026, that was changed.

The signing sheet and the connect sheet now close only with ✕. Swiping, tapping outside and pressing
Escape do nothing, and the sheets have no drag handle.

Sending tokens from the wallet itself still uses a confirm button rather than a slide. Making the two
consistent is an open issue (#461).
