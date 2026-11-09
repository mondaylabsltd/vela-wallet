# F01 · Supported Networks Registry (24 Chains + Custom)

> **Superseded (R23): the code contradicts the network list in part. The 24 mainnets (20 shown by default; Stable, MegaETH, Plume and XRPL EVM hidden) match the code. Not in this document: the built-in list also holds 9 testnets for the testnet environment (spec 105), and every read a signing verdict stands on is asked of two independent providers and chain-id probed first.**
>
> **What is true:** `rust/crates/vela-core/src/app/network_admin.rs` (`BUILTIN_CHAINS`), `rust/crates/vela-core/src/app/shown_networks.rs`; `docs/security/hardening.md` §12. Checked against the code at commit `fa6083c87` (R23, 2026-11-09). This document is kept as history; it does not describe the product.

| | |
|---|---|
| **Epic** | F — Networks & RPC Infrastructure |
| **Status** | ⚠️ Superseded (R23, 2026-11-09): see the note above |
| **Owner** | Shelchin |
| **Depends on** | — |
| **Related** | F02, F03, E04, G10 |

## 1. Summary

Vela ships a registry of **24 EVM networks** (12 until 2026-09-17, specs 060/061) — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base,
Avalanche, Gnosis, **Unichain, Tempo, Monad, World Chain, Arc, X Layer, Stable, Soneium, MegaETH,
Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume, XRPL EVM** — plus **user-added custom chains** (F02).
Each entry carries chainId, native coin, logos, and per-chain config that the rest of the app keys off.
(Source of truth: `BUILTIN_CHAINS` in `rust/crates/vela-core/src/app/network_admin.rs`; the site's list is checked against it by `app-web/getvela.app/src/lib/networks.test.ts`.)

**Shown by default: 20** (owner decision 2026-10-29). The table's order is the order every list shows
them: Ethereum, BNB Chain, Base, Polygon, Arbitrum, Avalanche, Optimism, Monad, Robinhood Chain, Arc,
X Layer, Mantle, Gnosis, Tempo, World Chain, Celo, Unichain, Ink, Soneium, Kaia. **Stable, MegaETH,
Plume and XRPL EVM** stay Vela networks (relay, fees, logos, clear signing) but are in a person's lists
only on their word (`shown_by_default: false`; `shown_networks.rs`, stored as `vela.shownNetworks`):
Settings → Add network (or a site's `wallet_addEthereumChain`) shows one again as a Vela network in one
step, and removing one keeps its settings. A device with no record is settled once, by the core: each
of the four stays shown where the device finds the person using it — a last-known balance, activity, a
multisig, a token added there, an RPC/explorer of their own, the network added by hand earlier.

## 2. Background & context

The account model (B06) and address (B07) are chain-independent, so "supporting a chain" is mostly
configuration: RPC discovery (F03), price venue (E02), Chainlink feed (E04), and — for custom chains —
a contract-suite check (F02). Tempo is special: no native coin, gas in stablecoins (G10).

## 3. Users & stories

- As a **multi-chain user**, I want the major EVM chains available out of the box, so that I can transact where I need to.
- As a **power user**, I want to add a custom chain, so that I'm not limited to the shipped list (F02).

## 4. Functional requirements

- **FR-1** — Provide the 24 built-in chains with chainId, native symbol/decimals, display name, and logo; 20 of them shown by default, in the table's order, the other four on the person's word (§1).
- **FR-2** — Mark chain-specific traits: Tempo has no native coin (stablecoin gas, G10); L2 gas bumps (G03).
- **FR-3** — Allow user-added custom chains, persisted locally (A06) after validation (F02).
- **FR-4** — Expose the active/selected network to balances (D02), pricing (E01), send (H01), and dApp capability advertisement (K04).

## 5. Non-functional requirements

- **NFR-1** — Registry is the single source for chain metadata; other modules reference it.
- **NFR-2** — Adding/removing a network is restart-free.

## 6. UX / flow notes

Network filter/selection surfaces (NetworkFilterSheet) let users scope views per chain. Custom networks are added via F02's validated flow.

## 7. Acceptance criteria

- [ ] **AC-1** — All 24 chains are known with correct native coin and logo; a fresh profile lists the 20 shown by default, in order; a profile holding money on one of the other four keeps it listed.
- [ ] **AC-2** — Tempo is flagged as stablecoin-gas (no native coin).
- [ ] **AC-3** — A validated custom chain persists and participates in balances/send.

## 8. Out of scope / non-goals

- Custom-network validation — **F02**; RPC discovery — **F03**; Tempo gas — **G10**.

## 9. Dependencies, risks & open questions

- **Risk:** stale "8 networks" claims in old docs — treat the registry as truth.
- **Open question:** signing path for chains lacking the P-256 precompile is 🧭 (F02).

## 10. Source anchors

- `rust/crates/vela-core/src/app/network_admin.rs` `BUILTIN_CHAINS` — the 24-chain registry (the Expo-era `src/models/chains.ts` is gone).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 14.
