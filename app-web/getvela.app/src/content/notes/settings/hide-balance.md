---
title: "Hiding your balance"
description: "Tap the total on the wallet's home screen to hide your balances. Vela remembers this on the device until you tap again or erase the device."
facts:
  - "Where | Tap the total on the home screen"
  - "Remembered | On this device, for every wallet"
  - "Reset by | Erase This Device"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/balance_dashboard.rs#L800-L807
  - rust/crates/vela-core/src/app/balance_dashboard.rs#L969-L984
  - rust/crates/vela-core/src/app/balance_dashboard.rs#L1053-L1072
  - rust/crates/vela-core/src/app/activity_feed.rs#L844-L847
  - rust/crates/vela-core/src/storage_catalog.rs#L1-L13
  - rust/crates/vela-core/src/storage_catalog.rs#L120-L168
  - rust/crates/vela-core/src/storage_catalog.rs#L225-L251
  - app-web/vela-wallet/src/lib/wallet/core/balance-types.ts#L26
  - app-web/vela-wallet/src/lib/wallet/ui/BalanceDisplay.svelte#L19-L41
  - app-web/vela-wallet/src/lib/wallet/live.ts#L516-L563
  - app-ios/VelaWallet/VelaWallet/Features/Wallet/WalletLive.swift#L486-L491
  - app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/wallet/WalletLive.kt#L189-L226
  - app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/wallet/WalletLive.kt#L498-L520
  - rust/crates/vela-core/i18n/locales/en.json#L161-L164
related:
  - settings/sign-out-remove-or-erase
  - settings/display-currency
draft: true
---

There's no switch for this in Settings. Tap the total on the wallet's home screen, and it turns into
dots with a crossed-out eye beside it. Tap the eye to show it again.

Vela stores the choice on the device under `vela.balanceHidden`, so it survives a restart and applies
to every wallet on that device.

## What it hides

In the web wallet, the extension, the desktop app and the iPhone app, hiding covers:

- the total on the home screen
- each asset's balance and value
- the amounts in your activity
- the pop-up that announces an incoming payment

One figure stays visible. An approval that lets a dApp spend an unlimited amount of a token still
reads "Unlimited" in your activity, because it's a standing risk rather than part of your balance.

The Android app currently hides only the total. Asset values and activity amounts stay visible there.

## What resets it

"Clear all caches" leaves it as it is, because it's a preference, not a cache. Signing out doesn't
change it either. "Erase This Device" resets it, along with every other preference.

Hiding only changes what the app shows. Anyone who knows your address can still read its balances on
a block explorer.
