---
title: "The four ways to create a wallet or sign in, and how they differ"
nav: "Four ways to sign in"
description: "You can create or sign in to a wallet with this device, another phone, a security key or the Trusted Signer. The first three are types of key."
facts:
  - "Kinds of key | This device, a phone, a security key"
  - "A way to reach a key | The Trusted Signer"
  - "Chosen | Once per device"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/mod.rs#L634-L648
  - rust/crates/vela-core/src/app/mod.rs#L222-L250
  - rust/crates/vela-core/src/app/method_words.rs#L120-L141
  - specs/019-onboarding-live-wiring/spec.md#L59-L71
  - specs/080-site-content-accuracy/claim-ledger.md#L13
  - specs/080-site-content-accuracy/claim-ledger.md#L28
docs: signers
related:
  - trusted-signer/trusted-signer-vs-in-app
  - security-layers/decide-your-keys-first
order: 1
draft: true
---

When you create a wallet or sign in, Vela shows up to four options. Three of them are types of key.
The Trusted Signer is a different way of using one of those keys.

| | What it is | Where the key lives | What you do to sign |
| --- | --- | --- | --- |
| **This device** | A passkey created on this phone or computer | Its password manager, synced if you allow it. A Windows Hello key stays on its PC | Face ID, a fingerprint or the device PIN |
| **Phone or tablet** | A passkey on another device | That device's password manager | Scan a new QR code each time, with Bluetooth on |
| **USB security key** | A hardware key, such as a YubiKey | Inside the key. It can't be copied out | Plug it in, touch it and enter its PIN |
| **Trusted Signer** | A page at sign.getvela.app that checks the transaction itself | Wherever the browser finds your passkey: any of the three above | Open the page, slide, then approve the browser's passkey prompt |

## Is the Trusted Signer a type of key?

No. The Trusted Signer is a separate web page. It decodes the transaction, calculates the data to be
signed, and then asks your browser for your passkey, which is one of the other three types. In
Settings, keys created through the Trusted Signer are labelled "Trusted Signer", even though they're
ordinary passkeys. [The Trusted Signer vs signing in the app](/notes/trusted-signer/trusted-signer-vs-in-app)
explains what it adds.

## Each device remembers your choice

Each device remembers the key you signed in with and how it reached that key, and uses the same key
for every signature. You aren't asked each time. To use a different key, sign in again with it.

The same key can be reached in different ways on different devices. For example, a passkey synced to
your phone can be used from your laptop by scanning a QR code.

## Where each option is available

- This device: every app except the Linux desktop. On a Mac, only the official build. On Android,
  with Google Play services or another passkey provider.
- Phone or tablet: every app.
- USB security key: the web wallet, the extension, the desktop app and Android.
- Trusted Signer: the desktop, iPhone and Android apps. Not the web wallet or the extension.
