---
title: "Your number format is a setting, not a guess"
nav: "Number format"
description: "Vela groups digits and writes the decimal mark the way you choose in Settings, not the way your browser or operating system would."
facts:
  - "Presets | Four, plus Automatic"
  - "Default | Automatic"
  - "Indian grouping | 12,34,567.89"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/l10n/mod.rs#L5-L14
  - rust/crates/vela-core/src/l10n/number.rs#L52-L116
  - rust/crates/vela-core/src/prefs.rs#L47
  - rust/crates/vela-core/src/prefs.rs#L165-L173
  - app-web/vela-wallet/src/lib/services/locale-format.ts#L1-L79
  - app-web/vela-wallet/src/lib/settings/live.ts#L738-L772
  - app-desktop/vela-wallet/src/executor/format_prefs.rs#L143-L162
  - app-android/vela-wallet/app/src/main/java/app/getvela/wallet/core/format/Formats.kt#L56-L68
  - rust/crates/vela-core/src/l10n/amount_text.rs#L18-L45
  - rust/crates/vela-core/src/l10n/currency.rs#L1-L34
  - app-desktop/vela-wallet/src/wallet/live.rs#L81-L95
  - app-web/vela-wallet/src/lib/wallet/live.ts#L168-L188
  - rust/crates/vela-core/i18n/locales/en.json#L48-L52
related:
  - balances/decimal-comma
  - settings/display-currency
draft: true
---

Under Settings → Localization → Number format you choose how Vela groups digits and which decimal
mark it uses. There are four presets:

- 1,234,567.89
- 1.234.567,89
- 1 234 567,89
- 12,34,567.89, the Indian style: the last three digits, then groups of two

The default is Automatic. It reads your system's number conventions, picks the closest of the four
presets, and formats exactly as that preset does.

## Why Vela doesn't use the system formatter

Browsers and operating systems format numbers from their own locale data, so the same balance could
read one way in one browser and another way on another machine. Vela writes every figure itself from
the chosen preset. Once a preset is set, a total doesn't depend on which browser or system shows it.
The space preset, for example, uses an ordinary space, even where French formatting would use a
narrow no-break space.

The preset decides how figures are shown. When you type an amount, a single comma is read as a
decimal point under every preset, because a phone's keypad follows the device's region rather than
this setting.

## What the preset doesn't decide

In the desktop app, each currency also sets its own number of decimals: none for the yen, won and
dong, three for the Kuwaiti dinar, two for most others. The web wallet, the extension and the phone
apps currently show two decimals for every currency.
