---
title: Whitepaper
description: "How Vela works and what you do — and don't — have to trust to use it: the account, the keys, the fee, the threat model, recovery, and what happens if Vela disappears."
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Status: alpha · last revised September 2026">
This page describes how Vela works today and what you do and don't have to trust
to use it. Vela is in <a href="/blog/vela-is-in-alpha">alpha</a> — start with
small amounts. Vela has no token. Everything here can be checked against the
open-source code; where the code and this page disagree, the code is right and
this page is a bug.
</Callout>

## Summary

Vela is a **self-custodial smart-contract wallet** for Ethereum and other EVM
networks. Each wallet is an unmodified **Safe v1.4.1** account, operated through
**ERC-4337**, and controlled by up to seven **passkeys** — WebAuthn P-256 keys
held by your devices, your password manager, or hardware security keys. There is
no seed phrase.

Vela, the company, never holds your keys and **cannot move, freeze or seize your
funds**. The apps, the relay that submits transactions, and the supporting
services are open source, and each service can be replaced with your own. What
you trust reduces to audited contracts, the authenticators that hold your keys,
the code of the app you sign with, and — for liveness only — a relay.

## Why Vela exists

- **Seed-phrase wallets** put a 12–24 word secret in front of every user: a single
  point of failure and a standing phishing target.
- **Custodial wallets** remove the seed phrase by taking custody of the funds.
- **Passkey wallets** that depend on one company's servers and closed code remove
  the seed phrase but leave you stranded if the company goes away.
- **Blind signing** — approving opaque data you can't read — is normal across the
  ecosystem and behind a large share of drained wallets.

Vela aims for the convenience of a passkey with none of those dependencies: a
standard account, open code, replaceable services, and transactions you can read
before you sign.

## Design principles

1. **Self-custody, no exceptions.** Keys are created and held by your
   authenticators. Vela's services only ever see public data and signed
   operations.
2. **Standard contracts, unmodified.** No contract in the path to your funds was
   written by Vela.
3. **Verify, don't trust.** The apps and services are public; the services can be
   self-hosted.
4. **No blind signing by default.** Transactions are decoded before you sign;
   what can't be decoded is labelled as such.
5. **Do less.** The wallet sends, receives and signs for dApps you choose.

## Architecture

```text
Vela apps — web, browser extension, desktop (macOS/Windows/Linux), iOS, Android
  one shared Rust core (rules, crypto, ABI, clear signing) + a native shell each
  • builds the UserOperation and shows what it does
  • asks your key for a WebAuthn assertion
        │  signed UserOperation (fee included)
        ▼
Relay (vela-relay, self-hostable)
  • quotes the fee, pays gas up front, submits handleOps
  • cannot change the operation
        ▼
EVM chain
  EntryPoint v0.7 → your Safe v1.4.1 → Safe 4337 module
  Safe passkey module verifies P-256 via the RIP-7212 precompile
```

Supporting services, all replaceable: a **public-key index** that registers new
wallets in an on-chain registry and answers lookups, a **chain-data** directory,
and an **exchange-rate** feed. See the [self-hosting guide](/docs/self-hosting).

### Account

Your wallet is a **Safe v1.4.1** proxy (SafeL2 singleton), with Safe's **4337
module v0.3.0** enabled as its module and fallback handler, operated through the
**EntryPoint v0.7**. Its owners are passkey signers from Safe's **passkey module
v0.2.1**: the first key is verified by the shared signer, and each additional key
by its own signer contract created by Safe's factory. The threshold is **1**.

The address is **deterministic and counterfactual**: it is computed with
`CREATE2` from the Safe setup data, which includes every founding key, before
anything is deployed. It is the same on every network. You can receive at it
immediately; your first transaction on each network deploys the wallet and pays
for it within that transaction's fee.

### Keys

A wallet has **one to seven keys**, fixed when you create it. Any one of them can
sign alone (1-of-n). A key can be:

- a passkey on the device you're using — synced by iCloud Keychain, Google
  Password Manager or another password manager if you allow it;
- another phone, reached by scanning a QR code (the WebAuthn hybrid transport);
- a hardware security key over USB or NFC, which syncs nowhere.

