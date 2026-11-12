# VELA-2026-002: a site connected to a Vela account could act for a Safe that account owns ("parent Safe")

| GHSA form field | Value |
| --- | --- |
| Title | A connected site could make a Vela account approve or execute for a Safe it owns |
| Ecosystem / package | Other / `mondaylabsltd/vela-wallet` |
| Affected versions | `<= 0.9.8` when the Vela account is an owner of another Safe (see "Affected versions"). The multisig feature never shipped with it. |
| Patched versions | The first release that contains merge `64da454d3` (not released yet) |
| Severity | High (Vela's assessment); CVSS 3.1 base 6.1 |
| CVSS vector | `CVSS:3.1/AV:N/AC:H/PR:N/UI:R/S:C/C:N/I:H/A:N` |
| Weakness | CWE-441: Unintended Proxy or Intermediary ("Confused Deputy") |
| Credit | An external researcher (reported 2026-09-29, in the appendix to the VELA-2026-001 report) |
| Status | **Fixed before release; Vela multisig never shipped with it.** Released versions remain exposed in the uncommon setup described below until the next release. The fix does not yet cover a target address written in a non-canonical spelling ("Known gap in the fix", below; fix in progress). **Do not publish this advisory before a release with the complete fix ships.** |

## Summary

A Vela account is a Safe. It can also be an **owner** of another Safe, its "parent". Since the
multisig feature, every Vela multisig is such a parent. Before that, the only way was for a user
to add their Vela address as an owner of a Safe created elsewhere, for example in Safe{Wallet}.

Vela's self-call guard refuses the Safe administration calls (owner, threshold, module, guard,
fallback handler) only when their target is **the signing account itself**. A site connected to
the Vela account could therefore ask the account to exercise its owner authority over a parent
Safe, in any of these forms:

- **(a)** a transaction `parent.execTransaction(…, signatures = {r = account, v = 1})`. This is
  Safe's "approved by `msg.sender`" signature. For a parent whose threshold this account meets
  alone (1-of-N), it executes anything at once: `swapOwner`, `addOwnerWithThreshold`, or a
  transfer of all funds;
- **(b)** a transaction `parent.approveHash(h)`, which gives one counting approval for any
  `SafeTx` hash. The selector was not checked anywhere;
- **(c)** EIP-712 typed data with `primaryType` `SafeTx` or `SafeMessage` under the parent's
  domain. The wallet wraps dApp typed data in the account's own `SafeMessage`, and the result is
  a nested EIP-1271 signature that the parent counts as this owner's.

## Details and history

**The reported form (v0.9.5).** The report's appendix combined this with VELA-2026-001. It put a
zero-amount Permit2 message at `params[0]` and a `SafeTx` for the parent at `params[1]`. v0.9.5
refused a `SafeTx` only when it was the first element that parsed, so the hidden `SafeTx` passed.
In an isolated EVM, with a Safe v1.5.0 parent whose **sole owner** was a Safe v1.4.1 child
running Safe4337Module v0.3.0, the child's EIP-1271 answer was accepted. The parent executed
`swapOwner` to an attacker, and the attacker then moved all of the parent's (synthetic) 2 ETH.
The control cases held: a corrupted signature, a parent owned by someone else, and a 2-of-2
parent without the second signature were all rejected. The VM used a deterministic EOA in place
of the child's WebAuthn owner. The reporter notes that a Safe v1.4.1 parent did not accept this
digest and signature shape in their earlier test.

**v0.9.6 closed the reported form.** That release refuses a typed-data request with more than one
document. It also refuses a `SafeTx` anywhere in a request, under any domain
([VELA-2026-001](VELA-2026-001-typed-data-preview-mismatch.md), merge `67e2d193d`).

**What v0.9.6 to v0.9.8 still allow.** These versions do not refuse forms (a), (b), or (c) with
`SafeMessage`, as long as the target is a Safe other than the signing account. We checked this
at v0.9.8: `self_call_guard.rs` compares each blocked selector's target with the signing account
only, `approveHash` is not listed, and there is no parent-Safe check. Such a request reaches the
normal confirmation sheet as a contract call or a typed-data signature. The user must confirm it
with their passkey. How clearly the sheet showed the inner action depended on the decoder; we
have not measured it for each version.

**The internal security review of 2026-11-07** found the same gap on `feat/multisig-account`.
There, every multisig the user creates or joins is a parent Safe, so the setup would have become
normal. The fix merged the same day, before any multisig release.

## Impact

For a user whose Vela account is an owner of another Safe, a connected malicious site could get
one confirmed request to:

- with (a) on a parent this account controls alone: take the parent over or empty it in one
  transaction;
- with (b) or (c): add this owner's approval toward any transaction or message for the parent.
  On a multi-owner parent, the other owners' approvals are still needed.

