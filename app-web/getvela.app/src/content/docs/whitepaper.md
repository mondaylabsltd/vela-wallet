---
title: Whitepaper
description: "How Vela works and what you do — and don't — have to trust to use it: the account, the keys, the fee, the threat model, recovery, and what happens if Vela disappears."
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Status: alpha · last revised November 2026">
This page describes how Vela works and what you do and don't have to trust to
use it. Vela is in <a href="/blog/vela-is-in-alpha">alpha</a> — start with small
amounts. Vela has no token. Everything here can be checked against the
open-source code; where the code and this page disagree, the code is right and
this page is a bug. Last verified against commit <code>fa6083c87</code> (the
development branch, which includes the unreleased multisig feature). The
reference threat model, with a code location and a test for every claim, is
<a href="https://github.com/mondaylabsltd/vela-wallet/blob/main/docs/SECURITY-MODEL.md">SECURITY-MODEL.md</a>.
</Callout>

## Summary

Vela is a **self-custodial smart-contract wallet** for Ethereum and other EVM
networks. Each wallet is an unmodified **Safe v1.4.1** account, operated through
**ERC-4337**, and controlled by up to seven **passkeys** — WebAuthn P-256 keys
held by your devices, your password manager, or hardware security keys. There is
no seed phrase.

Vela, the company, never holds your keys and has no role on your Safe, so it
**cannot move, freeze or seize your funds** by itself. It does write and serve the
software that asks your keys to sign — which is why the threat model below
matters. The apps, the relay that submits transactions, and the supporting
services are open source, and you can run your own copy of each. What you
trust, in short: the contracts, the authenticators that hold your keys, the code
of the app you sign with, the domain your passkeys belong to, and the services you
point the app at.

A Vela account can also be an owner of a **multisig**: a separate Safe that
several people's accounts (or other wallets) control together, M of N. Multisig
is built but **not released yet**; it is described below where it changes the
picture.

## Why Vela exists

- **Seed-phrase wallets** put a 12–24 word secret in front of every user: a single
  point of failure and a standing phishing target.
- **Custodial wallets** remove the seed phrase by taking custody of the funds.
- **Passkey wallets** that depend on one company's servers and closed code remove
  the seed phrase but leave you stranded if the company goes away.
- **Blind signing** — approving opaque data you can't read — is still common, and
  it is one of the ways wallets get drained.

Vela aims for the convenience of a passkey with none of those dependencies: a
standard account, open code, replaceable services, and transactions you can read
before you sign.

## Design principles

1. **Self-custody, no exceptions.** Keys are created and held by your
   authenticators. Vela's services never see them; what they do see is listed
   under Privacy.
2. **Standard contracts, unmodified.** No contract in the path to your funds was
   written by Vela.
3. **Verify, don't trust.** The apps and services are public; the services can be
   self-hosted.
4. **Decode before signing.** What can't be decoded carries an explicit
   blind-signing warning.
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
  Safe passkey module verifies P-256 via the EIP-7951 / RIP-7212 precompile
