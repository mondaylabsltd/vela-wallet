---
title: The account contract
description: "Your Vela wallet is an unmodified Safe v1.4.1. No contract in the path to your money was written by Vela — here is exactly which contracts it is, what that buys you, and what it costs."
---

# The account contract

Your wallet is not an app's private data structure. It is a **Safe v1.4.1**
smart account — the contract many large on-chain treasuries use — deployed
exactly as Safe publishes it, with no modification.

## Nothing in the path is ours

Every contract that can touch your money was written by Safe or by the ERC-4337
authors:

| Contract | Role in your wallet | Written by |
| --- | --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) (SafeL2, via a proxy) | The account itself; owners, threshold, execution | Safe |
| [Safe 4337 Module v0.3.0](https://github.com/safe-global/safe-modules/tree/4337/v0.3.0/modules/4337) | Lets the EntryPoint operate the Safe; also its fallback handler | Safe |
| [SafeWebAuthnSharedSigner v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) | Verifies the first key's P-256 signatures | Safe |
| [SafeWebAuthnSignerFactory v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) and the signers it creates | One small signer contract per additional key | Safe |
| [ERC-4337 EntryPoint v0.7](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0) | Runs your signed operation | ERC-4337 authors |

Vela's own contracts are not in this list: the **public-key registry** that
records each wallet's keys so a new device can find it
([recovery](/docs/recovery)), its companion domain registry, and the earlier
index they replaced. They hold no funds and have no role in your Safe.

The wallet repository contains no Solidity at all — you can check that in one
command:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # prints nothing
```

Vela holds no privileged role on your account: no admin key, no upgrade path,
no module it can add. Only your keys can change your Safe.

## Why "unmodified" is the word that matters

Plenty of wallets are built on "a Safe", a fork of Safe, or a Safe-inspired
account. The difference matters in three ways.

**The audits apply to what you are actually using.** Safe's audits cover these
releases, or earlier ones they differ from only by small documented changes (the
[audits page](/docs/security-audits) has the details). A fork's audits cover the
code before the fork; the modification is the part nobody audited.

**The ecosystem treats your account as a Safe, because it is one.** Block
explorers decode it, and Safe's tooling can read it and build transactions for
it. To *sign* those transactions, though, a program has to be able to ask your
key for a signature for `getvela.app`, the domain your passkeys belong to — so
Safe's own web app, served from another domain, can't sign for you. The
[self-hosting guide](/docs/self-hosting#if-getvela-app-disappears) lists what can.

**The attack surface is one many others are also watching.** A bespoke account
contract is watched mostly by its author. Safe's core contracts are watched by
everybody who holds money in a Safe; the 4337 and passkey modules have a smaller
audience, but a real one.

## What it costs

Being standard is not free:

- **Gas.** Your signature is verified on-chain and the transaction runs through
  the EntryPoint. A simple send from a deployed Vela wallet used roughly
  140,000–170,000 gas on-chain in our measurements on Gnosis (September 2026); a
  plain ETH transfer from an ordinary account uses 21,000. On top of the gas, the relay charges its fee — see
  [networks & fees](/docs/networks-and-fees).
- **The account must be deployed.** Your address is computed with `CREATE2`
  before anything exists on-chain, so you can receive at it immediately; your
  first outgoing transaction on each network pays to deploy the contract.
- **Not every chain qualifies.** Passkey signatures are verified with the
  **RIP-7212** precompile, and its address is part of every wallet's setup data, so
  a network without it cannot run Vela at all.
- **Safe's risk is now your risk.** Trusting a widely used contract is still
  trusting a contract. Vela has not added a second contract of its own for you
  to trust in the path to your money.

## What is and is not audited

Safe's contracts, its 4337 and passkey modules, and EntryPoint v0.7 have
published third-party audits. **Vela's own code — the apps, the backend services
and the registry contract — has not had a third-party audit, and none is
scheduled**; it is a goal for when the project can fund one, not a commitment
with a date. Every contract, its audit report, and the issues we track are in
[audits & known issues](/docs/security-audits).

## See for yourself

Your account is on-chain. Open your address in a block explorer once it is
deployed: it is a Safe proxy whose implementation is Safe's canonical SafeL2
v1.4.1 deployment, on every network.

Next: [audits & known issues](/docs/security-audits).
