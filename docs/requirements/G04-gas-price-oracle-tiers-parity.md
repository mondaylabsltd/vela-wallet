# G04 · Gas Price Oracle, Tiers & Wallet↔Bundler Parity

| | |
|---|---|
| **Epic** | G — Transaction Engine (ERC-4337) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F07 |
| **Related** | G03, G05, G10 |

## 1. Summary

The **bundler is the source of truth for gas price** (the wallet never marks it up). Authoritative
pricing comes from `pimlico_getUserOperationGasPrice`; Vela-specific `networkFeePerGas` /
`relayerFeePerGas` fields are read when present. Named tiers scale the price: **slow ×1.1, standard
×1.2, rapid ×1.5, fast ×2.0**. To fix a chronic "gas price too low / —" issue on Gnosis, the wallet's
`getGasPrices` must include `eth_maxPriorityFeePerGas` and mirror the bundler's `networkPrice = max(eth_gasPrice, baseFee + tip)`.

## 2. Background & context

If the wallet and bundler compute gas price differently, ops get rejected as underpriced or show "—".
Parity — same inputs, same formula — is required. The bundler owning the price keeps the fee model
honest (G05): the wallet can't inflate it.

## 3. Users & stories

- As a **user**, I want a fee that the bundler will actually accept, so that my tx isn't rejected as too cheap.
- As a **user**, I want to pick speed (slow/standard/rapid/fast), so that I trade cost for confirmation time.

## 4. Functional requirements

- **FR-1** — Fetch authoritative gas price via `pimlico_getUserOperationGasPrice`; prefer Vela `networkFeePerGas`/`relayerFeePerGas` when present.
- **FR-2** — Apply named tier multipliers: slow ×1.1, standard ×1.2, rapid ×1.5, fast ×2.0.
- **FR-3** — `getGasPrices` includes `eth_maxPriorityFeePerGas`; compute `networkPrice = max(eth_gasPrice, baseFee + tip)` to mirror the bundler.
- **FR-4** — Tier caps are **tier-aware** and exclude Tempo (G10, stablecoin gas).
- **FR-5** — Never mark up the bundler's price beyond the selected tier.

## 5. Non-functional requirements

- **NFR-1** — Wallet↔bundler parity: identical formula/inputs so quotes agree.
- **NFR-2** — Priced over the forwarded RPC (F07).

## 6. UX / flow notes

`GasFeeCard` shows the tier selector and the split fee (network + relayer, G05). A "—" fee indicates a parity/price gap — the bug this parity fixes.

## 7. Acceptance criteria

- [ ] **AC-1** — On Gnosis, fees render numerically (not "—") with priority fee included.
- [ ] **AC-2** — Selecting "fast" doubles vs base per the ×2.0 tier.
- [ ] **AC-3** — The wallet's quote matches what the bundler accepts (no underprice rejection).

## 8. Out of scope / non-goals

- Estimation of gas **limits** — **G03**; fee cap/split display — **G05**; Tempo — **G10**.

## 9. Dependencies, risks & open questions

- **Risk:** cross-repo drift with `vela-relay`'s networkPrice formula — keep in sync (G07).
- **Open question:** per-chain tip defaults where `eth_maxPriorityFeePerGas` is unsupported.

## 10. Source anchors

- `src/services/safe-transaction.ts:225-230,1336-1381` — tiers + price oracle.
- memory `project_wallet_bundler_gasprice_parity`; `docs/CONTENT-SOURCE-100-CLUES.md` — clue 51.
