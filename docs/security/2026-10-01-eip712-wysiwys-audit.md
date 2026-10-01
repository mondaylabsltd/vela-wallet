# EIP-712 typed data — "what you see is not what you sign" audit

**Commit audited:** `main` @ `4f2f8b633` (2026-10-01). Read-only; no passkey, wallet or chain was
touched. Every finding below was **executed**, not read off a comment: the probe tests named in §5
ran against this code (iOS, Android and desktop probes ran in the 082 tree, whose files for every
reader below are byte-identical to `main` — checked with `git diff`).

Two malicious shapes were the focus:

- **A** — `eth_signTypedData_v4` with `[benignTypedData, maliciousTypedData]` (no account).
- **B** — legacy `eth_signTypedData` / `eth_signTypedData_v1` with `[maliciousTypedData, benignTypedData]`.

## 1. Verdicts

| Surface | Verdict | What happens |
|---|---|---|
| Shared Rust core | **Confirmed** | No reader validates the shape. The signed digest, the SafeTx guard, the approval guard, the chain pick and the address check each pick their own element — shape A slips a `SafeTx` past the SafeTx refusal; shape B (and a legitimate legacy Permit) gets no approval-guard warning; Permit2 `PermitTransferFrom` / `PermitBatchTransferFrom` are never detected |
| Chrome extension / web wallet | **Confirmed** | Shape A: the extension's first boundary admits it, the sheet previews the **benign** document, the passkey signs the **malicious** one. Shape B: preview and signature agree (both `params[0]`), but the approval guard reads `params[1]` — no warning |
| iOS dApp browser | **Confirmed** | Shape B: the sheet previews `params[1]` (benign), the passkey signs `params[0]` (malicious). Shape A: preview and signature agree; the SafeTx refusal is bypassed (core) |
| Android dApp browser | **Confirmed** | Same as iOS |
| Desktop dApp browser | **Confirmed** | Same as iOS; in addition `_v1` / `_v3` get no clear-signing preview at all |
| Trusted Signer page | **Confirmed** | Shape B: the page previews `params[1]`, its own digest is `params[0]`. Its SafeTx refusal scans every element (correct). It labels `PermitTransferFrom` as an allowance and does not know `PermitBatchTransferFrom` |

Measured digest (shape B, Permit vs Mail, identical on iOS, Android, desktop and the signer page):
the sheet decodes `Mail`, the signature covers the Permit, `0x943a0f4c53aacfbdca2a71b6006730a924d743d11e9bb9048634192afd55e2eb`.

## 2. Per-reader map (`main` @ `4f2f8b633`)

| # | Question | Core | Web / extension | iOS | Android | Desktop | Signer page |
|---|---|---|---|---|---|---|---|
| 1 | Entry validation of params | none: `sign_request.rs` arrival (≈1470–1590) never checks the typed-data shape; `dapp_rpc.rs:205 requested_address` returns `None` (not refused) when `params[0]` is not an address | `extension/lib/protocol.js isWellFormedRequest` — any array | core | core | core | none |
| 2 | Preview (clear signing) document | — | `signing/core/sheet.svelte.ts:102` `params.find(p => typeof p === 'string' \|\| typeof p === 'object')` — **first** string/object (for a legitimate v4 that is the **address**) | `SigningController.swift:787 typedDataOf` — `params[1]` for every method | `SigningController.kt:486 typedDataOf` — `params[1]` for every method | `signing_host.rs:1549 typed_data_of` — `params[1]`, wired only for `_v4` and unsuffixed (`:1482`); `_v1`/`_v3` none | `resolve.js:731 resolveTypedData` — `params[1]` for every method |
| 3a | Approval guard document | `approval_guard.rs:1614 detect_approval` — `params[1] ?? params[0]` for **every** typed method | core (wasm) | core | core | core | `resolve.js` (same element as its preview) |
| 3b | SafeTx guard document | `self_call_guard.rs:345 detect_safe_tx_typed_data` — **first** element that parses | core | core | core | core | `resolve.js:1233 isSafeTx` — any element (correct) |
| 4 | Signed digest | `sign_message.rs:25 original_hash` — legacy `params[0]`, else `params[1] ?? params[0]` | `services/dapp-submit.ts:199 pickTypedDataParam` — the same rule, in TypeScript | core (`signMessageHash`) | core (`signMessageHash`) | core (`message_hash`) | `digest.js:138 digestFor` — the same rule, in JS |
| 5 | Chain pick | `sign_request.rs:758 extract_request_chain_id`, `dapp_session.rs:1916` — `params[1] ?? params[0]` | core | core | core | core | — |
| 6 | What the dApp gets / what travels on | the signature only; the raw params — both documents — are kept in the pending request, the record, and the intent sent to the signer page | same | same | same | same | receives both documents |

