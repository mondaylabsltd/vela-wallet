---
title: FAQ
description: Common questions about Vela — custody, passkeys, smart accounts, recovery, supported networks, fees, and privacy.
---

# FAQ

## Is Vela self-custodial?

Yes. Your wallet is a smart account controlled by a key that only you can use,
held by your device's OS and never seen by Vela. Vela cannot move, freeze, or
recover your funds.

## Is my wallet a normal account or a contract?

It's a **Safe smart account** (a smart contract), operated with ERC-4337 account
abstraction. That's what lets you sign with a passkey, read every transaction
before approving it, and use the same address on every network. See the
[whitepaper](/docs/whitepaper) for the architecture.

## Is there really no seed phrase?

Really. Your signing key is a passkey held by your device's OS, and Vela never
sees it. There's no
twelve-word phrase to write down, lose, or have phished. Read
[how passkeys work](/docs/passkeys) for why that's safe.

## What happens if I lose my phone?

If your passkey is synced through iCloud Keychain or Google Password Manager, you
sign in on a new device with the same account and your wallet comes back. See
[recovery & sign-in](/docs/recovery) for the full model and its limits.

## Which networks and tokens are supported?

Vela ships with **12 EVM networks** — Ethereum, BNB Chain, Polygon, Arbitrum,
Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad, and World Chain —
plus custom networks, holding native tokens and ERC-20s. Your address is the same on all of them. See
[networks & fees](/docs/networks-and-fees).

## How much does it cost?

The wallet is free and Vela has **no token**. You pay network **gas** out of
your own wallet balance, plus a relayer fee. The price is quoted by the relay
and shown as a _network fee / relayer fee / total_ split **before you sign** —
the exact cost of every transaction is on the confirm screen, and the quoted
amount is part of what you sign, so it can't change afterwards. Very cheap
transactions may hit a small minimum fee. On Tempo, which has no native coin,
gas is paid in USD stablecoins. Each network also needs a small,
**non-refundable deposit to activate its gas relayer account** (Vela may sponsor
this for new users); because that account can run down, you may have to top it
up again later — it isn't strictly one-time.
Details in [networks & fees](/docs/networks-and-fees).

## What can Vela (the company) see or do?

Vela stores your passkey's **public** key and the **name** you chose, to enable
cross-device sign-in. It cannot see your private key, your balances are read from
public chains, and there's no email signup. The [privacy policy](/privacy) is the
authoritative version.

## Is Vela open source?

Yes — the wallet and its four backend services (chain data, passkey index,
relay, exchange rates) are [public on GitHub](https://github.com/mondaylabsltd/vela-wallet) under
the MIT license, and you can self-host them.

## I have a question that's not here.

Open an issue on [GitHub](https://github.com/mondaylabsltd/vela-wallet) or reach us
on [X](https://x.com/realvelawallet) or [Telegram](https://t.me/velawallet).
