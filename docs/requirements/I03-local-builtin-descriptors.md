# I03 · Local Built-In Descriptors

| | |
|---|---|
| **Epic** | I — Clear Signing & Decoding |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | I02 |
| **Related** | I01, I08 |

## 1. Summary

Vela ships **built-in local descriptors** for high-value / common contracts and methods, sitting at the
**top** of the decoding cascade (I02). They provide the richest, offline-available rendering (intent +
labeled fields) without depending on a remote ERC-7730 fetch, so the most important interactions always
decode clearly.

## 2. Background & context

Remote descriptors can be missing or slow. Bundling descriptors for the interactions users hit most
(and for Vela's own flows) guarantees clarity for those, and provides a reliable baseline that the
richer remote layer can extend.

## 3. Users & stories

- As a **user**, I want common/important transactions to always decode clearly, so that I'm never blind-signing routine actions.
- As a **user offline-ish (flaky RPC)**, I want built-in descriptors to still render, so that decoding doesn't depend on a fetch.

## 4. Functional requirements

- **FR-1** — Maintain a local descriptor set (intent + field labels + risk hints) for common/important contracts and methods.
- **FR-2** — Place local descriptors **first** in the cascade (I02), ahead of remote ERC-7730.
- **FR-3** — Descriptors declare field roles (amount, recipient, spender, deadline) for consistent rendering (I01) and risk scoring (I08).
- **FR-4** — Work offline (no fetch dependency).

## 5. Non-functional requirements

- **NFR-1** — Descriptor changes are reviewed (touch a security surface, O02).
- **NFR-2** — Additive: a missing local descriptor simply defers to the next cascade layer.

## 6. UX / flow notes

No direct UI; determines how the signing sheet (I01) labels fields for covered contracts.

## 7. Acceptance criteria

- [ ] **AC-1** — A contract with a local descriptor renders labeled fields offline.
- [ ] **AC-2** — Absent a local descriptor, decoding defers to remote/standard layers (I02).
- [ ] **AC-3** — Descriptor field roles drive correct risk hints (I08).

## 8. Out of scope / non-goals

- Cascade order — **I02**; remote descriptors — **I02**; risk math — **I08**.

## 9. Dependencies, risks & open questions

- **Risk:** local descriptors drifting from real contract behavior — review on change.
- **Open question:** which additional contracts to bundle vs fetch.

## 10. Source anchors

- `src/services/local-descriptors.ts`, `src/services/clear-signing.ts`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 66.