Every signature needs the authenticator's own user verification — biometric, PIN
or touch. There is no session key. Keys cannot be added, removed or replaced
later: the address on every chain where the wallet is not yet deployed still
stands for the founding set, so changing owners on one chain would make the
account differ from chain to chain.

Passkeys belong to a relying party — Vela's are created for **`getvela.app`**.
That binding is what makes them phishing-resistant, and it is also a dependency
this paper returns to below.

### Signing flow

1. **Build** a UserOperation for your Safe — including a transfer that pays the
   relay — and simulate it.
2. **Decode** it into human-readable intent and show it to you.
3. **Sign**: your authenticator produces a WebAuthn assertion over the operation
   hash after verifying you.
4. **Encode** the assertion as the Safe signature the passkey module expects.
5. **Submit** the signed operation to the relay, which calls the EntryPoint.
6. **Verify on-chain**: the passkey module checks the P-256 signature with the
   RIP-7212 precompile before the Safe executes anything. There is no fallback
   verifier; a network without the precompile cannot be added.

### Fees

- The relay is paid **in band**: the operation declares zero EntryPoint fees and
  includes a transfer from your Safe to the relay's address. The amount and
  recipient are part of what you sign, so you pay exactly what the confirm screen
  showed.
- The fee is **three times the gas the wallet reserves for the operation**
  (the simulated estimates raised by half, with minimums), **priced at the higher
  of the wallet's own gas-price reading and the relay's price for the chosen
  speed**, with a minimum of about $0.01. On Tempo the multiple is two. Because of
  the padding and the headroom in the price, the fee is usually several times the
  operation's actual on-chain cost; the relay keeps the difference.
- The fee is paid in the network's coin or in a USD stablecoin the relay accepts
  (pathUSD on Tempo, which has no native coin). There is **no paymaster**: nobody
  sponsors gas, and nobody can filter transactions through a sponsorship policy.
- If a relay's own gas treasury on a network is empty, the wallet says so before
  you sign. There is no per-user deposit.

Details: [networks & fees](/docs/networks-and-fees).

### Clear signing

Calls and EIP-712 messages are decoded with **ERC-7730** descriptors — built into
the app for common contracts, fetched from the chain-data service, or matched to
standard token shapes — then, as a last resort, a public selector database,
labelled best effort. Anything left gets an explicit blind-signing warning.
Fetched descriptors are not cryptographically authenticated. An on-chain
unlimited token approval cannot be submitted until you reduce it; signed permits
are shown with a caution but not capped. Details:
[clear signing](/docs/clear-signing).

### Networks

Vela has 24 built-in networks — Ethereum, BNB Chain, Polygon, Arbitrum,
Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain, Arc,
X Layer, Stable, Soneium, MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink,
Plume and XRPL EVM — and accepts any EVM network that has the eleven contracts
it needs and the RIP-7212 precompile.

## Security model

**What Vela cannot do**

- Move, spend or freeze your funds — only your keys authorize your Safe, and Vela
  holds no role on it.
- Change a transaction after you sign it — any change invalidates the signature.
- Read your private keys — they stay in your authenticators.
- Add a key to your wallet, or remove one.

**What "cannot freeze" does not cover: the token.** USDC, USDT and most
fiat-backed tokens let their issuer blacklist any address, including yours. That
power belongs to the issuer and exists whatever wallet you use. What
self-custody gives you is that Vela is not a second party who can.

**What you trust**

- The **contracts**: Safe, its 4337 and passkey modules, EntryPoint v0.7, and the
  chain's RIP-7212 precompile.
- The **authenticators** that hold your keys, and — for synced passkeys — the
  Apple, Google or password-manager account behind them.
- **The code of the app you sign with.** It builds the transaction and shows you
  what it does. A compromised app can show one thing and ask you to sign another;
  the authenticator's prompt will not tell you the difference.
- The **RPC endpoints** you read from (a pool with failover; you can set your own).
- The **relay**, for liveness only: it can delay or refuse, not alter or steal.

**Threats considered**

- **Lost or stolen device** — a thief still needs to pass the authenticator's
  check; another key restores access.