```

Supporting services, all open source: a **public-key index** that registers new
wallets in an on-chain registry and answers lookups, a **chain-data** directory,
and an **exchange-rate** feed. For multisig (unreleased), a **genesis service**
records a multisig's setup and runs a **mailbox** where members leave proposals
and approvals, and the **PeerConnect** relay carries an outside owner's wallet
session. An optional **signing page** at sign.getvela.app can show and sign a
request on its own. See the [self-hosting guide](/docs/self-hosting).

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

Every signature needs the authenticator's own user verification — a biometric or
device PIN, or a security key's PIN and touch. There is no session key. Keys cannot be added, removed or replaced
later: the address on every chain where the wallet is not yet deployed still
stands for the founding set, so changing owners on one chain would make the
account differ from chain to chain.

Passkeys belong to a relying party — Vela's are created for **`getvela.app`**.
Browsers offer them only to pages on getvela.app or its subdomains (or to origins
getvela.app declares as related), and a phone's built-in passkeys work only in apps
getvela.app vouches for. That is what makes them resistant to phishing by
websites. It does not make them resistant to software you run: Chrome lets an
extension with permission for getvela.app use them, and a program on your
computer can ask a security key or a phone for a getvela.app signature directly.
The domain is also a dependency this paper returns to below.

### Signing flow

1. **Build** a UserOperation for your Safe — including a transfer that pays the
   relay — and simulate it.
2. **Decode** it into human-readable intent and show it to you.
3. **Sign**: your authenticator produces a WebAuthn assertion over the operation
   hash after verifying you.
4. **Encode** the assertion as the Safe signature the passkey module expects.
5. **Submit** the signed operation to the relay, which calls the EntryPoint.
6. **Verify on-chain**: the passkey module checks the P-256 signature with the
   EIP-7951 / RIP-7212 precompile before the Safe executes anything. There is no fallback
   verifier; a network without the precompile cannot be added.

### Fees

- The relay is paid **in band**: the operation declares zero EntryPoint fees and
  includes a transfer from your Safe to the relay's address. The amount and
  recipient are part of what you sign, so you pay exactly what the confirm screen
  showed.
- Where the relay bills the gas an operation uses, the fee is
  **`settlementGas × inBandFeePerGas`**: the gas the relay says it will bill,
  times the per-gas price it publishes for the chosen speed, with a minimum of
  about $0.01. The wallet checks both. It refuses a `settlementGas` above the
  smaller of the gas limits the operation is signed with and 125% of the wallet's
  own gas model plus 50,000, and a per-gas price more than three times the same
  formula applied to its own gas-price reading. Against a relay that publishes
  neither figure, the fee is three times the operation's gas limits at the higher
  of the two gas prices (two times on Tempo). Either way the fee is above the
  on-chain cost, and the relay keeps the difference. The exact amount is on the
  confirm screen before you sign, and from the moment you confirm, the amount and
  fee you pressed are the ones signed, or nothing is.
- The fee goes to whichever relay the wallet is set to: Vela's by default, or any
  vela-relay deployment, including one you run.
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
A fetched descriptor is never labelled verified — only one built into the app,
or a fetched one identical to it, earns that word — and only those can mark a
call as low-risk (green). A typed-data message is not "just a signature": a
permit or an order can move assets, and it gets the same checks.

A transaction is **simulated** before you sign, and the result counts only when
two independent RPC providers agree on it (or your own node, if you chose "Only
use my node"). When Vela could not read something it checks — no simulation,
one node answering, two nodes disagreeing, a simulation that fails, a call
nobody can decode — the confirm button says "… anyway", is styled as a danger,
and has to be **held for 1.5 seconds**. **Refuse blind signing** (Settings →
Security, off by default) refuses all of those instead.

An approval at the "unlimited" level (2^128 or more; 2^200 in v0.9.8 and
earlier) is shown in red with a cap on offer; unless you cap it, it is sent
exactly as the dApp built it. One at a hundred times your balance or more is
called out too. Approvals hidden inside a batch or a `multicall` are found and
checked the same way. Signed permits can't be capped, so they are signed as
asked or rejected. Details: [clear signing](/docs/clear-signing).

### Networks

Vela shows 20 networks by default — Ethereum, BNB Chain, Base, Polygon,
Arbitrum, Avalanche, Optimism, Monad, Robinhood Chain, Arc, X Layer, Mantle,
Gnosis, Tempo, World Chain, Celo, Unichain, Ink, Soneium and Kaia — and accepts
any EVM network that has the twelve contracts it checks for and the EIP-7951 /
RIP-7212 precompile. Stable, MegaETH, Plume and XRPL EVM are still supported,
hidden by default, and can be added back in Settings → Add network. Two of the twelve are
Safe's passkey signer factory and the signer code it deploys, which only a
wallet with more than one key needs; the check reports them separately.

## Security model

**What Vela cannot do**

- Move, spend or freeze your funds on its own — only your keys authorize your
  Safe, and Vela holds no role on it. (What Vela can do is ship software that asks
  you to sign; see the threats below.)
- Change a transaction after you sign it — any change invalidates the signature.
- Read your private keys — they stay in your authenticators.
- Add a key to your wallet, or remove one.

**What "cannot freeze" does not cover: the token.** USDC, USDT and most
fiat-backed tokens let their issuer blacklist any address, including yours. That
power belongs to the issuer and exists whatever wallet you use. What
self-custody gives you is that Vela is not a second party who can.

**What you trust**

- The **contracts**: Safe, its 4337 and passkey modules, EntryPoint v0.7, and the
  chain's EIP-7951 / RIP-7212 precompile.
- The **domain**: any page served from getvela.app or one of its subdomains can ask
  your keys for a signature.
- The **authenticators** that hold your keys, and — for synced passkeys — the
  Apple, Google or password-manager account behind them.
- **The code of the app you sign with.** It builds the transaction and shows you
  what it does. A compromised app can show one thing and ask you to sign another;
  the authenticator's prompt will not tell you the difference.
- The **RPC endpoints** you read from: a lying node can show wrong balances and
  history. It cannot make a simulation or a safety check read as clean on its
  own, because those need two independent providers to agree — but two providers
  that are really one, or that collude, can. You can set your own node.
- The **chain-data and exchange-rate services**: they supply token lists,
  descriptors, the fee-token list and the rates used to turn a fiat amount into a
  token amount.
- The **relay**: it can't change what you signed, but it can delay or refuse it,
  and choose when it lands — within the **30 minutes** every operation is valid
  for, after which the chain refuses it. So it could still trade ahead of a swap
  within your slippage inside that window. It sets the gas price your fee is
  based on, up to three times the wallet's own reading, and the gas it bills is
  capped as above.
- The **public-key index**, only for finding your wallet on a new device: its
  answer is checked against the registry contract on chain, and a wallet with
  several keys that only the index vouches for shows no address to receive at
  until the chain confirms it.

**Threats considered**

- **Lost or stolen device** — a thief still needs to pass the authenticator's
  check; another key restores access. But a key cannot be removed: if one may be in
  someone else's hands, move your funds to a new wallet, because the old address
  stays spendable by that key on every network.
- **Phishing by a website** — a passkey cannot be typed into a fake site, and
  browsers offer it only to pages on getvela.app and its subdomains (or origins it
  declares as related).
- **Software you run** — not covered by the passkey itself. A browser extension
  with access to getvela.app, an app getvela.app vouches for (today only Vela's
  own app, on its release key), or a program that talks to a security key or phone
  directly (a CTAP client, which writes the origin into the signed data itself)
  can ask for a getvela.app signature. The system prompt names the domain, not
  the transaction, so whatever you approve there is signed.
- **Malicious dApp** — addressed by clear signing, simulation, the approval
  guard, and refusals with no "anyway": a request for a call from your Safe to
  itself — `enableModule`, `addOwnerWithThreshold`, `swapOwner`,
  `setFallbackHandler`, `setGuard` and the rest of that family — is blocked,
  including inside a batch or a `MultiSend`, as is any leg carrying a
  `delegatecall` and a `SafeTx` typed-data signature. The same refusal covers any
  Safe your account is an owner of (a multisig): a site cannot use your account's
  authority to execute, approve or sign for it. Any one of them, signed once,
  would hand over an account as completely as the Bybit payload did, so the wallet
  does not offer them to be signed at all. A "Sign in with Ethereum" request for a
  different site, address or network is refused too. What the sheet showed is
  what is signed: the wallet refuses to sign an operation whose calls or fee
  differ from the ones reviewed.
- **Lying RPC node** — it can show you wrong balances. It cannot, alone, make a
  simulation or a multisig's safety checks read as clean: those need a second,
  independent provider to agree, and otherwise the confirm is "… anyway".
- **Compromised backend service** (relay, index, chain data, exchange rates) — no
  signing power, but real influence: refusing service, misleading descriptors or
  token lists, wrong exchange rates that change how much a fiat amount sends, and
  (for the relay) the timing and gas price above. A description fetched from the
  chain-data service is never called verified — only one built into the app, or
  a fetched one identical to it, earns that word, and the rest are shown with a
  line saying nothing authenticated them. Each service can be replaced.
- **Compromised app delivery** — a tampered web deployment, extension update or
  app build could present a malicious transaction for you to sign. This is the
  [Bybit](/docs/bybit-attack) class of attack, and **for a wallet whose keys
  belong to getvela.app, Vela does not stop it**: a compromised wallet can ask
  your passkey for a signature directly. What exists: the web core (the compiled
  code that makes every decision) rebuilds byte for byte from the source and is
  attested by CI; release packages carry GitHub build-provenance attestations
  (macOS images are notarized; the Windows installer is not code-signed); and you
  can build the extension or apps yourself. The
  [Trusted Signer](/docs/clear-signing-self-host), an independent signing page
  that does not share the app's code, can be chosen as the way you sign on every
  surface. Every app checks the page's bytes against the versions it ships with
  and refuses one that doesn't match. For a getvela.app wallet it guarantees that
  a tampered *interface* cannot make you approve one thing and sign another, but
  not that a fully compromised app can't sign without it. Only for a wallet made
  on a self-hosted page, whose keys belong to that page's domain, is the page the
  one thing that can sign.
- **Anything served from the domain** — any page on getvela.app or its
  subdomains, including a script it loads, could request signatures from Vela
  passkeys, and the prompt shows only "getvela.app". The website therefore forbids
  its own pages from using passkeys, and keeps its analytics script off the page
  that holds a key. If the domain changed hands, its new owner would also control
  which apps may use the passkeys. The extension and self-built apps carry their
  own code, though by default they still fetch descriptors and use services under
  getvela.app.

### Multisig (not released)

A Vela multisig is an unmodified Safe v1.4.1 whose owners are Vela accounts,
ordinary wallets, or other contracts, any M of N. Its members, threshold and
settings are read from the chain (two providers again), never taken from Vela's
service. Each member's device decodes, simulates and checks the transaction
itself and recomputes the hash it approves; a change to who controls the
multisig that the transaction does not declare is refused with no override.
Vela signs only safe shapes: no `delegatecall` except into Safe's
MultiSendCallOnly, no gas refunds out of the multisig, no module, guard or
fallback-handler changes. The **mailbox** is not trusted: everything read from it
is recomputed, a stranger's post is kept out of the list and the counts, and it
cannot hold up the next transaction. But the mailbox needs no login, and anyone
who knows a multisig's address can read its pending proposals.

## Recovery

Creating a wallet publishes its public keys and address to a public
**registry contract** on Gnosis (copyable to Ethereum). On a new device you sign
in with **any one** key; the app finds the wallet through the index or, failing
that, directly from the registry, and checks that the keys recompute to the
recorded address. The index's answer is checked against the registry on chain;
if no chain can confirm a wallet with several keys yet, you are signed in but the
wallet shows no address to receive at until it is confirmed. A single-key wallet
can also be rebuilt from two signatures with no registry at all.

<Callout type="warning" title="Your keys are your recovery">
There is no seed phrase, no social recovery and no guardian — nothing Vela could
lose, leak or be compelled to use. If every founding key is lost, the wallet
cannot be recovered. Create the wallet with more than one key, keep passkey sync
on if you rely on it, and secure the account behind it.
</Callout>

Details: [recovery & sign-in](/docs/recovery).

## If Vela disappears

Your funds stay in your Safe on-chain. The contracts don't depend on Vela, and
every service Vela runs is open source for someone else to run. The one thing that cannot move is the
passkeys' relying party, `getvela.app`: a copy of the web wallet on another
domain creates a different wallet. For existing wallets, the Vela browser
extension (which can use `getvela.app` passkeys by permission) and apps you build
yourself (with a phone or security key) keep working without getvela.app. The
[self-hosting guide](/docs/self-hosting#if-getvela-app-disappears) spells out
each path and its limits. Independent access to a chain also requires that chain
to support EIP-7951 / RIP-7212.

## Privacy

No account, no email, no KYC. What becomes public is written to the registry when
you create a wallet: each key's public key and credential ID, the authenticator
model, your wallet name and key labels, the address, and the signed registration
data. Vela's index sees that record before submitting it, and the addresses you
look up names for; Vela's relay sees your address, the operations you submit and
the RPC endpoint your app uses (including any API key in its URL), and keeps
operations for a limited time to retry and diagnose. Every service sees your IP
address. The multisig mailbox (unreleased) stores proposals and approvals with
no login: anyone who knows a multisig's address can read its pending
transactions. Bug reports become public GitHub issues.

The website uses cookieless analytics (Rybbit). The web wallet and the browser
extension send usage statistics **only if you turn them on** (off by default),
and never anything about a transaction, an address or an amount. The iPhone,
Android and desktop apps send none. The [privacy policy](/privacy) is the
authoritative list.

## Open source

Everything is MIT-licensed: the wallet (all apps and the core), the relay, the
public-key index, the exchange-rate service and the chain-data directory. Code:
[github.com/mondaylabsltd](https://github.com/orgs/mondaylabsltd/repositories).

## No token

Vela has no token and no plans for one. There is nothing to buy, farm or
speculate on. Fees are paid in each network's coin or a stablecoin.

## Audit status and limitations

Safe's contracts, its 4337 and passkey modules, and EntryPoint v0.7 are
independently audited and widely used. **Vela's own code — the apps, the backend
services, the key registry contract and the multisig SafeGenesisRegistry — has
not had an independent third-party audit, and none is scheduled**; a professional audit is a goal for when the
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
- EIP-7951 / RIP-7212 — P-256 signature verification precompile
- WebAuthn / FIDO2 — Passkeys
- [Safe smart account v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
