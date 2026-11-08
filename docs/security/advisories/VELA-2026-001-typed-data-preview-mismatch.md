# VELA-2026-001: a typed-data request could preview one EIP-712 message and sign another

| GHSA form field | Value |
| --- | --- |
| Title | Typed-data signing request could preview one EIP-712 message and sign another |
| Ecosystem / package | Other / `mondaylabsltd/vela-wallet` (web wallet, Chrome extension, desktop, Trusted Signer page; iOS and Android source) |
| Affected versions | `<= 0.9.5` (verified at v0.9.3, v0.9.4, v0.9.5; see "Affected versions") |
| Patched versions | `0.9.6` |
| Severity | High (Vela's assessment, explained below); CVSS 3.1 base 5.3 |
| CVSS vector | `CVSS:3.1/AV:N/AC:H/PR:N/UI:R/S:U/C:N/I:H/A:N` |
| Weakness | CWE-451: User Interface Misrepresentation of Critical Information |
| Credit | An external researcher (reported 2026-09-29) |
| Status | Fixed. Advisory not yet published. |

## Summary

`eth_signTypedData*` requests carry the EIP-712 document inside a `params` array. Up to v0.9.5,
the wallet's components did not agree on which element of that array was "the" document. The
confirmation sheet decoded one element, while the passkey signed the EIP-712 digest of another.
The approval guard and the `SafeTx` refusal each read an element of their own as well.

A site the user had already connected could send a request with two typed-data documents: a
harmless one where the preview looked, and a harmful one where the signature looked. The user
confirmed what they saw, and the passkey signed something else. The signature came out as a
valid EIP-1271 signature of the user's Safe over the hidden document.

## Details

There were two request shapes.

**Shape A: `eth_signTypedData_v4` with `[benignTypedData, maliciousTypedData]`, with no account
address.** This affected the web wallet and the Chrome extension.

- `extension/lib/protocol.js` `isWellFormedRequest` accepted any JSON array under the size limit.
- The sheet (`signing/core/sheet.svelte.ts`) previewed the first string or object,
  `params.find(...)`, which is element 0.
- The submit path (`services/dapp-submit.ts` `pickTypedDataParam`) hashed `params[1] ?? params[0]`,
  which is element 1, and asked the passkey to sign that digest wrapped in the Safe's
  `SafeMessage`.
- The core's approval guard read element 1, but it did not recognise Permit2 SignatureTransfer
  (`PermitTransferFrom` and `PermitBatchTransferFrom`). The core's `SafeTx` refusal read the first
  element that parsed, so a `SafeTx` placed second also passed.

The reporter's verifier ran the exact v0.9.5 JS and WASM against synthetic inputs. The preview
showed **"Approve, Amount to spend 0 TST"** for a zero-amount `PermitTransferFrom`. The digest
selected for signing belonged to a non-zero `PermitBatchTransferFrom`. The approval guard raised
nothing, and confirmation was allowed.

**Shape B: legacy `eth_signTypedData` / `eth_signTypedData_v1` with
`[maliciousTypedData, benignTypedData]`.** An internal audit found this shape while fixing shape
A ([spec 085 audit](../../../specs/085-eip712-wysiwys/audit.md)). It affected the desktop app,
the Trusted Signer page, and the iOS and Android source. Each of these previewed `params[1]`,
while the core's `sign_message::original_hash` signed `params[0]` for the legacy method names.
The audit measured the same mismatch on all four, with an identical signed digest.

The raw `params` were available under "Technical details", which is collapsed by default. A user
who expanded it could have seen both documents. We do not count that as a mitigation.

## Impact

A connected site could obtain the user's Safe signature over a typed-data document they had not
been shown. The concrete consequence the reporter demonstrated (in an isolated EVM, with dummy
tokens and unmodified Permit2 and Safe v1.4.1 contracts) was a Permit2 `PermitBatchTransferFrom`
signature. One redemption of it moved the Safe's balances of every token that **had an existing
ERC-20 allowance to Permit2**, up to the signed amounts.

The real-world conditions are:

- the user had connected the malicious site, and confirmed the request with their passkey;
- for the Permit2 route, the Safe had approved the tokens to Permit2 by the time of redemption
  (a long-deadline signature can wait for an approval), the signature's nonce was unused, and its
  deadline had not passed;
- nothing in the report shows a direct transfer from a Safe that never approved Permit2.

A second consequence is covered separately in
[VELA-2026-002](VELA-2026-002-parent-safe-authority.md): a hidden `SafeTx` for another Safe that
the user's Vela account owns.

The reporter did not obtain a real passkey signature from a live wallet, and neither did we. We
know of no exploitation. We cannot rule it out, because a signature looks the same whichever
document the user was shown.

**Why High, while the CVSS base score is 5.3.** CVSS rates this Medium because it needs a
connected site, a user confirmation, and existing Permit2 allowances (AC:H). We rate it High
because the defect is in the confirmation itself. The one screen meant to tell the user what
they sign showed something else. For a user with Permit2 allowances, the loss was bounded only by
those allowances and balances.

## Affected versions

| Surface | Affected | How we know |
| --- | --- | --- |
| Web wallet (`wallet.getvela.app`) and Chrome extension, shape A | v0.9.3, v0.9.4, v0.9.5 | The same selectors are in each tagged source. The preview selector arrived in `08596b6c7` (2026-09-04), and every public tag after it has it. Builds before that commit had a different preview selector, which we have not verified. |
| Desktop app (published on GitHub Releases), shape B | v0.9.3, v0.9.4, v0.9.5 | `signing_host.rs` `typed_data_of` reads `params[1]`; the core signs `params[0]` for the legacy names (tagged source) |
| Trusted Signer page (`sign.getvela.app`), shape B | builds before `0ba8ee8c…` | spec 085 audit; only users who chose the Trusted Signer used it |
| iOS and Android apps, shape B | source up to v0.9.5 | spec 085 audit. These apps were not yet distributed through the stores. |
| Shared core: no shape check, and `SafeTx` refused only in the first parseable element | up to v0.9.5 | spec 085 audit |

The earliest affected release was not established. v0.9.3 is the oldest public tag.

## Fix

Pull request #339, merge commit **`67e2d193d87061955014465798a264d5c821c798`** (2026-10-01),
released in **v0.9.6** (`ae321ed`, published 2026-10-03). The main commits:

- `4092143a8`, core: a typed-data request is parsed once, strictly. The method must be one of
  `eth_signTypedData`, `_v1`, `_v3`, `_v4`; the request must hold exactly one account and one
  document, in that method's order, or it is refused with `-32602` before any sheet opens. A
  document for an account other than the one granted is refused with `4100`. The preview, the
  approval guard, the `SafeTx` refusal and the passkey digest all use that one canonical document.
  A `SafeTx` anywhere in the request is refused. Permit2 SignatureTransfer single and batch are
  flagged.
- `b218026ae`, clients: every sheet decodes the core's one document. The extension refuses the
  bad shapes at its own boundary first.
- `cae65212d`, Trusted Signer page: one reader for the preview and the digest. It shipped as page
  build `0ba8ee8c…`, deployed 2026-10-01.

Commit `7e279310c` (pull request #337) was a first fix covering only web and extension v4. It
was closed unmerged in favour of #339 and never shipped.

Regression tests (named in [spec 085 results](../../../specs/085-eip712-wysiwys/results.md)):
`the_malicious_shapes_are_refused_before_the_sheet`, `a_safe_tx_anywhere_is_refused`,
`permit2_signature_transfer_is_flagged`, `a_document_for_another_account_is_refused_4100`, and
per-platform tests that the sheet's document hashes to the signed digest (web
`typed-data-request.test.ts`, desktop, iOS, and the signer page's `typed-shape-test.mjs`).

## Workarounds

- Update. The web wallet has served v0.9.6 or later since 2026-10-03. Install the newest desktop
  package and **re-install the Chrome extension** from the newest release. The extension is
  installed by hand and does not update itself.
- If you signed typed data from a site you do not fully trust while on an affected version, check
  the ERC-20 approvals your Safe has given to Permit2 (`0x000000000022D473030F116dDEE9F6B43aC78BA3`)
  and revoke the ones you do not need. A SignatureTransfer permit can only pull a token the Safe
  has approved to Permit2. It also stops working at its deadline, or once its nonce is used or
  invalidated (Permit2 `invalidateUnorderedNonces`, sent from the Safe).

## Timeline

| Date (UTC+8 unless noted) | Event |
| --- | --- |
| 2026-09-04 | `08596b6c7` introduces the first-element preview selector in the web sheet |
| 2026-09-24 | v0.9.5 tagged (`ce55fb6`), the version the report analyses |
| 2026-09-29 | External report: "Vela Wallet v0.9.5 previews one typed message and prepares to sign another", with a verifier and an EVM proof of concept |
| 2026-10-01 | Exact receipt time not recorded. PR #337 opened (web/extension v4 only). Internal audit of every surface finds shape B (spec 085). PR #339 merged (`67e2d193d`), #337 closed. Signer page `0ba8ee8c…` deployed. |
| 2026-10-03 | v0.9.6 released (05:07 UTC), and the web wallet deploys from it |
| 2026-11-08 | Advisory drafted (no public advisory existed before this) |
| *pending* | Advisory published on GitHub |

## References

- [specs/085-eip712-wysiwys/](../../../specs/085-eip712-wysiwys/): the audit, spec and results
- EIP-712; EIP-1271; Uniswap Permit2 `SignatureTransfer`
