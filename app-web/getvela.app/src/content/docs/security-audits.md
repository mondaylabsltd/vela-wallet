---
title: Audits & known issues
description: "Every contract Vela depends on, who audited which version, whether the audited version is the one deployed, the open findings we are watching, and what has not been audited at all."
---

"Audited" is a claim about specific code at a specific version, so this page cites
the reports, the commits and the deployed addresses — and lists what is **not**
audited, which matters just as much.

Last verified on 8 November 2026, against commit `eebc246f4` (the development branch, which
includes the unreleased multisig feature) and release v0.9.8. Each audit date and finding below
was checked against the cited report. If you find an error, tell us and we will fix it. To
report a vulnerability, see the [security policy](https://github.com/mondaylabsltd/vela-wallet/blob/main/SECURITY.md).
The "Gaps in Vela's own defences" list was re-checked on 9 November 2026 against commit
`fa6083c87`. The threat model, with a code location for every claim, is
[SECURITY-MODEL.md](https://github.com/mondaylabsltd/vela-wallet/blob/main/docs/SECURITY-MODEL.md).

## The funds path

Every contract that can touch your money is a canonical deployment of
third-party code with published reviews.

### Safe v1.4.1 — the account itself

Your wallet is a [Safe](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
proxy using the SafeL2 singleton and SafeProxyFactory. Batches go through
MultiSend.

[Ackee Blockchain audited Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(final report 16 March 2023, fix review 28 March): 11 findings, none critical or
high; the two medium findings were acknowledged rather than changed. The scope
was SafeL2, SafeProxyFactory, CompatibilityFallbackHandler, MultiSendCallOnly and
SignMessageLib. v1.4.1 differs from v1.4.0 in one functional line, an ERC-4337
compatibility fix in module setup
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)); Safe
consulted Ackee and concluded no re-audit was needed. MultiSend is unchanged in
logic since v1.3.0, which [G0 Group audited](https://github.com/safe-global/safe-smart-account/tree/main/docs).
All addresses match [safe-deployments](https://github.com/safe-global/safe-deployments).
The core contracts are in scope of the
[Safe Foundation bug bounty](https://docs.safefoundation.org/security/bug-bounty),
whose highest tier pays up to $1,000,000.

The 2025 Bybit incident is not a contract finding: attackers tampered with the
JavaScript served to Safe's web interface, and Safe's
[forensic statement](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
found no vulnerability in the contracts. [Our page on it](/docs/bybit-attack)
explains why the same class of attack concerns every wallet interface, ours
included.

### Safe4337Module v0.3.0 — the ERC-4337 adapter

Deployed at `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` (Sourcify exact match),
and also set as your Safe's fallback handler. Reviewed three times —
[reports here](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md):

- **Ackee Blockchain**, final report March 2024: one warning (use of the
  compiler optimizer) acknowledged, nothing above it open.
- **Certora**, review 6–11 August 2026 of the v0.3.0 release commit (`5931a27`,
  fix review `f877239`), scoped to Safe v1.4.1 and later with EntryPoint v0.7: one
  **medium** finding, acknowledged and **not fixed** in v0.3.0. It is
  _authorization changes do not invalidate subsequent UserOperations already
  validated in the same bundle_; see "Known issues" below. There are also three
  informational findings, one of them fixed: a README example that swaps
  `validAfter` and `validUntil`.
- **Nethermind**, report dated 7 August 2026, of the same commit: no findings. The
  report says it covered the module's use with Safe **v1.3.0**. Vela runs it on
  Safe v1.4.1, which is the configuration the Certora review covers.

These two reviews are recent, but the code is not new: they review the March 2024
v0.3.0 release, unchanged. SafeModuleSetup v0.3.0 (`0x2dd6…5b47`), which enables
the module when a wallet is deployed, was in the scope of both.

The module's history has one disclosed issue: v0.1.0 did not sign `initCode` and
`paymasterAndData`, a gas-griefing vector
[fixed in v0.2.0](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module);
Safe reports v0.1.0 was not used outside testnets. Vela uses v0.3.0 with
EntryPoint v0.7 and Safe 1.4.1, the configuration the module's release describes.

### Safe passkey module v0.2.1 — the signers

Your first key is verified by **SafeWebAuthnSharedSigner** at
`0x94a4F6affBd8975951142c3999aEAB7ecee555c2`. "Shared" means the contract
deployment is shared, the way the Safe singleton is; your key is not. Each Safe
stores its own P-256 public key in its own storage.

Every additional key has its own signer contract, created by
**SafeWebAuthnSignerFactory** at `0x1d31F259eE307358a26dFb23EB365939E8641195`
as a proxy to the **SafeWebAuthnSigner singleton** at
`0x4E27b51350e6c2083EE19011120F50DAfEc5CA50`.

The reviews that cover these contracts at v0.2.1
([reports](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)):

- A [Hats Finance audit competition](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
  (June–July 2024): no high or medium findings; three low, all fixed.
- **Certora**'s review of the release commit: no new findings. (The earlier
  v0.2.0 audit notes the shared signer had not been audited yet — it was added
  after that audit.)
- **Nethermind**, report dated 7 August 2026: no findings. Unlike the Certora
  review, its scope includes SafeWebAuthnSharedSigner. Like its 4337 report, it
  says it covered use with Safe v1.3.0.

No contract-level vulnerability has been disclosed since release, and the
passkey contracts are in scope of the Safe Foundation bounty.

Passkey signatures are verified by the chain's **EIP-7951 / RIP-7212** precompile, with no
fallback verifier. Before enabling a network, the app checks the precompile with
a real signature. Two caveats: the original RIP-7212 specification has edge-case
flaws that [EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) fixes (they only
affect inputs that should fail anyway, not well-formed WebAuthn signatures), and a
probe cannot catch every way a chain's implementation might diverge.

### EntryPoint v0.7 — runs your operation

Deployed at `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, the
[canonical v0.7.0 release](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[Audited by OpenZeppelin](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
for the Ethereum Foundation (January 2024): no critical or high findings, five
medium, all 24 findings resolved; the fix-review commit matches the release. It
is in scope of the Ethereum Foundation's
[ERC-4337 bug bounty](https://docs.erc4337.io/community/bug-bounty) (up to
$250,000).

### Multisig: the contracts a Vela multisig uses (not released yet)

The multisig feature is on the development branch and is not in a release yet.
A Vela multisig is a standard Safe owned by Vela accounts and, optionally, other
addresses. It uses the same SafeL2 v1.4.1 singleton and SafeProxyFactory v1.4.1 as
the account above, but without the 4337 module. These contracts are added:

| Contract                                                                             | Role in a multisig                                                                                                                                                    | Audit status                                                                                                                                                                                                                                                                                                                      |
| ------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **CompatibilityFallbackHandler v1.4.1** `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` | The fallback handler of every multisig Vela creates; it gives the multisig's EIP-1271 answers                                                                         | In scope of [Ackee's Safe v1.4.0 audit](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md) (above)                                                                                                                                                                                                  |
| **MultiSendCallOnly v1.4.1** `0x9641d764fc13c8B624c04430C7356C1C7C8102e2`            | The only `delegatecall` target Vela will sign in a multisig transaction, used for batches                                                                             | In scope of Ackee's Safe v1.4.0 audit                                                                                                                                                                                                                                                                                             |
| **SafeToL2Setup v1.4.1** `0xBD89A1CE4DDe368FFAB0eC35506eEcE0b1fFdc54`                | Never used when Vela creates a multisig. It is accepted in a Safe **imported** from `Safe{Wallet}`, whose setup hook calls `setupToL2` to switch to the L2 singleton. | [Certora, Migration and Setup contracts](https://github.com/safe-global/safe-smart-account/blob/main/docs/Safe_Library_Contracts_Audit_Report_1_4_1.pdf) (review 18–23 August 2024, final report v3): no critical, high or medium findings; one low (the Solidity 0.7.6 compiler's known issues), acknowledged; six informational |
| **SafeGenesisRegistry v4** `0xF412388bF908E510066d3Be5c704bd9A327f945d`              | Vela's own contract: a public, write-once record of each multisig's creation parameters, so another device can rebuild the multisig from its address alone            | **Not audited**, see below                                                                                                                                                                                                                                                                                                        |

An imported Safe may also be v1.3.0 or v1.5.0. The wallet accepts the canonical
factories, singletons, fallback handlers and SafeToL2Setup of those releases, and
checks each one's runtime code hash. Safe's audits of those versions, including
G0 Group, Certora and Nethermind for v1.3.0 and Certora and Ackee for v1.5.0, are
in [safe-smart-account/docs](https://github.com/safe-global/safe-smart-account/tree/main/docs).

**The registry is not part of the funds path, and the wallet does not trust it.**
A record holds the factory, singleton, initializer and salt the Safe was created
with. The wallet re-derives the Safe's address from them and refuses a record
that does not make that address. On a chain where the multisig is not deployed
yet, the wallet treats its owners as known only if every contract the genesis
depends on (factory, singleton, fallback handler, setup hook) holds Safe's
canonical code on that chain. The registry has no owner and no upgrade path.
Anyone may write a record. A record's parameters never change, and only the Safe
itself can rename its record. The worst a faulty registry can do is make a
multisig hard to find or add on a new device. It cannot change who controls one.

## Known issues we are watching

### Changes of authorization within one bundle (Safe4337Module, Certora M-01)

The EntryPoint validates every operation in a bundle before executing any of
them. So if one operation removes an owner, an operation signed by that owner and
placed later in the same bundle still passes validation and runs. Safe
acknowledged this and did not change v0.3.0.

Vela's apps never build owner changes for a Vela account, and a dApp that asks for
one is refused outright, so Vela itself never triggers this. A multisig's member
changes do not go through the 4337 module: they run as `execTransaction`, and a
multisig Vela creates has no module, so this finding does not apply to them. It
still matters for anyone removing a compromised key through other Safe tooling:
they could not rely on that key being cut off within the same bundle.

### Interception of a signed operation (EntryPoint before v0.9)

In February 2026, researchers
[disclosed](https://erc4337.substack.com/p/improving-useroperation-execution) a
griefing and censorship vector affecting every EntryPoint before v0.9, including
v0.7. Someone who obtains a signed operation before it is mined can execute it
inside a call they control and force the inner execution to revert: the operation
fails and has to be signed again. (With Vela's in-band fee the fee transfer reverts
with it, so the relay rather than you absorbs the gas.) It affects operations that call reentrancy-protected
contracts or can be made to revert by temporary state; simple transfers are not
affected. Used against withdrawal flows repeatedly, it could keep funds
unavailable for a while. It cannot forge a signature or redirect funds.

Vela's relay submits operations directly rather than through a shared mempool, but
a pending `handleOps` transaction is still visible in the public mempool, so this
narrows the exposure rather than removing it. The fix exists only in EntryPoint
v0.9 (November 2025); v0.7 cannot be patched. Migrating depends on Safe's 4337
module supporting v0.9, and this page will say when it happens.

### Gaps in Vela's own defences

Not contract findings, but places where the wallet protects you less than you
might assume. Each is tracked for a fix, except where it says it is a deliberate
trade-off:

- **An unlimited approval goes out if you keep it** — a deliberate trade-off,
  because a capped approval breaks Permit2 and batched swaps. An "unlimited"
  approval (2^128 or more; 2^200, or 2^152 for Permit2, in v0.9.8 and earlier) is
  shown in red and sent as the dApp asked unless you cap it. Signed permits can't
  be capped anywhere.
- **The Trusted Signer is opt-in, and for a getvela.app wallet it is not an
  authority boundary.** The independent signing page is used only on a device
  where you chose it. Every app refuses a page whose bytes don't match the
  versions it ships with. But the apps can ask a getvela.app passkey directly,
  so a fully compromised app does not need the page; only a wallet made on a
  self-hosted page has the page as its one way to sign.
- **The website loads a third-party analytics script** on the same domain as the
  passkeys. The site forbids its pages from using passkeys (a Permissions-Policy
  header), and keeps the script off the page that holds a key.

## What is not audited

- **Vela's own contracts.** The
  [public-key registry](https://github.com/mondaylabsltd/p256-index/tree/main/contracts)
  at `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` (Gnosis; the same address on
  Ethereum and Base), the original registry deployment at
  `0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf` (its address is part of every
  registration's signature domain), and the earlier index they
  replaced (`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`, read-only history). They
  are unaudited. They hold no funds, have no owner and cannot be upgraded; they are
  a discovery layer, not an authorization layer. Spending power comes only from
  the keys configured in your Safe. The worst realistic failure is that a wallet
  becomes harder to find on a new device, not that money moves.
- **The Safe Genesis Registry**, Vela's own contract for multisig records:
  [source](https://github.com/mondaylabsltd/safe-genesis-registry), v4 at
  `0xF412388bF908E510066d3Be5c704bd9A327f945d` on Gnosis and Ethereum (the same
  address on every chain). No third party has audited it. Its own repository
  records an internal adversarial review (October 2026), which is not an audit.
  Like the public-key registry, it holds no funds, has no owner and cannot be
  upgraded. The multisig service and mailbox that serve its records are Vela
  backend services, covered by "Vela itself" below.
- **Multicall3.** Its README
  [says](https://github.com/mds1/multicall3) "This contract is unaudited." Vela uses
  it only for batched reads — balances, token details, price quotes — never with
  approvals or funds.
- **The deterministic deployers** (Arachnid's CREATE2 proxy and Safe's singleton
  factory) — ecosystem-standard and stateless, without formal audits. Vela's
  network check fails closed if they are missing; it checks that code exists at
  the address, not that it matches byte for byte.
- **Tempo.** One of the 20 networks Vela shows by default, with no native coin;
  Vela pays gas there in the pathUSD stablecoin. As of September 2026, Tempo's
  [security policy](https://github.com/tempoxyz/.github/blob/main/SECURITY.md)
  says the protocol is still undergoing audit and has no active bug bounty. Funds
  held on Tempo, and gas paid there, carry that chain-level risk; treat it as the
  newest and least-proven chain on the list.
- **Vela itself.** The apps, the backend services and the contracts above have not
  had a third-party audit, and none is scheduled. That is the largest caveat on
  this page. The details are in [Vela is in alpha](/blog/vela-is-in-alpha). Start
  with small amounts, and read the code.

## Past security issues

Vulnerabilities that affected, or could have affected, Vela users. Each advisory
gives the affected versions, the fix commit, a timeline and workarounds. Full
text: [docs/security/advisories](https://github.com/mondaylabsltd/vela-wallet/tree/main/docs/security/advisories).
Once published, they also appear under the repository's
[security advisories](https://github.com/mondaylabsltd/vela-wallet/security/advisories).

| ID            | Issue                                                                                                                                                         | Severity | Status                                                                                                                                                                                                                      |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| VELA-2026-001 | A typed-data request could preview one EIP-712 message and sign another. Reported externally on 29 September 2026.                                            | High     | Fixed in v0.9.6 (3 October 2026)                                                                                                                                                                                            |
| VELA-2026-002 | A site connected to a Vela account could make it approve or execute for a Safe that account owns (a "parent" Safe). Reported externally on 29 September 2026. | High     | Fixed before release: the multisig feature never shipped with it. Released versions up to v0.9.8 are exposed only if your Vela account is an owner of a Safe created elsewhere; the fix reaches them with the next release. |
| VELA-2026-003 | getvela.app's app-association files let development and demo Android and iOS apps request getvela.app passkeys. Found by an internal review.                  | Medium   | Fixed and deployed on 7 November 2026; no known exploitation                                                                                                                                                                |

Internal incidents that put no user's keys or funds at risk are recorded in
[docs/security/incidents.md](https://github.com/mondaylabsltd/vela-wallet/blob/main/docs/security/incidents.md).

## Check it yourself

Every address below is a canonical public deployment. Verify them against
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
and the [EntryPoint release](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| Contract                                   | Address                                      |
| ------------------------------------------ | -------------------------------------------- |
| SafeL2 singleton v1.4.1                    | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1                    | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| MultiSend v1.4.1                           | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| CompatibilityFallbackHandler v1.4.1 ¹      | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| SafeModuleSetup v0.3.0                     | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0                      | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1            | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| SafeWebAuthnSignerFactory v0.2.1           | `0x1d31F259eE307358a26dFb23EB365939E8641195` |
| SafeWebAuthnSigner singleton v0.2.1        | `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` |
| EntryPoint v0.7                            | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3                                 | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Public-key registry (Vela, unaudited)      | `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` |
| MultiSendCallOnly v1.4.1 (multisig)        | `0x9641d764fc13c8B624c04430C7356C1C7C8102e2` |
| SafeToL2Setup v1.4.1 (imported multisigs)  | `0xBD89A1CE4DDe368FFAB0eC35506eEcE0b1fFdc54` |
| Safe Genesis Registry v4 (Vela, unaudited) | `0xF412388bF908E510066d3Be5c704bd9A327f945d` |

¹ Checked when a network is added. A Vela account uses the 4337 module as its
fallback handler instead; a multisig uses this handler.