Method routing: `method.contains("signTypedData")` (`sign_request.rs:701, 760, 818`,
`sign_message.rs:17, 43`, `approval_guard.rs:1631, 1972`, `self_call_guard.rs:300`,
`dapp_session.rs:1916`, `extension/lib/protocol.js:97`) — any spelling (`eth_signTypedData_v2`,
`x_signTypedData_y`) reaches a digest. A one-element v4 (`[typedData]`, no account) is signed.

`app-web/vela-wallet/src/lib/services/approval-guard.ts` (another `params[1] ?? params[0]`) is
imported by nothing but its own tests — dead code.

## 3. The two shapes, client by client

| Shape | Web / ext | iOS | Android | Desktop | Signer page |
|---|---|---|---|---|---|
| A: v4 `[benign, malicious]` | **preview ≠ signed**; approval guard sees the malicious one; SafeTx guard sees the benign one | preview = signed (malicious is shown); **SafeTx refusal bypassed** | same as iOS | same as iOS | preview = signed; SafeTx refused |
| B: legacy `[malicious, benign]` | preview = signed; **approval guard sees the benign one** | **preview ≠ signed**; guard sees benign | same as iOS | same as iOS | **preview ≠ signed** |
| Neither is refused before the sheet | ✗ | ✗ | ✗ | ✗ | ✗ |

## 4. Permit2

| | Core approval guard | Core clear signing | Signer page |
|---|---|---|---|
| `PermitSingle`, `PermitBatch` (AllowanceTransfer) | detected | descriptors | `PermitSingle` described |
| `PermitTransferFrom` (SignatureTransfer) | **not detected** | descriptor | described as an **allowance** ("valid until") — it is a one-shot transfer |
| `PermitBatchTransferFrom` | **not detected** | **none** | **unknown type** (caution only) |

## 5. Evidence

- `rust/crates/vela-core/tests/audit_typed_data_selectors.rs` — 7 tests, all passing on `main`: shape
  A signs the second document and is not refused; `[Mail, SafeTx]` is signed with no SafeTx refusal;
  shape B signs the first document while the approval guard finds nothing; a legitimate legacy
  Permit gets no warning; a one-element v4 is signed; unlisted suffixes are signed; Permit2
  SignatureTransfer is not detected.
- Desktop: `typed_data_of` + `message_hash` on shape B → shown `Mail`, signed `0x943a0f4c…`.
- iOS (`VelaWalletTests`, simulator): `SigningController.typedDataOf` + `SignExecutor.messageHash` → shown `Mail`, signed `0x943a0f4c…`.
- Android (JVM unit test): `SigningController.typedDataOf` + `SignExecutor.messageHash` → shown `Mail`, signed `0x943a0f4c…`.
- Web (vitest, real wasm core): `isWellFormedRequest` admits shape A; the sheet's pick hashes to the Mail, `pickTypedDataParam` to the Permit.
- Signer page (`node`, the page's own libraries): shape B → preview `typedUnknown · Mail`, digest `0x943a0f4c…`; a legitimate legacy request makes the preview throw.

## 6. Fix (implemented on `fix/typed-data-wysiwys`)

One reader, in the core, used by everyone:

1. `vela_core::typed_data_request::canonical(method, params)` — exact methods only (`eth_signTypedData`,
   `_v1` = `[typedData, address]`; `_v3`, `_v4` = `[address, typedData]`); exactly two params; a
   well-formed address in its slot; one EIP-712 document with `types`, `primaryType`, `domain`,
   `message` that hashes. Returns the account, the canonical document JSON and its digest.
2. The request is refused (`-32602`) at arrival when the shape is wrong or the account is not the
   one the site was granted, before any sheet opens; otherwise the pending params are replaced by
   the canonical two — so every later reader holds one document.
3. The signed digest, the SafeTx guard (plus a scan of every element), the approval guard and the
   chain pick all read the canonical document; Permit2 `PermitTransferFrom` /
   `PermitBatchTransferFrom` are detected.
4. Every preview (web sheet, iOS, Android, desktop) asks the core for the canonical document; the
   web submit path signs the core's digest; the extension's boundary refuses the wrong shapes first;
   the signer page reads the same element for its preview as for its digest and refuses the wrong
   shapes itself.

The probes above are kept as regression tests, inverted to the required behaviour.
