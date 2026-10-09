---
title: "Why the signing sheet has no Reject button"
nav: "No Reject button"
description: "The signing sheet has one button, and it approves. You reject a request by closing the sheet with ✕. A swipe or a tap outside doesn't close it."
facts:
  - "Approve | Tap the button, then your passkey"
  - "Reject | Close the sheet with ✕"
  - "Swipe or tap outside | Does nothing"
checked: "2026-10-09"
commit: "c69e92365"
sources:
  - app-web/vela-wallet/src/lib/signing/ui/SigningBody.svelte#L88-L118
  - app-web/vela-wallet/src/lib/wallet/ui/BottomSheet.svelte#L35-L42
  - specs/079-android-dapp-browser-stability/spec.md#L20-L27
docs: clear-signing
related:
  - browser/lock-without-a-label
  - how-it-works/what-happens-when-you-send
draft: true
---

When a dApp asks Vela to sign something, the signing sheet has one main button, labelled with the
action itself, such as "Sign" or "Confirm swap". To reject the request, close the sheet with the ✕
in its corner. There is no Reject button next to the main one, so there's no second button to tap
by mistake.

Tapping the button doesn't sign anything on its own. It raises your passkey prompt, and approving
that with Face ID, a fingerprint, the device PIN or your security key is the second step. While
something blocks the request, the button is disabled and the reason is shown under it. A request
Vela refuses outright, such as one that would hand over the wallet, shows only a button to close
it.

## Closing the sheet

Closing the sheet tells the dApp that you rejected the request, and the dApp has to send it again.
Early versions also closed the sheet when you swiped down or tapped outside it. After a stray touch
rejected a request during testing on Android in September 2026, that was changed. The signing
sheet and the connect sheet now close only with ✕. Swiping, tapping outside and pressing Escape do
nothing, and the sheets have no drag handle.

## Why it's a button now

Until October 2026 the signing sheet confirmed with a slide, while sending tokens from the wallet
confirmed with a button, so the same kind of decision took two different gestures (issue #461). On
8 October 2026 the signing sheet moved to the same button in every app. The Trusted Signer page
still uses a slide.
