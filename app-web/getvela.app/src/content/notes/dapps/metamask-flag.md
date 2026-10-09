---
title: "Why Vela tells older dApps it's MetaMask"
description: "Many older dApps connect only to a wallet that reports itself as MetaMask, so Vela's provider does. Newer dApps find it under its own name, Vela Wallet."
facts:
  - "Legacy flag | isMetaMask set to true"
  - "EIP-6963 name | Vela Wallet"
  - "EIP-6963 id | app.getvela"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/provider/inpage.js#L1-L15
  - rust/crates/vela-core/provider/inpage.js#L384-L413
  - rust/crates/vela-core/provider/inpage.js#L428-L470
  - rust/crates/vela-core/provider/inpage.js#L177-L195
  - rust/crates/vela-core/provider/inpage.js#L102-L117
  - specs/027-web-extension-provider/research.md#L199-L208
  - specs/096-dapp-interop-pass/findings.md#L25
docs: install
related:
  - browser/web-wallet-has-no-browser
draft: true
---

A dApp can find a browser wallet in two ways. Older dApps read `window.ethereum`, and many of them
check `window.ethereum.isMetaMask` before they offer to connect. Newer dApps use EIP-6963: each
wallet announces a name, an icon and an id, and the dApp lists every wallet it hears from.

Many older dApps never added EIP-6963. If `isMetaMask` isn't set, they show no wallet or refuse to
connect. So Vela's provider sets `isMetaMask` to `true`.

The same provider runs in the Vela browser extension and in the built-in browser of the desktop
(macOS, Windows), iPhone and Android apps.

## How newer dApps see Vela

Through EIP-6963, Vela announces the name `Vela Wallet` and the id `app.getvela`. A dApp with a
wallet list shows Vela under its own name, and a real MetaMask in the same browser still announces
itself separately. A dApp that needs to know it has Vela can check `isVela`.

Vela sets `window.ethereum` only when no other wallet has set it. If another extension got there
first, Vela leaves it in place and adds itself to that wallet's `providers` list, if it has one.

A site that reads only the flag shows Vela as MetaMask. In a test on 2 October 2026, PancakeSwap
offered Vela only under its MetaMask entry.

## Other details

- When the provider learns the network for the first time, it fires `connect` and not
  `chainChanged`. Many dApps reload the page on `chainChanged`, so it fires only on a later switch.
- The wallet icon is embedded in the script as a data URI. A dApp that draws it makes no request
  to a Vela server, so the icon can't reveal which sites you visit.
