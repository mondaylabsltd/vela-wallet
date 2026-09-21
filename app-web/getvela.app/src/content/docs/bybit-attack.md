---
title: The Bybit attack, and the path it used
description: In February 2025 Bybit lost about $1.5 billion. The Safe contracts were not broken — the interface was. This page explains the path, and what in Vela's design closes it.
---

# The Bybit attack, and the path it used

On 21 February 2025, Bybit lost roughly **$1.5 billion** from a Safe multisig
cold wallet. It is the largest theft in the history of the industry, and it is
worth reading carefully, because almost everything about it was *correct*
except one thing.

## What happened

The short version, from the public post-mortems:

1. An attacker compromised a **`Safe{Wallet}` developer's machine** and injected
   malicious JavaScript into the AWS S3 bucket serving the `Safe{Wallet}`
   front-end. The code went in on 19 February and was triggered on 21 February,
   targeted at Bybit's specific Safe.
2. Bybit's signers opened the interface and reviewed a transaction that looked
   ordinary.
3. The payload actually sent to their **hardware wallets** was not that
   transaction. It was a `delegatecall` that overwrote the Safe proxy's
   `masterCopy` — slot 0 — replacing the entire account implementation with the
   attacker's.
4. The signers approved. The signatures were valid. The contract did exactly
   what it was told.

Attribution was made publicly to North Korea-linked activity (the FBI named the
TraderTraitor cluster).

## What was *not* broken

- **Not the Safe contracts.** They executed a validly signed instruction.
  No bug in Safe was exploited.
- **Not the cryptography.** Every signature was genuine.
- **Not the hardware wallets.** Ledger devices were in the loop and signed
  anyway — because a hardware wallet shows you what it is given, and what it
  was given was the malicious payload. A device that cannot decode a
  `delegatecall` into something a human can evaluate protects the *key*, not
  the *decision*.

What was broken is the assumption underneath every wallet interface: **that the
screen describing a transaction and the bytes being signed are the same thing.**

## Why this is the general case, not a freak event

Every signature you have ever produced in a web wallet rested on that
assumption. The interface builds the payload, the interface renders the
summary, and nothing independent checks that one matches the other. If the code
serving that interface is replaced — by a compromised build pipeline, a hijacked
CDN, a malicious dependency, a stolen deploy credential — the summary becomes
whatever the attacker wants it to be, and your signature is real.

This is the risk Vela's signing design is aimed at. Not phishing. Not a leaked
key. **A signing screen that is lying to you.**

## What Vela does about it

**Clear signing, to the calldata.** Every transaction is decoded into
human-readable intent before you approve — amount, recipient, what the call
actually does ([ERC-7730](/docs/clear-signing)). A call we cannot decode is
**flagged as undecodable**, not quietly rendered as if it were fine. The Bybit
payload was a `delegatecall` that swapped an implementation address; that is
precisely the shape of thing that should stop a signer dead, and hiding it
behind a friendly summary is how it did not.

Two limits to be exact about. A dApp cannot ask Vela for a `delegatecall` at
all — the requests a page can make produce ordinary calls — so the Bybit payload
itself could not arrive that way. But a page *can* ask for an ordinary call to
your own account, for example to add an owner or change a module. Vela decodes
such a call and shows it, like any other; it does not yet block it. And if Vela's
own code were replaced, as `Safe{Wallet}`'s was, the decoding would be the
attacker's too — which is what the next point is for.

**An independent path that can check the interface.** Vela has built a
zero-build, zero-dependency [signing page](/docs/clear-signing-self-host) that
decodes the request and performs the WebAuthn signature on its own — a single
folder of static files you can read end to end, serve yourself, or load as a
browser extension. Its purpose is to be a second opinion that does not share the
main app's supply chain. *Status: built and tested; not published, and no Vela
app sends requests to it yet.* This page will say so plainly when that changes.

**No admin role for us to lose.** Vela's accounts are
[unmodified Safe v1.4.1](/docs/account-contract), and Vela holds no privileged
role on them — no admin key and no upgrade path of ours that we could be coerced
or compromised into using. Be clear about what that does *not* remove: the
primitive Bybit's attackers used — an owner-signed `delegatecall` that rewrites
the account's implementation — still exists in every Safe, Vela's included
(Vela's own batched transactions use `delegatecall` into Safe's MultiSend). It
takes a valid signature from one of your keys. The defences against being
talked into giving one are the decoding above and the independent check.

**A fresh check for every signature.** Every signature needs your key's own
confirmation — Face ID, a fingerprint, a PIN, or a touch on a security key.
There is no long-lived session key, so there is no window in which something can
sign on your behalf without you present.

**Self-hosting as the backstop.** The apps and the backend services are open
source. If you do not want to trust our build pipeline at all, build the
extension or an app yourself and run the services you need — the
[self-hosting guide](/docs/self-hosting) walks through it. That is the only
answer to this class of attack that does not require trusting somebody.

## What Vela does not claim

Vela's front-end could be compromised the same way `Safe{Wallet}`'s was. Our code
is not audited. Saying otherwise would be exactly the kind of assurance this
incident should have ended.

What the design tries to do is narrow the path: make the payload legible
instead of opaque, hold no admin role that could be abused on your behalf, and
give you a way to verify with something that is not us. The honest summary is that
**this class of attack is mitigated by design, not eliminated**, and the parts
that would harden it further are listed, unfinished, in
[audits & known issues](/docs/security-audits).

## Sources

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
