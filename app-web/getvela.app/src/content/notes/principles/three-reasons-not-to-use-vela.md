---
title: "Why the home page gives three reasons not to use Vela"
nav: "Three reasons not to use Vela"
description: "The home page lists three reasons not to use Vela right after the introduction, so you can weigh them before you deposit anything."
facts:
  - "Reason 1 | A relay fee on top of the gas"
  - "Reason 2 | Any key can sign alone"
  - "Reason 3 | Vela's own code isn't audited"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - app-web/getvela.app/src/lib/i18n/messages/en.ts#L307-L339
docs: why-vela
related:
  - security-layers/decide-your-keys-first
  - interface/orange-means-money-moves
featured: true
draft: true
---

The section after the introduction on getvela.app is called "The trade-offs, up front". It lists
three reasons not to use Vela:

1. Every transaction pays a fee to the relay on top of the gas.
2. Any key can sign on its own, so each extra key is another way into the wallet.
3. The contracts Vela runs on are audited, but Vela's own code isn't.

It ends with "If one of those three is unacceptable to you, Vela is not for you yet."

These are things you would otherwise find out after depositing: the fee when you first send, the
risk of an extra key when you lose one, and the audit status if you go looking for it. They're on
the home page so you see them before you move any money.

The section has been shortened over time rather than expanded. In September 2026 the first item lost
a paragraph that explained in detail that the fee you see is the fee you pay. The parts that affect
what you pay stayed: gas higher than a plain transfer, a fee on top of the on-chain cost, and the
option to switch relays or run your own.
