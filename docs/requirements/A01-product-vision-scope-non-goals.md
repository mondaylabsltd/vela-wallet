# A01 · Product Vision, Scope & Non-Goals ("Does Less on Purpose")

| | |
|---|---|
| **Epic** | A — Product Foundations & Cross-Cutting |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | — |
| **Related** | A02, A03, N01, all epics (this scopes them) |

## 1. Summary

Vela is **an open-source, self-custodial wallet for ETH and ERC-20s** whose entire product thesis is
*deliberate minimalism*: "a wallet that does less — on purpose." Every feature must earn its place
against a smaller attack surface. This doc fixes the scope and the **non-goals** so that later PRDs
inherit a consistent "what we will and won't build" boundary.

## 2. Background & context

Most wallets grow into NFT galleries, swap aggregators, DeFi dashboards, and in-app dApp browsers —
each a new attack surface and audit burden. Vela treats the feature *gap* as a security virtue:
"fewer paths to attack, fewer moving parts to audit." The one exception the product did take on is
a dApp browser: iOS, Android and desktop (macOS, Windows) ship one, and the Chrome extension injects
the same provider into ordinary pages (Epic K). Everything it opens still goes through one signing
path, which is how the gap stayed a virtue.

## 3. Users & stories

- As a **self-custody user**, I want a wallet that does the core things correctly, so that I trust it with real money.
- As a **security-minded user**, I want the product to *refuse* risky surface area, so that I'm protected by design, not by discipline.

## 4. Functional requirements

- **FR-1** — In scope: seedless passkey identity (Epic B), send/receive (H), multi-chain balances & pricing (D/E), clear signing (I/J), dApp connect via WalletPair (K).
- **FR-2** — Explicit non-goals: **no NFT gallery, no built-in swaps, no DeFi dashboard, no fiat on-ramp, no token/airdrop**. (An in-app dApp browser *was* a non-goal and is no longer: it ships on iOS, Android and desktop.)
- **FR-3** — Supported assets: **native coins and ERC-20s** across the 24 built-in chains (F01); native-coin gas per chain (plus Tempo's stablecoin gas, G10).
- **FR-4** — Any proposed feature must state which non-goal it does or doesn't cross, and justify the added surface.

## 5. Non-functional requirements

- **NFR-1** — Openness: MIT-licensed app + all three backend services, self-hostable (A06/N02).
- **NFR-2** — Runs in the browser with nothing to download; the four shells (web, desktop, iOS, Android) share one Rust core rather than one app codebase (A04).

## 6. UX / flow notes

Product surface stays small enough that the home screen (D01) can be activity-first, not a feature grid. Copy leans on "less is safer" (see N — thought-leadership framing lives in content, not the app).

## 7. Acceptance criteria

- [ ] **AC-1** — The shipped app contains none of the FR-2 non-goals.
- [ ] **AC-2** — Supported-asset claims match F01's chain list and the KNOWN_TOKENS registry (D04).
- [ ] **AC-3** — Every other PRD references A01 for its scope boundary.

## 8. Out of scope / non-goals

- Brand voice / taglines — see **A02**. Privacy/no-token specifics — see **A03**.

## 9. Dependencies, risks & open questions

- **Risk:** scope creep; each new surface must be weighed against the minimalism thesis.
- **Open question:** None (vision is settled; roadmap additions must still honor the non-goals).

## 10. Source anchors

- `app-web/getvela.app/src/routes/+page.svelte` — positioning & "does less" framing.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 1, 2, 8, 11, 13, 14, 15.
