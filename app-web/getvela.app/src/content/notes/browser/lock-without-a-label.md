---
title: "Why the address bar shows a lock but no “Secure” label"
nav: "A lock, never “Secure”"
description: "The lock tells you a page loaded over HTTPS. It doesn't tell you the site is safe, so Vela's browser doesn't label any site as secure."
facts:
  - "HTTPS page | Closed lock"
  - "Plain HTTP page | Open lock, and no wallet"
  - "Text labels | None"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - specs/079-android-dapp-browser-stability/spec.md#L39-L40
  - specs/079-android-dapp-browser-stability/spec.md#L386-L390
  - app-desktop/vela-wallet/src/icons.rs#L95-L97
  - app-android/vela-wallet/app/src/main/java/app/getvela/wallet/core/designsystem/components/VelaIcons.kt#L706
  - app-ios/VelaWallet/VelaWallet/DesignSystem/LucideIcons.swift#L50
  - rust/crates/vela-core/src/app/dapp_permissions.rs#L874-L916
related:
  - signing/no-reject-button
draft: true
---

In the built-in browser on desktop, iPhone and Android, a page loaded over HTTPS shows a closed lock,
and a page loaded over plain HTTP shows an open lock. There's no text next to either icon, and screen
readers announce only the protocol.

HTTPS means the connection between you and the site is encrypted. It doesn't mean the site can be
trusted: a phishing site can get a valid certificate as easily as any other. In a wallet, what makes
a site dangerous is what it asks you to sign, and a "Secure" label next to the address could be read
as a judgment about that. Earlier Android builds showed such a label. It was removed in September
2026.

Pages loaded over plain HTTP can't connect to the wallet at all, because anyone on the network path
could change them. The exception is a page served from your own machine, such as a dApp you're
developing.

To decide whether to trust a request, read the signing sheet. It shows what the transaction does,
decoded from the data you're about to sign.
