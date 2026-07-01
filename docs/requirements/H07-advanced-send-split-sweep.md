# H07 · Advanced Send: Split (1→N) & Sweep (N→1)

| | |
|---|---|
| **Epic** | H — Send & Receive |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | G01, K07 |
| **Related** | H08, H06, G05 |

## 1. Summary

Two advanced modes, **mutually exclusive**: **split** (one token → many recipients, 一币多人) and
**sweep** (many tokens → one address, 一址归集). Each executes as **one MultiSend UserOp** via
`sendBatchCalls`. Max-send reserves native gas (EntryPoint prefund) so a sweep can't revert with an
AA21-class error. Both are built, committed, and typecheck-clean.

## 2. Background & context

Paying many people or consolidating dust normally means N transactions. Batching into a single atomic
MultiSend (one signature, one gas payment) is faster and cheaper, and it's the execution engine under
the payroll importer (H08).

## 3. Users & stories

- As a **user**, I want to send one token to many people at once, so that payouts are a single action.
- As a **user**, I want to sweep many tokens to one address, so that I consolidate cheaply and atomically.

## 4. Functional requirements

- **FR-1** — **Split:** given a token and N `SplitRecipient { address, amount }`, build `buildSplitCalls` → MultiSend; `sumSplitBaseUnits` computes the total for balance check.
- **FR-2** — **Sweep:** given N tokens → one recipient, `buildMultiTokenCalls`; `selectAllValuable` / `isMultiSelectable` pick tokens with value.
- **FR-3** — Split and sweep are **mutually exclusive** in the UI.
- **FR-4** — Execute as **one** `sendBatchCalls` MultiSend UserOp (G01/K07).
- **FR-5** — `reserveNativeGas` reserves the EntryPoint prefund so a full sweep of native can't revert (AA21).

## 5. Non-functional requirements

- **NFR-1** — Base-unit/BigInt math throughout; totals exact at each token's decimals (D04).
- **NFR-2** — Atomic: all legs succeed or the op reverts (no partial payout).

## 6. UX / flow notes

`MultiRecipientEditor` for split; multi-select token list for sweep. Group selection (H06) prefills split recipients. Fee shown once (G05) for the whole batch.

## 7. Acceptance criteria

- [ ] **AC-1** — A split to N recipients settles as one MultiSend UserOp.
- [ ] **AC-2** — A sweep of native + tokens reserves gas and does not revert.
- [ ] **AC-3** — Split and sweep cannot be active simultaneously.

## 8. Out of scope / non-goals

- Table import (payroll) — **H08**; groups — **H06**; EIP-5792 batch from dApps — **K07**.

## 9. Dependencies, risks & open questions

- **Risk:** very large batches vs gas ceiling — bound recipient/token counts.
- **Open question:** per-recipient failure semantics (currently all-or-nothing, atomic).

## 10. Source anchors

- `src/services/batch-send.ts:82` (`buildSplitCalls`), `:90` (`sumSplitBaseUnits`), `:101` (`buildMultiTokenCalls`), `:135` (`selectAllValuable`), `:148` (`reserveNativeGas`).
- `src/components/send/MultiRecipientEditor.tsx`.
- memory `project_advanced_send_modes`; `docs/CONTENT-SOURCE-100-CLUES.md` — clue 98.
