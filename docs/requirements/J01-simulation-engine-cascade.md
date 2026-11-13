# J01 · Simulation Engine Cascade (`eth_simulateV1` → Tevm → `eth_call`)

> **Superseded (R23): the code contradicts the simulation cascade. There is no Tevm → `eth_simulateV1` → `eth_call` cascade on the signing path. The signing sheet's verdict is one `eth_simulateV1` read, asked of two independent providers (or the person's own node) and compared by meaning in the core. One node answering, two that disagree, no answer, or a simulation that reverts makes the confirm "… anyway" (held 1.5 s). A non-answer is a caution, never "no info". Tevm survives only as a disabled seam in the web send path.**
>
> **What is true:** `rust/crates/vela-core/src/app/sim_outcome.rs`, `rust/crates/vela-core/src/app/corroborate.rs`, `rust/crates/vela-core/src/app/sign_confirm.rs` (`posture`); `docs/security/hardening.md` §12–§13. Checked against the code at commit `fa6083c87` (R23, 2026-11-09). This document is kept as history; it does not describe the product.

| | |
|---|---|
| **Epic** | J — Simulation & Safety Guards |
| **Status** | ⚠️ Superseded (R23, 2026-11-09): see the note above |
| **Owner** | Shelchin |
| **Depends on** | F03 |
| **Related** | J02, J03, J04, I01 |

## 1. Summary

`simulateAssetChanges` predicts a transaction's effects through a **3-tier engine cascade**: (1)
`eth_simulateV1` via the **user's own RPC**, (2) an optional local **Tevm** fork, (3) a single
`eth_call` revert pre-check. **No third-party simulation service.** The result drives a balance-change
preview (J02) before signing; a `null` result always means **"no info," never a false "will fail."**

## 2. Background & context

Simulation lets users see what a transaction does before signing (I01). Keeping it on the user's own RPC
(plus an optional local fork) avoids trusting a third-party simulator (A03) and keeps privacy. The
cascade degrades gracefully: better engines first, cheap revert-check last.

## 3. Users & stories

- As a **user**, I want to preview a transaction's effects before signing, so that I catch surprises.
- As a **user**, I want "no info" to be honest, so that a failed simulation never masquerades as "this will fail."

## 4. Functional requirements

- **FR-1** — Try `eth_simulateV1` on the user's RPC (F03) first.
- **FR-2** — Optionally use a local **Tevm** fork (a disabled seam behind a `new Function` import escape) when available.
- **FR-3** — Fall back to a single `eth_call` revert pre-check.
- **FR-4** — Return `null` = **no information** (not "will fail"); a genuine revert is distinct (J04).
- **FR-5** — Use **no third-party** simulation service.

## 5. Non-functional requirements

- **NFR-1** — Runs over the resilient pool (F03/F04); a simulation failure never blocks signing (it degrades to "no preview").
- **NFR-2** — The Tevm seam is import-isolated so it never bloats the default bundle.

## 6. UX / flow notes

Feeds `BalanceChangePreview` (J02) in the signing sheet (I01). When simulation yields nothing, the sheet shows "no preview available," not a false failure.

## 7. Acceptance criteria

- [ ] **AC-1** — A simulatable tx returns predicted asset changes via `eth_simulateV1`.
- [ ] **AC-2** — When simulation can't run, the result is `null` = no info (not "will fail").
- [ ] **AC-3** — No third-party simulation endpoint is contacted.

## 8. Out of scope / non-goals

- Preview rendering — **J02**; trust model — **J03**; revert text — **J04**.

## 9. Dependencies, risks & open questions

- **Risk:** `eth_simulateV1` support varies per RPC — cascade covers gaps.
- **Open question:** enabling the Tevm fork by default (currently a disabled seam).

## 10. Source anchors

- `src/services/tx-simulation.ts:162`, `src/services/sim-engine-rpc.ts`, `src/services/sim-engine-tevm.ts`.
- memory `project_tx_simulation_module`; `docs/CONTENT-SOURCE-100-CLUES.md` — clue 25.
