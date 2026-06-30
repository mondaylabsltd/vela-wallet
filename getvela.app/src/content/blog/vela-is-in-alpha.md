---
title: Vela is in alpha — what that means, and what we're asking of you
description: "Vela works and holds real funds, but it's early software. Here's the honest version: why we say alpha, why to start with small amounts, and why the best thing you can do is read the code."
date: 2026-06-16
author: Shelchin
tags: [alpha, security, transparency]
---

Vela is in alpha. I could bury that word three clicks deep, but instead it's in
the top bar of the site, and this post is the honest version of what it means. If
you're going to trust early software with money, you deserve to know exactly what
you're stepping into.

## What "alpha" actually means here

Vela works. It's a real self-custodial wallet, it's used with real funds, and it
gets better almost every day. What it hasn't had is years of production
hardening — the long tail of weird edge cases that only show up when a lot of
people use something for a long time. That's the distance between "works" and
"boring and battle-tested," and it's the distance we're still closing.

## So: start with small amounts

That's my one ask. While we're in alpha, use amounts you'd be comfortable putting
into any new piece of software — not because I expect something to go wrong, but
because that's simply the rational way to treat anything this young. As Vela
matures — more time in production, more eyes on the code — that calculus changes.
For now, start small and scale up as your own confidence grows.

## You hold the keys — and the responsibility

Vela is fully self-custodial. We never hold your keys and **cannot move, freeze,
or recover your funds**. Your wallet is a passkey on your device, backed up by
iCloud Keychain or Google Password Manager.

That cuts both ways. By design there's no seed phrase and no social recovery — and
so no guardian we could lose, leak, or be forced to act on. The flip side is real:
if you lose both your device and your cloud-synced passkey, with no other copy, the
account can't be recovered. So keep your platform's passkey backup turned on. The
[recovery doc](/docs/recovery) explains exactly how it works.

## No third-party audit — and I won't pretend one is scheduled

Every Vela wallet is a [Safe](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
smart account, and Safe's contracts are independently audited and secure billions
in value. Vela's own integration around them has **not** had a third-party
security audit — and I won't imply one is booked when it isn't. A professional
audit is something I want to fund as the project can support it; today it is not
on the calendar.

What reviews Vela's code right now is less formal, and I'd rather be precise about
it than oversell it. The code is open, and the eyes on it are whoever is capable
and willing to look — early users reading the source, contributors testing the
integration, and increasingly people running it through AI tools to review. I
rely on that, and it genuinely helps. It is **not** a substitute for a
professional audit, and I won't dress it up as one. Weigh the security of your
funds accordingly. That gap is exactly why "alpha" is the honest label.

## The best thing you can do: don't trust me — verify

Vela is fully open source — the app and all three backend services, under the MIT
license. With early software, that isn't a footnote, it's the whole point. Read
the code. Run the services on your own infrastructure. Try to break it and tell
me how. The bug reports and pull requests early users send genuinely decide what
gets built next.

- Read the code: [github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet)
- Found something? [Open an issue](https://github.com/mondaylabsltd/vela-wallet/issues)
- Want the full security and recovery model in depth? [Read the whitepaper](/docs/whitepaper)

That's the deal. Vela is early, it's honest about being early, and it's built in
the open so you never have to take my word for any of it. Start small, keep your
backup on, and kick the tires.
