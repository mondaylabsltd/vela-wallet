# G06 · Gas Account (Relayer EOA) & Sponsored Activation

| | |
|---|---|
| **Epic** | G — Transaction Engine (ERC-4337) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F07 |
| **Related** | G05, G07, F02 |

## 1. Summary

The first transaction on each network activates a **dedicated per-Safe, per-chain "gas account"
(relayer EOA)**. New users may get **Free Activation (sponsored)**; the deposit is **non-refundable**,
tops up from gas refunds, and can run down / need re-activation. Sponsorship is **capped per wallet**,
depends on Vela's per-chain treasury, and is **never offered on custom/test networks** (server-gated on
nonce ≤ 3 + WebAuthn registration + treasury balance).

## 2. Background & context

Because gas is paid from the user's balance via a relayer (G05), each Safe needs a funded relayer EOA
per chain. Sponsoring the first activation removes the cold-start friction of "you need gas to get
gas," but it must be bounded (anti-abuse) and honest (non-refundable, may need re-activation).

## 3. Users & stories

- As a **new user**, I want my first tx on a chain sponsored where possible, so that I can start without pre-funding gas.
- As a **user**, I want honest terms (non-refundable, may re-activate), so that I'm not surprised later.

## 4. Functional requirements

- **FR-1** — Activate a dedicated relayer EOA per Safe per chain on the first tx.
- **FR-2** — Offer **Free Activation (sponsored)** when eligible; the deposit is **non-refundable** and tops up from gas refunds.
- **FR-3** — Gate sponsorship server-side on: nonce ≤ 3, WebAuthn registration present, and sufficient per-chain treasury; cap per wallet.
- **FR-4** — **Never** sponsor on custom/test networks (F02).
- **FR-5** — Surface honest state: activated / running low / needs re-activation.

## 5. Non-functional requirements

- **NFR-1** — Anti-abuse: eligibility is server-enforced, not client-claimable.
- **NFR-2** — Degrades to user-funded activation when sponsorship is unavailable.

## 6. UX / flow notes

First-tx-on-chain may show a "Free Activation" state; when a gas account runs low, the underfunded flow (G07) surfaces a top-up modal (`BundlerFundingModal`).

## 7. Acceptance criteria

- [ ] **AC-1** — An eligible new user's first tx on a supported chain can be sponsored.
- [ ] **AC-2** — Custom/test networks are never sponsored.
- [ ] **AC-3** — A depleted gas account prompts re-activation/top-up (G07), not a silent failure.

## 8. Out of scope / non-goals

- Underfunded detection/modal — **G07**; fee model — **G05**.

## 9. Dependencies, risks & open questions

- **Risk:** treasury exhaustion changes eligibility dynamically — must degrade gracefully.
- **Open question:** per-chain sponsorship caps tuning.

## 10. Source anchors

- `src/services/bundler-service.ts:118-227` — gas account activation + sponsorship gating.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 18.
