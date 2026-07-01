# I06 · Dependency-Free ABI & EIP-712 Decoding (Nested Dynamic Types)

| | |
|---|---|
| **Epic** | I — Clear Signing & Decoding |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | I01 |
| **Related** | I02, I05, I07, J05, K06 |

## 1. Summary

A **dependency-free ABI decoder** parses Solidity signatures (tuples/arrays), recomputes selectors via
keccak256, and decodes with correct **relative-offset handling** and negative-index/byte-slice path
resolution (`path.-1`, `params.path[-20:]`) needed to read Uniswap V2/V3 swap routes. Dynamic arrays are
capped at 200 elements. The same layer decodes **EIP-712 typed data** for message signing (K06) and the
approval guard (J05).

## 2. Background & context

Shipping a heavy ABI library bloats the bundle and adds trust surface; a hand-rolled decoder keeps
control and cross-platform parity (B06 crypto). Correct offset/negative-index handling is essential to
extract real fields (e.g. the last hop of a swap path) rather than mislabeling them.

## 3. Users & stories

- As a **user**, I want complex calls (swaps, permits) decoded accurately, so that the signing sheet is trustworthy.
- As a **maintainer**, I want no heavy ABI dependency, so that the bundle and trust surface stay small.

## 4. Functional requirements

- **FR-1** — Parse Solidity signatures including nested tuples/arrays; recompute selectors via keccak256.
- **FR-2** — Decode with correct relative-offset handling; support negative-index/byte-slice paths (`path.-1`, `params.path[-20:]`).
- **FR-3** — **Cap dynamic arrays at 200 elements** (DoS/expansion guard).
- **FR-4** — Decode **EIP-712 typed data** (domain, types, message) for message signing (K06) and Permit2/approval detection (J05).
- **FR-5** — Return structured fields for rendering (I01) and risk scoring (I08).

## 5. Non-functional requirements

- **NFR-1** — Zero heavy ABI dependency; parity across platforms.
- **NFR-2** — Robust to malformed input (bounded parsing, no crashes).

## 6. UX / flow notes

No direct UI; produces the fields the signing sheet (I01) labels and the approval guard (J05) inspects.

## 7. Acceptance criteria

- [ ] **AC-1** — A Uniswap V3 swap's path (including the last hop via negative index) decodes correctly.
- [ ] **AC-2** — A dynamic array beyond 200 elements is capped, not expanded unbounded.
- [ ] **AC-3** — EIP-712 typed data decodes into domain/types/message for K06/J05.

## 8. Out of scope / non-goals

- Descriptor cascade — **I02**; risk math — **I08**; revert decoding — **J04**.

## 9. Dependencies, risks & open questions

- **Risk:** exotic encodings; fall back to best-effort/blind (I01) rather than mislabel.
- **Open question:** None.

## 10. Source anchors

- `src/services/abi-decode.ts`, `src/services/eip712.ts`, `src/services/clear-signing.ts:826`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 72.
