---
title: "Display currency: what it changes and what it doesn't"
nav: "Display currency"
description: "It sets the currency Vela shows values in. Transactions are always signed in token amounts, so changing it never changes what a transaction sends."
facts:
  - "Web and extension default | US dollar"
  - "Desktop and phone default | Your region's currency"
  - "If Vela has no rate | Figures shown in US dollars"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/display_currency.rs#L1-L21
  - rust/crates/vela-core/src/app/display_currency.rs#L105-L119
  - rust/crates/vela-core/src/app/display_currency.rs#L257-L373
  - app-web/vela-wallet/src/lib/settings/core/currency-executor.ts#L1-L47
  - app-web/vela-wallet/src/lib/wallet/live.ts#L168-L188
  - rust/crates/vela-core/src/app/send.rs#L455-L464
  - rust/crates/vela-core/src/app/send.rs#L3481-L3527
  - app-desktop/vela-wallet/src/executor/display_currency.rs#L103-L130
  - app-ios/VelaWallet/VelaWallet/Features/Settings/DisplayCurrencyExecutor.swift#L84-L88
  - app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/settings/core/CurrencyExecutor.kt#L86-L90
  - rust/crates/vela-core/i18n/locales/en.json#L48-L49
docs: send-and-receive
related:
  - settings/number-format
  - settings/hide-balance
draft: true
---

Vela reads your balances in tokens and prices them in US dollars. The display currency converts those
dollar values into the currency you choose: the total on the home screen, each asset's value and the
other fiat figures in the app. You set it under Settings → Localization → Currency.

## What it doesn't change

Every transaction is signed in token amounts, so the display currency doesn't change what a
transaction sends or the fee it pays.

On the send screen you can type an amount in your display currency instead of the token, and Vela
converts it to the token at the current rate. If you change the display currency while such an
amount is typed, the field is cleared, so a figure typed in one currency is never read in another.

## When Vela has no rate

If Vela can't get a rate for the currency you chose, it shows every figure in US dollars, labelled
USD. It doesn't put your currency's symbol on a dollar figure, and it never assumes a rate of 1. You
also can't type amounts in that currency on the send screen until a rate arrives.

When you pick a new currency, figures stay in the old one until the new rate arrives. The currency and
the rate then change together, so a balance worth ¥1,860 never flashes up as ¥12.

## The default

The web wallet and the browser extension start in US dollars, because a browser doesn't report a
region currency. The desktop and phone apps use the currency of the device's region, but only once a
rate for it has loaded. If it can't be loaded, they stay on US dollars and try again at the next
launch. A currency you choose yourself always takes priority.
