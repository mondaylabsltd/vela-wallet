---
title: FAQ
description: Common questions about Vela — custody, passkeys, smart accounts, recovery, supported networks, fees, and privacy.
---

# FAQ

## Is Vela self-custodial?

Yes. Your wallet is a smart account controlled by a key that only you can use,
stored in your device's secure hardware. Vela cannot move, freeze, or recover
your funds.

## Is my wallet a normal account or a contract?

It's a **Safe smart account** (a smart contract), operated with ERC-4337 account
abstraction. That's what lets you sign with a passkey, read every transaction
before approving it, and use the same address on every network. See the
[whitepaper](/docs/whitepaper) for the architecture.

## Is there really no seed phrase?

Really. Your signing key is a passkey in your device's secure element. There's no
twelve-word phrase to write down, lose, or have phished. Read
[how passkeys work](/docs/passkeys) for why that's safe.

## What happens if I lose my phone?

If your passkey is synced through iCloud Keychain or Google Password Manager, you
sign in on a new device with the same account and your wallet comes back. See
[recovery & sign-in](/docs/recovery) for the full model and its limits.

## Which networks and tokens are supported?

Vela ships with **8 EVM networks** — Ethereum, BNB Chain, Polygon, Arbitrum,
Optimism, Base, Avalanche, and Gnosis — plus custom networks, holding native
tokens and ERC-20s. Your address is the same on all of them. See
[networks & fees](/docs/networks-and-fees).

## How much does it cost?

The wallet is free and Vela has **no token**. You pay network **gas**, in each
network's native token, out of your own wallet balance, plus a relayer fee set to
**roughly the network fee itself** — so about **2× the on-chain cost**, shown as a
clear *network fee / relayer fee / total* split before you confirm (the price is
quoted by the bundler, and the wallet refuses anything above ~3× the network
rate). Each network also needs a small, **non-refundable deposit to activate its
gas relayer account** (Vela may sponsor this for new users); because that account
can run down, you may have to top it up again later — it isn't strictly one-time.
Details in [networks & fees](/docs/networks-and-fees).

## What can Vela (the company) see or do?

Vela stores your passkey's **public** key and the **name** you chose, to enable
cross-device sign-in. It cannot see your private key, your balances are read from
public chains, and there's no email signup. The [privacy policy](/privacy) is the
authoritative version.

## Is Vela open source?

Yes — the wallet and all three backend services (chain data, passkey index,
bundler) are [public on GitHub](https://github.com/atshelchin/vela-wallet) under
the MIT license, and you can self-host them.

## I have a question that's not here.

Open an issue on [GitHub](https://github.com/atshelchin/vela-wallet) or reach us
on [X](https://x.com/realvelawallet) or [Telegram](https://t.me/velawallet).
