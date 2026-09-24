---
title: FAQ
description: "Short answers about custody, keys, recovery, networks, fees, what Vela can see, open source, and what happens if Vela goes away."
---

# FAQ

## Is Vela self-custodial?

Yes. Your wallet is a Safe smart account controlled only by your keys, which stay
in your devices, your password manager or your security keys. Vela holds no key
and no role on it, so it can't move, freeze or recover your funds by itself. It
does write the software that asks your keys to sign — see the
[threat model](/docs/whitepaper).

## Is there really no seed phrase?

Really. Your keys are passkeys, and a passkey has no secret you can write down or
type. See [how passkeys work](/docs/passkeys).

## What do I need to create a wallet?

A device that supports passkeys (a recent phone or computer with Face ID,
fingerprint or Windows Hello), or two hardware security keys. No email, no
account, no starting balance. You can create the wallet with up to seven keys; they can't
be added later. See [create your wallet](/docs/create-wallet).

## What happens if I lose my phone?

Sign in on a new device with any other key: the same passkey synced through
iCloud Keychain or Google Password Manager, another phone, or your security key.
If the phone held your only key and that key wasn't synced, the wallet can't be
recovered. See [recovery & sign-in](/docs/recovery).

## Which networks and tokens are supported?

24 built-in EVM networks, including Ethereum, Base, Arbitrum, Optimism, Polygon,
BNB Chain, Gnosis and Avalanche, plus any EVM network you add that meets the
requirements. Native coins and ERC-20 tokens. The address is the same on every
network. See [networks & fees](/docs/networks-and-fees).

## What does it cost?

- **The apps:** the web wallet, the browser extension and the desktop apps are
  free. The iOS and Android apps will be a one-time purchase in the stores; you
  can also build any app from source for free.
- **Each transaction:** a fee paid from your wallet to the relay that submits it
  — Vela's, unless you point the wallet at another relay or run your own. It
  covers the gas plus the relay's margin, with a minimum of about $0.01. The exact
  amount is on the confirm screen before you sign and is part of what you sign.
  There is no deposit and no subscription.
  [How the fee is calculated](/docs/networks-and-fees#fee).
- **No token.** Vela has none and plans none.

## Can I use Vela with dApps?

Yes, through the Vela browser extension (Chrome, Edge, Brave) and the browser
built into the desktop (macOS, Windows), iOS and Android apps. The web wallet at
wallet.getvela.app doesn't connect to dApps. See [install](/docs/install#dapps).

## What can Vela see or do?

Vela can't read your keys or move your funds by itself. Its services see your IP
address and what the app asks them: the index sees your public keys and wallet
name when it registers a new wallet, and the addresses you look up; the relay sees
your address, the operations you submit and the RPC endpoint your app uses; the
chain-data service sees which tokens and contracts your app asks about. What becomes public on-chain
is listed on [create your wallet](/docs/create-wallet#what-is-public). The
[privacy policy](/privacy) is the full, authoritative version.

## Is Vela open source?

Yes, all of it, under the MIT licence: the wallet apps and the core, the relay,
the public-key index, the exchange-rate service and the chain-data directory, on
[GitHub](https://github.com/orgs/mondaylabsltd/repositories). Each service can be
run by you — see the
[self-hosting guide](/docs/self-hosting).

## Is Vela audited?

The contracts your money sits in — Safe and its modules, and the ERC-4337
EntryPoint — are audited. Vela's own code is not, and no audit is scheduled. See
[audits & known issues](/docs/security-audits).

## What if Vela shuts down?

Your funds stay in your Safe on-chain. For an existing wallet, the Vela browser
extension and apps you build yourself keep working without getvela.app, and every
service is open source for someone else to run. The
[self-hosting guide](/docs/self-hosting#if-getvela-app-disappears) lists the
paths and their limits.

## I have a question that's not here.

Open an issue on [GitHub](https://github.com/mondaylabsltd/vela-wallet/issues), or
reach us on [X](https://x.com/realvelawallet) or [Telegram](https://t.me/velawallet).
