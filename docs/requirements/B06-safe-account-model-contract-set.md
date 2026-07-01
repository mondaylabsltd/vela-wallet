# B06 · Safe v1.4.1 Account Model & Canonical Contract Set

| | |
|---|---|
| **Epic** | B — Identity: Passkeys & Account Model |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | B01 |
| **Related** | B07, G01, G02, F02, K04 |

## 1. Summary

Every Vela wallet is an **unmodified Safe v1.4.1 smart account**, operated via **ERC-4337 account
abstraction (EntryPoint v0.7)**, with the **Safe 4337 Module** and a **WebAuthn signer
(SafeWebAuthnSharedSigner)** as the sole owner (threshold 1). One canonical contract set is used
**identically on every chain**, and on-chain P-256 verification runs through the **RIP-7212
precompile**. This is the concrete, standards-based backbone under the seedless UX.

## 2. Background & context

Rolling a custom smart-account contract would demand its own audit and invite bespoke bugs. Vela
instead composes audited, widely-used building blocks (Safe, EntryPoint, Safe4337Module) and adds only
the WebAuthn signer configuration — keeping "audited Safe vs proprietary account" as a battle-card win
(clue 19).

## 3. Users & stories

- As a **security evaluator**, I want the account to be standard audited Safe contracts, so that I'm not trusting a bespoke wallet contract.
- As a **developer**, I want the same addresses/contracts on every chain, so that integration is uniform.

## 4. Functional requirements

- **FR-1** — Account = Safe v1.4.1 singleton behind a `SafeProxy`, with the Safe4337Module enabled and `SafeWebAuthnSharedSigner` as sole owner (threshold 1).
- **FR-2** — Canonical addresses (same on all chains): SafeProxyFactory `0x4e1DCf7A…ec67`, Safe Singleton `0x29fcB43b…C762`, FallbackHandler `0xfd0732Dc…Ec99`, EntryPoint v0.7 `0x0000000071727De2…da032`, Safe4337Module `0x75cf1146…c226`, SafeModuleSetup `0x2dd68b00…5b47`, WebAuthn shared signer `0x94a4F6af…55c2`, MultiSend `0x38869bf6…B526`.
- **FR-3** — On-chain P-256 verification uses `verifiers = 0x100` (RIP-7212 precompile).
- **FR-4** — Hand-rolled Keccak-256 (0x01 padding) + minimal CBOR parser extract the COSE key with **no native crypto dependency**, identical on iOS/Android/web, locked by golden vectors.
- **FR-5** — Adding a custom network requires this full contract suite **plus** the RIP-7212 precompile to exist (F02).

## 5. Non-functional requirements

- **NFR-1** — Deterministic, cross-platform crypto parity (golden test vectors are the gate).
- **NFR-2** — No contract in the set is modified/forked — "unmodified Safe."

## 6. UX / flow notes

No direct UI. Surfaces indirectly as counterfactual addresses (B07) and as the "audited Safe" trust claim (A02).

## 7. Acceptance criteria

- [ ] **AC-1** — A deployed Vela account matches the expected Safe v1.4.1 + 4337 module + WebAuthn-owner configuration on-chain.
- [ ] **AC-2** — The contract addresses are identical across all supported chains.
- [ ] **AC-3** — Keccak/CBOR outputs match golden vectors on all three platforms.

## 8. Out of scope / non-goals

- Address derivation — **B07**; UserOp building/deploy — **G01**; signature encoding — **G02**.

## 9. Dependencies, risks & open questions

- **Risk:** any change to the contract set forks addresses (see B07) — auto-High-risk.
- **Open question:** a signing path for chains **without** the P-256 precompile is roadmap 🧭 (F02).

## 10. Source anchors

- `src/services/safe-address.ts:21-28` (contract set), `:120` (RIP-7212 verifiers).
- `src/services/eth-crypto.ts:110-146`, `src/services/attestation-parser.ts:23-45` — keccak + CBOR.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 4, 36, 45, 46.
