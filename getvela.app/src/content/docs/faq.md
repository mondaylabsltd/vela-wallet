---
title: FAQ
description: Common questions about Vela — custody, passkeys, recovery, supported networks, and privacy.
---

# FAQ

## Is Vela self-custodial?

Yes. Your wallet is controlled by a key that only you can use, stored in your
device's secure hardware. Vela cannot move, freeze, or recover your funds.

## What happens if I lose my phone?

If your passkey is synced through iCloud Keychain or Google Password Manager, you
sign in on a new device with the same account and your wallet comes back. See
[recovery & sign-in](/docs/recovery) for the full model and its limits.

## Is there really no seed phrase?

Really. Your signing key is a passkey in your device's secure element. There's no
twelve-word phrase to write down, lose, or have phished. Read
[how passkeys work](/docs/passkeys) for why that's safe.

## What can Vela (the company) see or do?

Vela stores your passkey's **public** key and the **name** you chose, to enable
cross-device sign-in. It cannot see your private key, your balances are read from
public chains, and there's no email signup. The
[privacy policy](/privacy) is the authoritative version.

## Which networks and tokens are supported?

Vela launched on **Gnosis Chain** with support for ETH-style native tokens and
ERC-20s, and is built to add more networks. See
[networks & fees](/docs/networks-and-fees).

## Is Vela open source?

Yes — the wallet is [public on GitHub](https://github.com/atshelchin/vela-wallet).
You can read exactly what it does.

## How much does it cost?

The wallet is free to use. You only pay network gas fees for transactions, which
on Gnosis Chain are typically a fraction of a cent.

## I have a question that's not here.

Open an issue on [GitHub](https://github.com/atshelchin/vela-wallet) or reach us
on [X](https://x.com/realvelawallet) or [Telegram](https://t.me/velawallet).
