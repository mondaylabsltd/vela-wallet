# G10 · Tempo Stablecoin-Gas Transaction Path

| | |
|---|---|
| **Epic** | G — Transaction Engine (ERC-4337) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | G01, F07 |
| **Related** | G04, G05, F01 |

## 1. Summary

**Tempo (chainId 4217) has no native coin** — gas is paid in **TIP-20 stablecoins** (default `pathUSD`
at the reserved `0x20c0…0000`, 6 decimals). Vela signs the UserOp with **`maxFeePerGas = 0`** and
**batches a `pathUSD.transfer(bundlerEOA, reimbursement)`** (priced ~2× realistic gas) into the
MultiSend; the bundler submits `handleOps` inside a native Tempo tx paying gas in the fee token. It uses
the **same EntryPoint + Safe + passkey + cross-chain address consistency** as every other chain.

## 2. Background & context

Founder mandate: keep the EntryPoint + Safe + passkey stack and address consistency (B06/B07) intact
even on a chain with no native gas coin. So instead of a paymaster, the reimbursement is an in-band
stablecoin transfer to the bundler EOA, and the bundler's outer tx pays gas in the fee token.

## 3. Users & stories

- As a **Tempo user**, I want to pay gas in a stablecoin, so that I don't need a native coin I can't get.
- As a **user**, I want the same wallet/address/passkey on Tempo as everywhere, so that nothing about my identity changes.

## 4. Functional requirements

- **FR-1** — Detect Tempo (4217); use `pathUSD` (`0x20c0…0000`, 6 decimals) as the default fee token.
- **FR-2** — Sign the UserOp with **`maxFeePerGas = 0`**.
- **FR-3** — Batch a `pathUSD.transfer(bundlerEOA, reimbursement)` (≈2× realistic gas, per the fee model G05) into the MultiSend (G01).
- **FR-4** — Forward the correct RPC so the bundler reaches Tempo and the reimbursement targets the right EOA (F07).
- **FR-5** — Exclude Tempo from native gas-price tiers/caps (G04).

## 5. Non-functional requirements

- **NFR-1** — Address/identity consistency with all chains (B07) is preserved.
- **NFR-2** — Reimbursement amount respects the fee-model transparency/cap intent (G05).

## 6. UX / flow notes

On Tempo, the fee is shown in the stablecoin, not a native coin. Otherwise the send/confirm flow (H01) is identical.

## 7. Acceptance criteria

- [ ] **AC-1** — A Tempo send signs with `maxFeePerGas = 0` and batches a `pathUSD` reimbursement.
- [ ] **AC-2** — The bundler's `handleOps` settles, paying gas in the fee token.
- [ ] **AC-3** — The Tempo address equals the user's address on every other chain (B07).

## 8. Out of scope / non-goals

- General fee model — **G05**; bundler routing — **F07**; other chains' native gas — **G04**.

## 9. Dependencies, risks & open questions

- **Risk:** RPC must reach the correct bundler EOA (F07) or reimbursement misfires.
- **Open question:** additional TIP-20 fee tokens beyond `pathUSD`.

## 10. Source anchors

- `src/services/tempo.ts`, `src/services/safe-transaction.ts:626-737`.
- memory `project_tempo_gas_integration`; `docs/CONTENT-SOURCE-100-CLUES.md` — clue 54.