- **Phishing** — a passkey cannot be typed into a fake site and is not offered to
  other domains.
- **Malicious dApp** — addressed by clear signing and the approval guard. A dApp
  can request an ordinary call to your own account (for example, adding an
  owner); Vela decodes it but does not yet block it.
- **Compromised backend service** (relay, index, chain data, exchange rates) —
  no signing power. The worst cases are refusing service, and — for the
  chain-data service — serving misleading descriptors, which is why fetched
  descriptors are not treated as authenticated and why the service can be
  replaced.
- **Compromised app delivery** — a tampered web deployment, extension update or
  app build could present a malicious transaction for you to sign. This is the
  [Bybit](/docs/bybit-attack) class of attack. Mitigations today: the
  decoding and approval guard in the app itself, store and notarized builds, and
  building the extension or apps from source yourself. An independent signing
  page that does not share the app's code is built but not yet connected.
- **Loss of the domain** — whoever controls `getvela.app` could serve pages that
  request signatures from Vela passkeys. Apps with their own code (the extension,
  self-built apps) do not depend on what the domain serves.

## Recovery

Creating a wallet publishes its public keys and address to a public
**registry contract** on Gnosis (copyable to Ethereum). On a new device you sign
in with **any one** key; the app finds the wallet through the index or, failing
that, directly from the registry, and checks that the keys recompute to the
recorded address. A single-key wallet can also be rebuilt from two signatures
with no registry at all.

<Callout type="warning" title="Your keys are your recovery">
There is no seed phrase, no social recovery and no guardian — nothing Vela could
lose, leak or be compelled to use. If every founding key is lost, the wallet
cannot be recovered. Create the wallet with more than one key, keep passkey sync
on if you rely on it, and secure the account behind it.
</Callout>

Details: [recovery & sign-in](/docs/recovery).

## If Vela disappears

Your funds stay in your Safe on-chain. The contracts don't depend on Vela, and
every service Vela runs can be replaced. The one thing that cannot move is the
passkeys' relying party, `getvela.app`: a copy of the web wallet on another
domain creates a different wallet. For existing wallets, the Vela browser
extension (which can use `getvela.app` passkeys by permission) and apps you build
yourself (with a phone or security key) keep working without getvela.app. The
[self-hosting guide](/docs/self-hosting#if-getvela-app-disappears) spells out
each path and its limits. Independent access to a chain also requires that chain
to support RIP-7212.

## Privacy

No account, no email, no KYC. What becomes public is written to the registry when
you create a wallet: each key's public key and credential ID, the authenticator
model, your wallet name and key labels, the address, and the signed registration
data. Vela's index sees that record before submitting it; Vela's relay sees your
address and the operations you submit, and keeps them for a limited time to
retry and diagnose. The website uses cookieless analytics. The
[privacy policy](/privacy) is the authoritative list.

## Open source

The wallet (all apps and the core), the relay and the exchange-rate service are
MIT-licensed; the chain-data directory is MIT as well. The public-key index is
public but does not yet carry a licence file. Code:
[github.com/mondaylabsltd](https://github.com/orgs/mondaylabsltd/repositories).

## No token

Vela has no token and no plans for one. There is nothing to buy, farm or
speculate on. Fees are paid in each network's coin or a stablecoin.

## Audit status and limitations

Safe's contracts, its 4337 and passkey modules, and EntryPoint v0.7 are
independently audited and widely used. **Vela's own code — the apps, the backend
services and the registry contract — has not had an independent third-party
audit, and none is scheduled**; a professional audit is a goal for when the
project can fund one, not a commitment with a date. Until then, review is
informal: the code is open, capable members of the community read it, and it is
reviewed with AI tools. That helps; it is not equivalent to a professional audit.
Treat Vela as alpha software. Details:
[audits & known issues](/docs/security-audits).

## References

- ERC-4337 — Account abstraction via the EntryPoint
- EIP-1271 — Signature validation for contracts
- ERC-7730 — Clear-signing descriptors
- EIP-5792 — Wallet call batching (`wallet_sendCalls`)
- RIP-7212 / EIP-7951 — P-256 signature verification precompile
- WebAuthn / FIDO2 — Passkeys
- [Safe smart account v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
