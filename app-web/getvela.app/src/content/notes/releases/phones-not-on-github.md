---
title: "Why the phone apps aren't on GitHub Releases"
nav: "Phone apps and GitHub Releases"
description: "Phone packages on GitHub couldn't be installed as downloaded, and the phone apps will be sold in the stores. You can also build them yourself, for free."
facts:
  - "On GitHub Releases | Desktop apps, browser extension"
  - "iPhone, Android | App Store and Google Play, not yet listed"
  - "Build it yourself | Free"
  - "Decided | 18 September 2026"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - specs/063-release-channels/spec.md#L16-L28
  - specs/063-release-channels/spec.md#L41-L63
  - specs/063-release-channels/spec.md#L129-L145
  - docs/RELEASING.md#L20-L21
  - docs/ARCHITECTURE.md#L107-L116
  - docs/ARCHITECTURE.md#L132-L141
  - specs/080-site-content-accuracy/claim-ledger.md#L29
  - rust/crates/vela-core/i18n/locales/en/onboarding.json#L178-L183
  - app-ios/VelaWallet/VelaWallet/VelaWallet.entitlements#L30-L46
  - specs/088-store-readiness/audit.md#L63
docs: install
related:
  - keys/four-ways-to-sign-in
draft: true
---

Vela's releases on GitHub carry the desktop apps for macOS, Windows and Linux, and the browser
extension. They don't carry iPhone or Android packages, and the release process forbids attaching
them.

## What happened

The first tagged releases, 0.9.0 to 0.9.2, included phone packages. Neither could be installed:

- The Android package had no signature. Android's installer can't read an unsigned package, so it
  stopped with "package info is null".
- The iPhone package was unsigned too, and iOS can't install an unsigned app from a download.

On 18 September 2026 the rule became that a package goes on GitHub Releases only if a person with
no developer tools can install it and reach a wallet. Phone packages don't go there at all.

## Why not sign them?

The iPhone and Android apps are coming to the App Store and Google Play as a one-time purchase. A
free package on GitHub would undercut that price, however it was signed.

## How to get a phone app now

The store apps aren't available yet. The code is open, so you can build the iPhone or Android app
yourself for free. A self-built app is the same wallet, except that it can't use "This device",
the phone's own passkey. iOS and Android let an app use a `getvela.app` passkey only when the app
is signed with Vela's key.

The other two ways in don't depend on that signature:

- "Phone or tablet": scan a QR code with another phone that holds your passkey.
- "USB security key": a FIDO2 key over USB, on Android. Security keys on iPhone haven't been
  confirmed to work yet.
