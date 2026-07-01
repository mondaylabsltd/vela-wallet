# D05 · Received-Transfer Detection (Log Polling + EIP-7708 + Allowlist)

| | |
|---|---|
| **Epic** | D — Balances, Portfolio & Activity |
| **Status** | ✅ Shipped (deposit tracing forward FR 🚧) |
| **Owner** | Shelchin |
| **Depends on** | F03 |
| **Related** | D08, F05, F08, D01 |

## 1. Summary

Vela detects incoming transfers **client-side** by polling `eth_getLogs` per chain on the ERC-20
`Transfer` topic with the wallet as recipient. Thanks to **EIP-7708**, native ETH transfers emit the
same event, so **one query catches both** ERC-20 and native (native recognized by sentinel emitters).
Results are filtered against a **per-chain allowlist** (known stablecoins + user-added tokens + native
sentinels) and re-validated client-side, so airdrop spam can't slip in.

## 2. Background & context

There's no third-party indexer (A01/A03) — deposit detection must be on-chain. The allowlist is a
deliberate anti-spam gate: unsolicited scam tokens don't get to render a fake deposit. **Forward FR
(🚧 roadmap "See every coin you receive"):** plain native deposits and coins arriving via internal
calls emit no log on most chains today, so a block-tracing transfer service is being built to surface
those too.

## 3. Users & stories

- As a **user**, I want to see coins I receive, so that deposits show up without a manual refresh.
- As a **user**, I don't want scam-airdrop tokens cluttering my activity, so that only real/known deposits show.

## 4. Functional requirements

- **FR-1** — Per chain, run one `eth_getLogs` on the `Transfer` topic filtered to the wallet as recipient.
- **FR-2** — EIP-7708: recognize native transfers via sentinel emitters in the same query.
- **FR-3** — Filter to a per-chain **allowlist** (known stablecoins + user-added tokens D03 + native sentinels); re-validate logs client-side.
- **FR-4** — Cooperate with `eth_getLogs` range-cap handling (F05) by splitting ranges when the RPC caps them.
- **FR-5 (🚧 forward)** — Trace blocks to surface native/internal-call deposits that emit no log, on every chain.

## 5. Non-functional requirements

- **NFR-1** — Best-effort and rate-limit tolerant (F08); unreachable RPC yields no false "you received X."
- **NFR-2** — Log amounts are treated with the asymmetric-trust caution used elsewhere (J03) — a received amount is only confident when the token is known.

## 6. UX / flow notes

Detected deposits appear in the activity feed (D08) and home (D01). No push notifications (no telemetry backend, A03) — detection is on-foreground/refresh.

## 7. Acceptance criteria

- [ ] **AC-1** — An incoming known-stablecoin transfer appears in activity after a poll.
- [ ] **AC-2** — An EIP-7708 native transfer is detected by the same query.
- [ ] **AC-3** — An unsolicited non-allowlisted token does **not** render as a deposit.

## 8. Out of scope / non-goals

- Outgoing tx tracking — **D08/G09**; range-cap mechanics — **F05**.

## 9. Dependencies, risks & open questions

- **Risk:** chains without EIP-7708 or with log gaps miss native/internal deposits — the 🚧 tracing service addresses this.
- **Open question:** performance/coverage bounds of the block-tracing service (in progress).

## 10. Source anchors

- `src/services/transfer-monitor.ts:38-221` — log polling + EIP-7708 + allowlist.
- `getvela.app/src/routes/roadmap/+page.svelte` — "See every coin you receive" (🚧).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 64.
