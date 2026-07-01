# K05 · EIP-1271 Message Signing (SafeMessage Double-Wrap)

| | |
|---|---|
| **Epic** | K — dApp Connect (WalletPair) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | G02 |
| **Related** | K06, K07, I01, B02 |

## 1. Summary

For dApp message signing (`personal_sign` / `eth_signTypedData`), Vela produces an **EIP-1271** signature
by **double-wrapping** the dApp's hash as a **SafeMessage**: `computeSafeMessageHash` = `keccak256(0x1901
++ domainSep{chainId, verifyingContract = the Safe itself} ++ keccak256(SAFE_MSG_TYPEHASH ++
keccak256(abi.encode(originalHash))))`, and — unlike the SafeOp path (G02) — **omits the 12-byte
validity prefix**.

## 2. Background & context

A smart-account (Safe) validates signatures via EIP-1271 against a SafeMessage bound to the Safe as
verifying contract. Getting the exact wrapping right (and the prefix difference vs SafeOp) is what makes
dApp logins/permits verify on-chain. The decoded message is shown via clear-signing (I01/K06).

## 3. Users & stories

- As a **user**, I want to sign dApp messages (login, permit) from my smart account, so that EIP-1271 dApps accept me.
- As a **user**, I want to see the message meaning before signing, so that I'm not blind-signing (I01).

## 4. Functional requirements

- **FR-1** — Compute the SafeMessage hash: `keccak256(0x1901 ++ domainSep{chainId, verifyingContract = the Safe} ++ keccak256(SAFE_MSG_TYPEHASH ++ keccak256(abi.encode(originalHash))))`.
- **FR-2** — **Omit** the 12-byte validity prefix used by the SafeOp path (G02).
- **FR-3** — Sign with a fresh biometric (B02); encode the WebAuthn contract signature (G02) for EIP-1271 validation.
- **FR-4** — Decode/display the message (typed data via I06) in the signing sheet (I01); SIWE messages get domain-binding treatment (K06).

## 5. Non-functional requirements

- **NFR-1** — Cross-platform-identical hashing (B06 golden vectors).
- **NFR-2** — The signature validates via the Safe's EIP-1271 `isValidSignature`.

## 6. UX / flow notes

Signing sheet (I01) renders the message; SIWE gets a "Sign in to {domain}" treatment (K06). Biometric required per signature.

## 7. Acceptance criteria

- [ ] **AC-1** — A `personal_sign` produces an EIP-1271-valid signature for the Safe.
- [ ] **AC-2** — Typed-data messages decode and render before signing.
- [ ] **AC-3** — The SafeMessage wrap omits the 12-byte prefix (distinct from SafeOp).

## 8. Out of scope / non-goals

- UserOp signing — **G02**; SIWE parsing — **K06**; batch calls — **K07**.

## 9. Dependencies, risks & open questions

- **Risk:** confusing SafeOp vs SafeMessage encodings — they differ by the prefix; keep tests for both.
- **Open question:** None.

## 10. Source anchors

- `src/services/safe-transaction.ts:941-1043` — `computeSafeMessageHash` (double-wrap, no prefix).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 43.
