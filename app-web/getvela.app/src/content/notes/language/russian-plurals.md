---
title: "The plural forms that were wrong on every phone"
nav: "Russian plurals on phones"
description: "The old phone app chose plural forms by the English rule, so Russian showed the wrong form after numbers such as 2 and 21. Vela's Rust core now chooses them."
facts:
  - "Forms checked | 675, in 15 languages"
  - "Wrong on phones | 26"
  - "Of those, in Russian | 18"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - specs/004-rust-i18n/spec.md#L44-L58
  - specs/004-rust-i18n/spec.md#L327-L330
  - specs/004-rust-i18n/sc004-change-set.md#L1-L14
  - specs/004-rust-i18n/sc004-change-set.md#L103-L115
  - rust/crates/vela-core/src/i18n/plural.rs#L1-L10
  - docs/ARCHITECTURE.md#L80
  - specs/039-retire-expo-tree/results.md#L24-L30
  - app-ios/VelaWallet/VelaWallet/Localization/Loc.swift#L5-L8
related:
  - engineering/one-rust-core
  - balances/decimal-comma
draft: true
---

Russian nouns change form after a number. `1 получатель` and `21 получатель` use one form,
`2 получателя` uses another, and `5 получателей` a third. English has two forms: one, and other.

## What went wrong

The old React Native app ran on Hermes, React Native's JavaScript engine. Hermes 0.14.1 included
`Intl`, but not `Intl.PluralRules`, which the translation library, i18next, uses to choose a form.
Without it, i18next fell back to the English rule without a warning: one form for 1, and another
for every other number.

So on iPhone and Android, Russian showed `21 получателей` where `21 получатель` is correct, and
`2 получателей` where `2 получателя` is correct. The correct forms were in the translation files,
but the app never selected them. The tests didn't catch it, because they ran in Node, which has
full locale data and chose the right forms.

A check of 5 plural keys in 15 languages at 9 counts, 675 cases in all, found 26 that were wrong
on phones. 18 were Russian. The other 8 were French and Brazilian Portuguese at zero, which the
plural rules for both languages treat as singular.

## What Vela does now

Translation moved into vela-core, the Rust library under every Vela app, on 31 July 2026. The core
carries its own plural rules for all 15 languages, so it doesn't depend on the platform for them.
They matched Node's full locale data in all 182,790 cases checked. The whole engine is also compared
with i18next on every language and key, plus 50,000 random sets of options.

The React Native app was deleted on 11 September 2026. The iPhone and Android apps that replaced it
call the core's engine through their Swift and Kotlin bindings.
