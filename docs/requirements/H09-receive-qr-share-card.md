# H09 · Receive Screen, QR & Share Card

| | |
|---|---|
| **Epic** | H — Send & Receive |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | B07 |
| **Related** | H04, E07, M04 |

## 1. Summary

The receive screen shows the user's **address (identical on every chain, B07)** as a scannable QR and a
shareable **receive card** (a rendered image users can save/share). Because the address is counterfactual
(B07), users can receive **before the account is deployed**. Optional controls let a user request a
specific token/amount (an EIP-681-style payment request the sender can scan, H04).

## 2. Background & context

Receiving must be dead-simple and reassuring — the same address everywhere removes "which chain?"
confusion. A polished share card fits the "big-tech custom UX" bar (M04) and makes sharing an address
feel intentional, not raw.

## 3. Users & stories

- As a **user**, I want a QR and address to receive funds, so that others can pay me easily.
- As a **user**, I want to share a nice receive card, so that requesting payment looks trustworthy.
- As a **user**, I want to request a specific amount/token, so that the sender's app pre-fills it (H04).

## 4. Functional requirements

- **FR-1** — Display the account address (same on all chains, B07) with copy and a scannable QR.
- **FR-2** — Render a shareable **receive card** image (`ReceiveShareCard` / `share-card.ts`) that can be saved/shared via the platform seam (A04).
- **FR-3** — Communicate that funds can be received before deployment (counterfactual, B07).
- **FR-4** — Optional request controls: choose token/amount to encode an EIP-681 payment request (decoded by H04 on the sender side).
- **FR-5** — Copy/share feedback uses haptics + copy-feedback (M04).

## 5. Non-functional requirements

- **NFR-1** — Card rendering works on web (`dom-to-image`/`view-shot`) and native (A04).
- **NFR-2** — QR encodes a correct, checksummed address / EIP-681 URI.

## 6. UX / flow notes

`ReceiveScreen` + `ReceiveRequestControls` + `ReceiveShareCard`. Copy-address gives premium haptic/branded feedback (M04). No chain picker needed for the base address (same everywhere).

## 7. Acceptance criteria

- [ ] **AC-1** — The receive QR scans to the correct address on another device.
- [ ] **AC-2** — A share card renders and can be saved/shared on web and native.
- [ ] **AC-3** — A requested token/amount produces a scannable EIP-681 payment request (H04).

## 8. Out of scope / non-goals

- Sender-side scan/parse — **H04**; deployment — **G01**.

## 9. Dependencies, risks & open questions

- **Risk:** users assuming a per-chain address — copy clarifies it's the same everywhere (B07).
- **Open question:** None.

## 10. Source anchors

- `src/screens/wallet/ReceiveScreen.tsx`, `src/components/ReceiveShareCard.tsx`, `src/components/ReceiveRequestControls.tsx`, `src/services/share-card.ts`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 6, 85.
