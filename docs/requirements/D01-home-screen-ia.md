# D01 · Home Screen IA (Activity-First)

| | |
|---|---|
| **Epic** | D — Balances, Portfolio & Activity |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | C02 |
| **Related** | D02, D08, H01, H09, K03, M03 |

## 1. Summary

The home screen is organized **activity-first** (payments and recent activity foreground; balances and
a single connection surfaced), not as an asset-heavy portfolio grid. It presents the total balance
(via `AmountText`, M03), a primary action dock (send/receive/scan — WaveDock, M04), the activity feed
(D08), and rate-limit-tolerant balances (F08).

## 2. Background & context

Vela is payment-first and minimal (A01). The home IA was rebuilt from asset-first to
activity-first/single-screen: Activity = ERC-20 + EIP-7708 transfers; Assets = the holdings list
(`HoldingsList` → token detail); Connections = a single dApp session. The goal is that the most
common job (pay / see what happened) is the default view, with holdings one visible tap away.

2026-07 hero simplification: the hero is bare — label (`Total balance · CODE`) plus the number,
which tap-toggles balance privacy (persisted; also masks the feed and holdings). The
display-currency control moved to Settings › Localization (N01 FR-1, E06); the external
"Statement" link was removed (per-tx explorer links in the detail sheet cover verification); the
tap-balance asset sheet was replaced by the Assets tab.

## 3. Users & stories

- As a **user**, I want to see recent activity and my balance immediately, so that the common jobs are one glance away.
- As a **user**, I want send/receive/scan within thumb reach, so that paying is fast.

## 4. Functional requirements

- **FR-1** — Show total balance atomically (M03) with graceful states: loading, cached (F08), and error.
- **FR-2** — Foreground the **activity feed** (D08): ERC-20 transfers + EIP-7708 native transfers, plus pending dApp ops (L01).
- **FR-3** — Provide the primary action dock (send H01, receive H09, scan) via WaveDock (M04) with the emerging Scan FAB.
- **FR-4** — Surface the single active dApp connection (K03) as a compact entry, not a full panel.
- **FR-5** — Pull-to-refresh (VelaRefresh, M04) re-fetches balances/activity; rate-limited chains show cached data without a scary banner (F08).

## 5. Non-functional requirements

- **NFR-1** — First paint uses cached data (A06) then reconciles to fresh (D02) — never blocks on network.
- **NFR-2** — Layout adapts across text scales (M02) and platforms (A04).

## 6. UX / flow notes

Single-screen, scrollable. The hero's only interaction is tap-to-hide (an EyeOff glyph appears
only beside the masked value). Holdings live in the `[ Activity | Assets | Connections ]`
segmented toggle; the network filter applies to both Activity and Assets. Design = "depth through
shadow, not glass" (M01). No feature grid.

## 7. Acceptance criteria

- [ ] **AC-1** — Home renders cached balance/activity instantly on launch, then updates to fresh.
- [ ] **AC-2** — Send/receive/scan are reachable from the dock.
- [ ] **AC-3** — A rate-limited chain shows cached balance, not an error banner.

## 8. Out of scope / non-goals

- Portfolio aggregation internals — **D02**; activity data — **D08**; primitives — **M04**.

## 9. Dependencies, risks & open questions

- **Risk:** activity completeness (native/internal deposits) — tracked as forward FR in D05.
- **Open question:** None.

## 10. Source anchors

- `src/screens/wallet/HomeScreen.tsx`, `src/components/ui/HoldingsList.tsx`, `src/components/ui/WaveDock.tsx`, `src/components/ui/VelaRefresh.tsx`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 64, 79, 85; memory `project_home_ia_redesign`.
