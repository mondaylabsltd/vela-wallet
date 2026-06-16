---
title: Building Vela in public
description: Why we're documenting every step of building a self-custodial wallet that has no seed phrase — and what to expect from this blog.
date: 2026-06-12
author: Shelchin
tags: [building-in-public, story]
---

This is the first post on the Vela blog, so it's worth saying plainly what this
space is for.

Vela is a self-custodial wallet for ETH and ERC-20 tokens. You sign with a
passkey — your face or your fingerprint — and there is no seed phrase to write
down, lose, or have phished out of you. That single decision shaped almost every
other part of the product, and the stories behind those decisions are what this
blog is about.

## What we'll write about here

Three kinds of posts, roughly:

- **Build notes** — the engineering decisions, the dead ends, the trade-offs.
  Why passkeys instead of seed phrases. Why every wallet is a Safe smart account.
  How recovery works without a custodian.
- **Release notes** — what shipped, what changed, what's next. Short and honest.
- **The longer story** — the why behind the project. Who it's for, and what we
  think a wallet should feel like in 2026.

## Why build in public

Two reasons, and they're both selfish.

The first is trust. A wallet asks you to believe that the people who wrote it
are careful, honest, and not going anywhere. You can't earn that with a landing
page. You earn it by showing your work — the reasoning, the mistakes, the fixes
— over a long time. Vela is open source for the same reason.

The second is that writing things down makes them better. Every time we've had
to explain a decision clearly enough to publish it, we've found the weak spots
ourselves first.

So: welcome. If you want the short version of what Vela is, the
[docs](/docs) are the place to start. If you want the long version, it'll show
up here, one post at a time.
