# G05 · Fee Model (≈2× Cost, ~3× Cap Guard)

| | |
|---|---|
| **Epic** | G — Transaction Engine (ERC-4337) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | G04 |
| **Related** | G06, G10, H01, H02 |

## 1. Summary

Gas is paid from **your own wallet's native-token balance** (no paymaster). You pay roughly **2× the raw
on-chain cost** (network fee + relayer fee, shown **split** before you confirm), and the wallet
**refuses any quote above ~3× the network rate** (`GasQuoteTooHighError`,
`MAX_QUOTE_VS_CHAIN_MULTIPLE = 3n`). (The README's "60% markup" is stale.)

## 2. Background & context

A relayer submits the UserOp and must be reimbursed, hence the ~2× (network + relayer). Transparency
(showing the split) and a hard cap (~3×) protect users from being over-charged by a misbehaving or
mispriced bundler — the cap is a safety valve, not a target.

## 3. Users & stories

- As a **user**, I want to see exactly what I'm paying and why (network vs relayer), so that the fee is transparent.
- As a **user**, I want protection from an absurd quote, so that a bad bundler can't overcharge me.

## 4. Functional requirements

- **FR-1** — Fee = network fee + relayer fee (≈2× raw cost), paid from the account's **native balance** (no paymaster).
- **FR-2** — Display the fee **split** (network + relayer) before confirmation.
- **FR-3** — **Refuse** any quote > ~3× the on-chain network rate: throw `GasQuoteTooHighError` (`MAX_QUOTE_VS_CHAIN_MULTIPLE = 3n`).
- **FR-4** — The bundler owns the price (G04); the wallet only validates the cap and displays the split.
- **FR-5** — Tempo settles this fee in a stablecoin instead of native (G10).

## 5. Non-functional requirements

- **NFR-1** — The cap is enforced pre-submit; a too-high quote never reaches signing.
- **NFR-2** — Fee display is honest and localized (E06/M02).

## 6. UX / flow notes

`GasFeeCard` shows split fee + tier (G04). Max-send reserves this fee so a send can't underfund gas (H01/H07). Insufficient native balance for gas routes to the gas-account/top-up flow (G06/G07).

## 7. Acceptance criteria

- [ ] **AC-1** — The confirm screen shows network and relayer fees separately.
- [ ] **AC-2** — A quote > ~3× network rate is rejected with `GasQuoteTooHighError`.
- [ ] **AC-3** — Gas is deducted from native balance (no paymaster path).

## 8. Out of scope / non-goals

- Price oracle/tiers — **G04**; gas account — **G06**; Tempo — **G10**.

## 9. Dependencies, risks & open questions

- **Risk:** stale "60% markup / one-time deposit" claims in old README — this doc is the truth.
- **Open question:** None.

## 10. Source anchors

- `src/services/safe-transaction.ts:1305,1373-1378` — fee split + `MAX_QUOTE_VS_CHAIN_MULTIPLE`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 17.
