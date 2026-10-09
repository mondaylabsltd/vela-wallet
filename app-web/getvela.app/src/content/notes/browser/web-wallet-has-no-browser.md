---
title: "Why the web wallet has no dApp browser"
description: "The web wallet is itself a page in your browser, and a page can't give other sites a wallet. To use dApps in a desktop browser, install the Vela extension."
facts:
  - "Web wallet tabs | Wallet, Contacts, Settings"
  - "Explore tab | macOS, Windows, iPhone, Android"
  - "dApps in a desktop browser | The Vela extension"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - app-web/vela-wallet/src/lib/wallet/destinations.ts#L1-L15
  - app-web/vela-wallet/src/lib/wallet/ui/TabBar.svelte#L10-L23
  - specs/022-explore-signing-ui/data-model.md#L18-L24
  - specs/080-site-content-accuracy/claim-ledger.md#L27
  - specs/080-site-content-accuracy/claim-ledger.md#L29
  - app-desktop/vela-wallet/src/wallet/page.rs#L268-L280
  - docs/ARCHITECTURE.md#L149
docs: install
related:
  - dapps/metamask-flag
  - browser/lock-without-a-label
draft: true
---

The desktop app on macOS and Windows, the iPhone app and the Android app each have an Explore tab.
It holds a built-in browser that puts Vela's provider into the pages it opens, so dApps connect
through EIP-1193 and EIP-6963 as they would to any browser wallet.

The web wallet at wallet.getvela.app leaves Explore out. It runs as a page inside your browser, and
a page can't add its own script to a page from another site. So it has no way to give a dApp a
wallet to talk to. The web wallet shows three tabs instead of four: Wallet, Contacts and Settings.
This was decided on 2 September 2026.

The web wallet doesn't connect to dApps in any other way, and there is no WalletConnect.

## How to use dApps from a desktop browser

Install the Vela browser extension. It gives dApp pages the same provider as the apps' built-in
browsers. It also uses the same `getvela.app` passkeys as the web wallet, so the same keys open the
same address. The extension isn't on the Chrome Web Store yet, so you download it and load it
unpacked.

The Linux desktop app has no Explore tab either. It runs as a native Wayland client, and Wayland
doesn't let one program place another program's page inside its window.