Vela releases up to v0.9.8 have no multisig feature. In released versions the exposure therefore
needs a user who has made their Vela account an owner of a Safe outside Vela, connected a
malicious site, and confirmed its request. We do not know how many users, if any, have this
setup. We know of no exploitation.

**Why High, while the CVSS base score is 6.1.** The impact on an affected parent is complete
control, and the parent is a separate account (S:C). AC:H reflects the uncommon ownership
setup.

## Affected versions

| Version | Status |
| --- | --- |
| up to v0.9.5 | Affected: the reported two-document form, plus forms (a), (b), (c) |
| v0.9.6 to v0.9.8 (current release) | Affected: forms (a), (b), and (c) with `SafeMessage`, but only for a Vela account that owns another Safe |
| `feat/multisig-account` before `64da454d3` | Affected (unreleased) |
| `feat/multisig-account` from `64da454d3` | Fixed for targets written as `0x` and 40 hex digits; other spellings open (see "Known gap in the fix") |

## Fix

Merge **`64da454d34415f2c24208c191f1e2d4f45869727`** (2026-11-07, "Merge p0-parent-safe-guard"),
documented in [`docs/multisig-machines.md` §31](../../multisig-machines.md#31-parent-safe-guard).
The rule lives in one core module,
[`parent_safe_guard.rs`](../../../rust/crates/vela-core/src/app/parent_safe_guard.rs), and every
surface applies it.

A site's request is refused, with no "anyway" path, when it would act for **a Safe this account
owns**. That covers `execTransaction` (any inner call), `approveHash`, any call on the self-call
list, any of those nested in `multiSend` or `execTransaction` (to depth 4), and typed data with
`primaryType` `SafeTx` or `SafeMessage` whose `verifyingContract` is that Safe. Every document in
the request is read.

"A Safe this account owns" is either:

- a multisig on the device's list, which is refused with no read; or
- any other target of those shapes for which an on-chain `isOwner(account)` answers `true`.

The sheet stays shut until that read answers. If the read cannot be made, the request is refused
as "couldn't check". It is never allowed.

The submit chokepoint enforces the rule again on the final params. The Trusted Signer page
applies it independently (page build `2c649b76…`). The appendix's proof of concept is replayed
as a vector on every shell (`ParentSafeGuardTests`, `ParentSafeGuardVectorTest`, desktop
`multisig/scenarios.rs`, signer `samples/takeover-test.mjs` 67/67).

The fix ships with the first release made from `feat/multisig-account`.

**Known gap in the fix (final review, 2026-11-12; open).** The guard looks at a call's `to` and
a typed-data `verifyingContract` only when it is written as `0x` followed by exactly 40 hex
digits (`parent_safe_guard.rs`, `is_address`); a target written any other way is skipped, not
refused. Other parts of the core read addresses more loosely: `primitives::parse_address`
accepts one without `0x`, and the EIP-712 encoder left-pads a short one to 20 bytes (`eip712.rs`,
`pad_scalar`). So a request that names a parent Safe in such a spelling is not refused by this
guard, and the fix does not yet cover every spelling of every form above. Canonicalising each
address before the check is in progress (to be recorded as `docs/multisig-machines.md` §41);
[SECURITY-MODEL G24](../../SECURITY-MODEL.md#13-known-gaps-and-accepted-risks) tracks it. This
advisory is not to be published until that fix is in the release that carries `64da454d3`.

## Workarounds

Until a release contains the fix:

- If your Vela account is an owner of a Safe created outside Vela, do not connect sites you do not
  fully trust to that Vela account. Use a separate Vela account for dApps.
- Refuse any request that calls `execTransaction` or `approveHash` on a Safe, and any typed data
  whose `primaryType` is `SafeMessage` or `SafeTx`. No ordinary dApp needs a member's account to
  sign these for a parent Safe.
- For a parent controlled by one owner, consider raising its threshold so that one compromised
  approval is not enough.

## Timeline

| Date (UTC+8) | Event |
| --- | --- |
| 2026-09-29 | Reported in the appendix to the VELA-2026-001 report, with an EVM proof of concept |
| 2026-10-01 | v0.9.6 fix (`67e2d193d`) refuses the reported two-document form and any `SafeTx` |
| 2026-10-03 | v0.9.6 released |
| 2026-11-07 | The internal security review finds forms (a), (b) and (c) still open, now on every multisig. Fixed the same day (`64da454d3`). |
| 2026-11-08 | Advisory drafted |
| *pending* | Release containing `64da454d3`; then the advisory is published |

## References

- [`docs/multisig-machines.md` §31](../../multisig-machines.md#31-parent-safe-guard)
- Safe `checkSignatures` (`v = 1`: approved hash or `msg.sender`; `v = 0`: EIP-1271 contract
  signature)
