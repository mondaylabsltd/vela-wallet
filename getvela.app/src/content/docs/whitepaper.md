---
title: Whitepaper
description: How Vela works and what you do — and don't — have to trust to use it. Architecture, the security model, recovery, and how to verify it all yourself.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Status: alpha · v0.1">
This page describes how Vela works today and what you do and don't have to trust
to use it. It favors honesty over marketing. Vela is in <a href="/blog/vela-is-in-alpha">alpha</a> —
start with small amounts. Vela has no token. Everything here is verifiable against
the open-source code.
</Callout>

## Summary

Vela is a **self-custodial smart-contract wallet** for EVM networks. Each wallet
is a [Safe](https://github.com/safe-fndn/safe-smart-account) smart account
controlled by a **passkey** — a WebAuthn (P-256) credential held in your device's
secure hardware and unlocked with Face ID, Touch ID, or a fingerprint. There are
no seed phrases and no private keys for you to copy, store, or lose.

Vela, the company, never holds your keys or your funds and **cannot move, freeze,
or seize them**. The app, the transaction bundler, and the supporting services
are all open source and self-hostable. What you trust reduces to audited smart
contracts, your operating system's passkey vault, and — for liveness only — a
relay you can replace or run yourself.

## Why Vela exists

Most wallets force a trade-off:

- **Seed-phrase wallets** put a 12–24 word secret in front of every user. It is
  the single point of failure and a constant phishing target.
- **Custodial wallets** remove the seed phrase but take custody of your funds —
  reintroducing the counterparty risk crypto was meant to remove.
- **Blind signing** — approving opaque hex you can't read — is normalized across
  the ecosystem and is behind a large share of drained wallets.

Vela aims to be as easy as a custodial app and as sovereign as a hardware wallet:
no seed phrase, no custody, and no transaction you can't read before you sign it.

## Design principles

1. **Self-custody, no exceptions.** Keys are generated and held by your device's
   secure element. Vela's servers only ever see public data.
2. **Verify, don't trust.** The entire stack — app and all three backend
   services — is open source under the MIT license.
3. **No blind signing.** Transactions are decoded into human-readable intent
   wherever a descriptor exists; unknown calls are flagged, not hidden.
4. **Do less.** The wallet holds ETH and ERC-20s and connects to dApps you
   choose. Less code to trust, a smaller attack surface.

## Architecture

```text
Vela App (iOS / Android / Web, one codebase)
  • Passkey (WebAuthn P-256, secure enclave)
  • UserOperation construction & signing
  • Clear-signing UI (ERC-7730)
        │  signed UserOperation
        ▼
Vela Bundler (ERC-4337, self-hostable)
  • Submits handleOps to the EntryPoint
  • Cannot alter or forge your transaction
        ▼
EVM chain
  EntryPoint v0.7 → Safe smart account
  WebAuthn signer verifies P-256 on-chain
```

### Account model

Your wallet is a **Safe v1.4.1** smart account (a proxy contract) operated through
**ERC-4337** account abstraction (EntryPoint v0.7) with the **Safe 4337 Module**
and a **WebAuthn signer** as the account's owner.

The address is **deterministic** and **counterfactual**: it is computed from your
passkey's public key via `CREATE2` before any transaction is sent, so you can
receive funds at it before it is ever deployed. The account deploys itself, paid
from its own balance, on your first transaction.

### Keys and authentication

Authentication uses **WebAuthn passkeys** on the **P-256** curve. The private key
is generated inside, and never leaves, your device's secure hardware, and is
synced by your OS passkey provider (iCloud Keychain or Google Password Manager).
**Vela's servers only ever see your public key.** Signing requires a fresh
biometric verification every time — there is no long-lived session key. See
[how passkeys work](/docs/passkeys) for the full detail.

### Signing and transaction flow

1. **Construct** an ERC-4337 `UserOperation` for your Safe and estimate gas.
2. **Decode** the call into human-readable intent and show it for review.
3. **Sign** — your secure element produces a WebAuthn assertion over the
   operation hash after biometric verification.
4. **Encode** the assertion as an **EIP-1271** contract signature.
5. **Relay** the signed operation to the bundler, which submits it to the
   EntryPoint.
6. **Verify on-chain** — the Safe verifies the P-256 signature on-chain (using the
   RIP-7212 precompile where the chain provides it) before executing.

The bundler receives an **already-signed** operation. It cannot change the
recipient, amount, or any other field without invalidating the signature.

### Bundler and gas model

- Each Safe gets a **dedicated relayer account** (gas account) per chain,
  activated by a **non-refundable** deposit. It tops itself up from EntryPoint gas
  refunds.
- Gas is paid **from your own wallet's balance** — there is **no paymaster** and
  no third party sponsoring (or gating) your transactions.
- The relayer fee is a transparent markup over on-chain gas price
  (`maxFeePerGas = gasPrice × 1.6`), shown in full before you confirm.
- That gas account can still run down over time, so it may need **re-activating
  again later** — it isn't strictly a one-time deposit.

The bundler is a **liveness** dependency, not a **custody** one: it can delay or
decline to relay, but it can never alter, forge, or steal. It is open source and
you can run your own. See [networks & fees](/docs/networks-and-fees).

### Clear signing (ERC-7730)

Vela decodes calldata and EIP-712 typed data using **ERC-7730** descriptors and
renders the **intent** (Swap, Send, Approve…), the **substance** (amounts,
addresses), and the **details** (nonce, deadline, raw calldata) on demand,
color-coded by risk. When no descriptor matches, Vela shows an explicit
blind-sign warning rather than pretending to understand the call.

### Networks

Vela supports 8 EVM networks — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism,
Base, Avalanche, and Gnosis — plus custom networks via configurable RPC endpoints.

## Security model

**What Vela cannot do:**

- Move, spend, or transfer your funds — only your passkey can authorize the Safe.
- Freeze or seize your account — the Safe is your contract on-chain; Vela has no
  privileged role on it.
- Sign on your behalf — every transaction needs a fresh biometric assertion.
- See your private key — it never leaves your device's secure hardware.
- Alter a transaction after you sign — any change invalidates the signature.

**What you do trust:**

- The **Safe contracts** (audited, widely used) and the WebAuthn signer that
  verifies your P-256 key.
- Your **OS passkey provider** (Apple / Google) to protect and sync your
  credential.
- The **RPC providers** you query (Vela uses a multi-source pool with failover;
  you can set your own).
- The **bundler** for liveness only — and you can self-host it.

**Threats considered:**

- **Lost or stolen device** — a thief still needs your biometrics/PIN to sign.
- **Phishing / malicious dApp** — addressed by clear signing.
- **Compromised Vela server** — yields no signing ability; the blast radius is
  degraded service, not loss of funds.
- **Supply-chain risk** — mitigated by open source and self-hosting.

## Recovery

Your passkey is backed up by your OS provider; on a new device, signing in with
the same Apple or Google account restores it, and your wallet reappears.

<Callout type="warning" title="Your platform passkey backup is your recovery">
Vela's recovery is your passkey, synced by iCloud Keychain or Google Password
Manager. By design there is no seed phrase, no social recovery, and no guardian —
nothing Vela could lose, leak, or be compelled to act on. The flip side is real:
if you lose <strong>both</strong> your device <strong>and</strong> your
cloud-synced passkey, with no other copy, the account cannot be recovered. Keep
your platform's passkey backup enabled and its account secured.
</Callout>

The full recovery model, including the honest limits, is in
[recovery & sign-in](/docs/recovery).

## If Vela disappears

Self-custody means your keys and funds do not depend on Vela being online. Funds
live in **your Safe contract on-chain**, and the bundler is open source and
replaceable.

One honest caveat: WebAuthn ties a passkey to a relying-party domain
(`getvela.app`). If that domain were permanently lost, passkeys bound to it would
need help to work elsewhere. Vela ships an open-source WebAuthn proxy browser
extension for exactly that disaster-recovery case — today a developer/recovery
tool rather than a polished consumer flow. Independent on-chain access also
depends on the destination chain's P-256 (RIP-7212) support, which is improving
across chains.

## Privacy

No accounts, no email, no KYC, no seed phrase to collect. Servers store only your
**public key** and a chosen account name (for cross-device recovery), published
on-chain by design. Transaction contents are not logged. The website uses
cookieless, self-hosted analytics. See the [privacy policy](/privacy).

## Verifiability and open source

Everything is **MIT-licensed and open source** — the app and all three backend
services (chain data, passkey index, bundler), which you can **self-host**
(Settings → Advanced → Service Endpoints). Read the code at
[github.com/atshelchin/vela-wallet](https://github.com/atshelchin/vela-wallet).

## No token

Vela has **no token** and no plans for one. There is nothing to buy, farm, or
speculate on. Gas is paid in each network's native asset.

## Audit status and limitations

The **Safe contracts** at the core of every Vela account are independently
audited and battle-tested. Vela's **own integration** around them has **not yet
undergone an independent third-party audit** — one is planned. Until then, treat
Vela as alpha software and use amounts you are comfortable putting into something
this young.

## References

- ERC-4337 — Account Abstraction via EntryPoint
- EIP-1271 — Standard signature validation for contracts
- ERC-7730 — Clear-signing / structured-data descriptors
- EIP-5792 — Wallet call batching
- RIP-7212 — Precompile for secp256r1 (P-256) signature verification
- WebAuthn / FIDO2 — Passkey authentication
- [Safe smart account v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
