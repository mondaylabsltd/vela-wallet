# K07 · EIP-5792 Batch Calls (`wallet_sendCalls` → `multiSend`)

| | |
|---|---|
| **Epic** | K — dApp Connect (WalletPair) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | G01 |
| **Related** | K04, J05, H07, L01 |

## 1. Summary

Vela supports **EIP-5792 `wallet_sendCalls`**: multiple calls batch into a **single Safe `multiSend`
UserOp** — one atomic transaction, one signature. The returned **batch id equals the userOpHash** for
receipt lookup (G09). Every leg is run through the never-unlimited-approval guard (J05) so a batch can't
smuggle an unbounded approval past the per-tx check.

## 2. Background & context

Atomic batching (approve+swap, multi-step DeFi) is a first-class 4337/Safe capability. Exposing it via
EIP-5792 — and advertising `atomic: supported` per chain (K04) — lets dApps do multi-call flows safely,
reusing the same MultiSend engine as split/sweep (H07).

## 3. Users & stories

- As a **dApp user**, I want multi-step actions to execute atomically with one signature, so that I don't approve N times.
- As a **user**, I want batch legs guarded, so that a batch can't hide an unlimited approval (J05).

## 4. Functional requirements

- **FR-1** — Implement `wallet_sendCalls`: encode the calls into a single Safe `multiSend` UserOp (G01).
- **FR-2** — Return a **batch id equal to the userOpHash** for receipt lookup (G09).
- **FR-3** — Run **each leg** through `enforceNoUnlimited` (J05); one unbounded leg fails the batch.
- **FR-4** — Advertise per-chain `atomic: supported` to dApps (K04).
- **FR-5** — Persist the batch as a pending dApp op at submit time (L01).

## 5. Non-functional requirements

- **NFR-1** — Atomic: all legs execute or the op reverts.
- **NFR-2** — One biometric signature for the whole batch (B02).

## 6. UX / flow notes

The signing sheet (I01) shows the batch as one transaction with its combined effect (J02). Confirmed via slide-to-confirm (M04) + biometric.

## 7. Acceptance criteria

- [ ] **AC-1** — A `wallet_sendCalls` with multiple calls settles as one MultiSend UserOp.
- [ ] **AC-2** — The returned batch id equals the userOpHash and looks up the receipt.
- [ ] **AC-3** — A batch containing an unbounded approval leg is rejected (J05).

## 8. Out of scope / non-goals

- UserOp/MultiSend engine — **G01**; approval guard — **J05**; advanced send (split/sweep) — **H07**.

## 9. Dependencies, risks & open questions

- **Risk:** large batches vs gas ceiling (G03) — bound call counts.
- **Open question:** partial-batch semantics (currently atomic all-or-nothing).

## 10. Source anchors

- `src/hooks/use-dapp-signing.ts:341-435`, `src/services/safe-transaction.ts:172-191`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 34.
