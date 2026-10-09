---
title: "Connected sites don't expire"
description: "A site you connect stays connected on that device until you disconnect it or erase the device. Vela never ends a connection after a set time."
facts:
  - "Expires | Never"
  - "Connects in | The extension, desktop and phone apps"
  - "Hosted web wallet | Doesn't connect to dApps"
  - "Disconnect revokes approvals | No"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/dapp_permissions.rs#L70-L73
  - rust/crates/vela-core/src/app/dapp_permissions.rs#L165-L179
  - rust/crates/vela-core/src/app/dapp_permissions.rs#L648-L679
  - rust/crates/vela-core/src/app/dapp_permissions.rs#L735-L772
  - rust/crates/vela-core/src/app/dapp_browser.rs#L367-L372
  - rust/crates/vela-core/src/app/dapp_browser.rs#L1150-L1208
  - app-web/vela-wallet/src/lib/settings/live.ts#L642-L690
  - rust/crates/vela-core/i18n/locales/en/connect.json#L81-L103
  - rust/crates/vela-core/i18n/locales/en.json#L144
  - rust/crates/vela-core/i18n/locales/en.json#L162
  - specs/080-site-content-accuracy/claim-ledger.md#L27
docs: install
related:
  - settings/erase-this-device
draft: true
---

When you press "Connect" on a site's request, Vela saves a permission for that site on the device.
It records the site, the address it was given and the network it connected on. While the
permission is there, the site can see your address and ask you to sign.

dApps connect through the Vela browser extension and the browser built into the desktop (macOS,
Windows), iOS and Android apps. The hosted web wallet doesn't connect to dApps.

## Which address does a site see?

A connection follows the account you're signed in to. When you switch accounts, connected sites are
moved to the new account and told its address, without asking you again. In the extension, a site
sees no address while you're signed out, and the connection returns when you sign back in.

## How do you disconnect?

- In the browser built into the desktop and phone apps, open the site's connection panel and
  choose "Disconnect".
- In Settings → Device storage, the Connections group lists each connected site. "Disconnect"
  removes one, and "Disconnect all" removes every connection on the device.

"Erase This Device" removes them all as well.

## What disconnecting doesn't undo

Disconnecting removes the permission from this device only. Connections aren't synced, so a site
you connected on another device stays connected there.

It also changes nothing on chain. A token approval you gave the site stays in force until you send a
transaction that revokes it. A permit you signed can still be used until its deadline.
