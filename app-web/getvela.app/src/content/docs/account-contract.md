---
title: The account contract
description: Your Vela wallet is an unmodified Safe v1.4.1. Nothing in the contract path was written by us — here is what that buys you, and what it costs.
---

# The account contract

Your wallet is not an app's private data structure. It is a **Safe v1.4.1**
smart account — the same contract used to hold treasuries far larger than
anything Vela will ever see — deployed exactly as Safe publishes it, with no
modification.

That sentence is short and the consequences are not, so this page spells them
out.

## Nothing in the path is ours

Four contracts stand between you and your money. Vela wrote none of them:

| Contract | Who wrote it |
| --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1) (the account itself, a proxy) | Safe |
| [Safe 4337 Module](https://github.com/safe-global/safe-modules/tree/main/modules/4337) | Safe |
| [SafeWebAuthnSharedSigner](https://github.com/safe-global/safe-modules/tree/main/modules/passkey) (verifies your P-256 key) | Safe |
| [ERC-4337 EntryPoint v0.7](https://eips.ethereum.org/EIPS/eip-4337) | The ERC-4337 authors |

There is no Vela contract. The repository contains no Solidity at all — you can
check that in one command:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # prints nothing
```

When Vela adds a network, it deploys **those** contracts at their canonical
addresses. It does not deploy a contract of its own design, and it holds no
privileged role on yours: no admin key, no upgrade path, no module we can add.

## Why "unmodified" is the word that matters

Plenty of wallets are built on "a Safe" or "a fork of Safe" or "a Safe-inspired
account". A fork is a new contract with an old reputation. The differences,
practically:

**The audits apply to what you are actually using.** Safe's audit reports cover
the bytecode at these exact releases. A fork's audits cover the code before the
fork. If a wallet has modified the account contract, every audit it cites is an
audit of something else, and the modification is the part nobody looked at.

**The ecosystem treats your account as a Safe, because it is one.** Block
explorers decode it. Safe's own transaction tooling understands it. If Vela is
gone tomorrow, your wallet is not an orphan format — it is the most widely
tooled smart account on Ethereum, and any Safe-compatible interface can drive
it. This is what makes ["if Vela disappears, your wallet does not"](/docs/why-vela)
a statement about contracts rather than about our intentions.

**The attack surface is one everybody else is also watching.** A bespoke
account contract is a contract only its author is looking at. This one is
looked at by everybody who holds money in a Safe.

## What it costs

Being standard is not free, and the trade-offs are real:

- **Gas.** A smart account verifies a signature on-chain. Expect roughly
  1.5–3× the gas of a plain EOA transfer, depending on the chain. See
  [networks & fees](/docs/networks-and-fees).
- **The account must be deployed.** Your address is computed with `CREATE2`
  before anything exists on-chain, so you can receive at it immediately, but
  the first outgoing transaction pays to deploy the contract.
- **Not every chain qualifies.** The WebAuthn signer verifies a P-256 signature
  on-chain, which needs the **RIP-7212** precompile. Vela refuses to enable a
  network without it rather than fall back to a weaker verifier.
- **Safe's risk is now your risk.** Trusting a widely-used contract is still
  trusting a contract. What Vela can say is that it has not added a second
  thing for you to trust on top.

## What is and is not audited

Safe's contracts and the WebAuthn signer module are audited, by third parties,
and those reports are public. **Vela's own app code has not been independently
audited**, and no audit is scheduled — it is a goal for when the project can
fund one, not a commitment with a date. Every contract Vela depends on, its
audit report, and the issues we track are listed in
[audits & known issues](/docs/security-audits).

## See for yourself

Your account is on-chain. Open it in a block explorer and read the
implementation address: it will be Safe's canonical deployment at v1.4.1, byte
for byte, on every network Vela supports.
