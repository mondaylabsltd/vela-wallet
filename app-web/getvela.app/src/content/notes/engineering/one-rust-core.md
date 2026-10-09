---
title: "Why one Rust core runs under four different apps"
nav: "One Rust core, four apps"
description: "Every Vela app is built on the same Rust library, vela-core, which holds the rules for money, keys and signing. The apps handle the interface."
facts:
  - "Apps | iPhone, Android, web, desktop"
  - "Shared core | vela-core, in Rust"
  - "Old app deleted | 11 September 2026"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - docs/ARCHITECTURE.md#L18-L26
  - specs/001-rust-core-bindings/spec.md#L1-L13
  - specs/039-retire-expo-tree/results.md#L24-L30
  - rust/crates/vela-core/src/app
related:
  - interface/orange-means-money-moves
featured: true
draft: true
---

Vela has four apps: iPhone (SwiftUI), Android (Jetpack Compose), the web wallet and browser extension
(SvelteKit), and desktop (gpui, written in Rust). All four use one Rust crate, vela-core, for anything
involving money, keys or signing. The apps display what the core decides.

## Why the core exists

Vela started as a single React Native app. Two problems led to the change:

- Code that had to be correct existed in several copies. Keccak-256 was implemented by hand in both
  TypeScript and Swift, and the Safe address calculation existed in three places. If the copies
  drifted apart, money could go to the wrong address, or the signing sheet could show a different
  transaction from the one being signed.
- Rules learned from bugs lived in UI code. "A passkey must sign once before anything is saved" was
  implemented in React state, and the send screen alone had about forty pieces of state. None of it
  could be tested without a browser.

## How the apps use it

The calculations moved to Rust on 28 July 2026, and the business rules followed over the next weeks.

- iPhone and Android call the core through generated Swift and Kotlin bindings.
- The web wallet and the extension run it as WebAssembly.
- The desktop app links it directly.

The React Native app was deleted on 11 September 2026. Keeping four apps consistent is ongoing
work, and the web wallet downloads a 4.7 MB WebAssembly file before it starts.
