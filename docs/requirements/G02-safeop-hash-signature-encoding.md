# G02 · SafeOp Hashing & WebAuthn Signature Encoding

| | |
|---|---|
| **Epic** | G — Transaction Engine (ERC-4337) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | G01, B01 |
| **Related** | B05, B06, K05 |

## 1. Summary

The hash the user signs is a **SafeOp EIP-712 typed-data hash bound to the Safe4337Module** — not a raw
userOpHash. `calculateSafeOpHash` hashes a `SafeOp(...)` struct with domain `{chainId, verifyingContract
= Safe4337Module}` (`validAfter`/`validUntil` = 0, no expiry). The WebAuthn assertion is then encoded
into a Safe **contract signature** with mandatory **low-s normalization** for the RIP-7212 precompile.

## 2. Background & context

The Safe4337Module validates a SafeOp hash, so Vela must compute exactly that. The on-chain P-256
precompile (RIP-7212) rejects high-s signatures, so `s` must be normalized to the low half. The
verifier templates `clientDataJSON`, so Vela extracts only the trailing fields rather than
reconstructing them (which is why reordering providers are rejected, B05).

## 3. Users & stories

- As a **user**, I want my biometric signature to be accepted on-chain, so that my transaction executes.
- As a **security reviewer**, I want the signed payload bound to the module + chain, so that a signature can't be replayed cross-context.

## 4. Functional requirements

- **FR-1** — `calculateSafeOpHash` hashes a `SafeOp(...)` struct with domain `{chainId, verifyingContract = Safe4337Module}`; `validAfter`/`validUntil` = 0.
- **FR-2** — `derSignatureToRaw` parses DER, left-pads r/s to 32 bytes, and **normalizes `s` to ≤ n/2** (RIP-7212 requirement).
- **FR-3** — Encode the on-chain signature as a Safe **contract signature**: `validAfter(6) ++ validUntil(6) ++ r(32) ++ s(32) ++ v=0x00(1) ++ dataLen(32) ++ abi.encode(authenticatorData, clientDataFields, sigR, sigS)`.
- **FR-4** — Extract only the trailing `clientDataFields` after the challenge (not reconstruct the JSON).
- **FR-5** — Each signature requires a fresh biometric (B02).

## 5. Non-functional requirements

- **NFR-1** — Cross-platform-identical encoding (locked by golden vectors, B06).
- **NFR-2** — Reject incompatible providers before signing (B05).

## 6. UX / flow notes

No direct UI beyond the biometric prompt; the signing sheet (I01) shows the decoded intent, not this encoding.

## 7. Acceptance criteria

- [ ] **AC-1** — The computed SafeOp hash matches the module's expectation for a given op.
- [ ] **AC-2** — A high-s WebAuthn signature is normalized to low-s before encoding.
- [ ] **AC-3** — The encoded contract signature validates on-chain via RIP-7212.

## 8. Out of scope / non-goals

- UserOp assembly — **G01**; EIP-1271 message signing — **K05** (uses a related but distinct SafeMessage wrap).

## 9. Dependencies, risks & open questions

- **Risk:** provider-specific clientDataJSON layouts — guarded by B05.
- **Open question:** None.

## 10. Source anchors

- `src/services/safe-transaction.ts:869-916` (SafeOp hash), `:982-1123` (signature encoding, clientDataFields).
- `src/services/attestation-parser.ts:51-122` (`derSignatureToRaw`, low-s).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 40, 41, 42, 44.
