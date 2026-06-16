---
title: "Release notes: the first public build"
description: What's in the first public version of Vela — passkey sign-in, ETH and ERC-20 support on Gnosis Chain, and cross-device recovery.
date: 2026-06-02
author: Shelchin
tags: [release-notes]
---

The first public build of Vela is out. It's deliberately small. The goal for
this version was to prove the core loop end to end: create a wallet with a
passkey, hold and move tokens, and get back into your wallet on a second device
without a seed phrase.

## What's in this build

**Passkey wallet creation.** Create a wallet with nothing but a name and your
face or fingerprint. No seed phrase screen, because there is no seed phrase.

**ETH and ERC-20 support.** Hold and send the native token and any ERC-20 on the
supported network. Balances and history are read live from public RPC endpoints,
with automatic failover across a list of providers so a single flaky node never
takes the app down.

**Cross-device sign-in.** Your passkey's public key is published to a small
on-chain index. Sign in on a new device with the same iCloud or Google account
and your wallet comes back — no twelve words to type.

**Gnosis Chain to start.** We're launching on one network on purpose. One chain
means fewer moving parts to get right while the fundamentals settle.

## Known limits

- One network for now. More are coming; the network picker is built to grow.
- No in-app token swaps yet.
- The passkey recovery model depends on your platform keychain sync. The
  [recovery doc](/docs/recovery) is the honest version of how that works.

## What's next

The near-term list, in rough order: more networks, clearer fees, and a round of
work on the empty and error states so that a slow or down RPC always *looks* like
a slow RPC and never like a lost balance.

If you want to follow along, the [blog](/blog) is where build notes land, and
the project is [open source on GitHub](https://github.com/atshelchin/vela-wallet).
