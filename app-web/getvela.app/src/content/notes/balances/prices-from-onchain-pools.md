---
title: "Where Vela's prices come from"
description: "Vela reads crypto prices on chain, from DEX pools checked against Chainlink, not from a price API. Fiat rates come from Chainlink or an exchange-rate server."
facts:
  - "Crypto prices | On chain, no price API"
  - "DEX price used if | Within 0.5× to 2× of Chainlink"
  - "Listed stablecoins | $1"
  - "Fiat rates | Chainlink or an exchange-rate server"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - app-web/vela-wallet/src/lib/services/wallet-api.ts#L1-L14
  - rust/crates/vela-core/src/app/balance_dashboard.rs#L441-L480
  - rust/crates/vela-core/src/app/balance_dashboard.rs#L520-L560
  - rust/crates/vela-core/src/app/balance_dashboard.rs#L577-L619
  - app-web/vela-wallet/src/lib/services/wallet-api.ts#L604-L649
  - app-web/vela-wallet/src/lib/services/wallet-api.ts#L690-L750
  - app-web/vela-wallet/src/lib/services/currency-rate.ts#L1-L37
  - app-web/vela-wallet/src/lib/services/fiat-rates.ts#L29-L48
  - app-ios/VelaWallet/VelaWallet/Features/Settings/DisplayCurrencyExecutor.swift#L24-L29
  - app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/settings/core/CurrencyRates.kt#L211-L235
  - app-desktop/vela-wallet/src/executor/display_currency.rs#L136-L157
  - app-web/vela-wallet/src/lib/services/endpoints.ts#L17-L19
  - https://github.com/mondaylabsltd/vela-relay
docs: self-hosting
related:
  - balances/total-never-drops-on-a-silent-network
draft: true
---

Vela doesn't ask a price API what a coin is worth. When it reads your balances on a network, the
same call asks that network's DEX pools for quotes, so balances and prices arrive together.

## A network's own coin

For a coin such as ETH or BNB, Vela quotes one wrapped coin against each stablecoin on the
network's list and keeps the price from the deepest pool. A nearly empty pool can be far off. On X
Layer, one pool quoted OKB at about $5 while a deeper one quoted about $81.

Vela then compares that price with Chainlink's, from a feed on the same network or, failing that,
the feed on Ethereum. The DEX price is used only when it's more than half and less than twice
Chainlink's. Otherwise Chainlink's price is used, because a gap that wide usually means a thin pool.
When only one source answers, Vela uses that one.

## Other assets

- Stablecoins on a network's built-in list count as $1. There's no check for a lost peg, because
  the only other measure would be the same DEX pools.
- Coins that are dollars by design count as $1: USD on Tempo, USDC on Arc and USDT0 on Stable.
- A token you add yourself takes its price from the first pool that answers, against a stablecoin
  or the network's wrapped coin. No second source checks it.

A token with no pool has no price, and your total leaves it out.

## Fiat currencies

Values in other currencies are converted from US dollars. For a set of major currencies with a
Chainlink feed, such as the euro, the pound and the yen, the web wallet and the phone apps read
that feed on Ethereum first. Other currencies, and all of them in the desktop app, come from an
exchange-rate endpoint. By default that's Vela's own Frankfurter server, and you can change it under "Service
Endpoints".

The relay that submits your transactions prices fees separately. Its README says it takes
native-coin prices from Binance.
