# O04 · Roadmap, Alpha Status & Public On-Chain Verification

| | |
|---|---|
| **Epic** | O — Ops, Testing, Store & Meta |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A02 |
| **Related** | B08, A01, N01 |

## 1. Summary

Vela publishes its **roadmap** and **alpha status** openly, and backs "don't trust — verify" with a
**live on-chain "wallets created" counter** — read from the Passkey Index contract on **Gnosis**
(`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`) through an auto-failover pool of public RPCs. Every wallet
is on-chain, every line is on GitHub, every claim is checkable. Status is shown as **"alpha · v0.1"** —
candor over hype.

## 2. Background & context

Public verifiability is the brand's spine (A02). The roadmap ("directions, not deadlines") sets honest
expectations; the on-chain counter demonstrates the architecture rather than asserting it. Both reinforce
that the project is built in the open by a solo founder (clue 93).

## 3. Users & stories

- As an **evaluator**, I want to verify Vela's claims on-chain and on GitHub, so that I don't have to trust marketing.
- As a **user**, I want an honest roadmap and status, so that I know what's shipped vs coming.

## 4. Functional requirements

- **FR-1** — Publish a roadmap (shipped / in-progress / next / exploring) as "directions, not deadlines."
- **FR-2** — Show alpha status openly ("alpha · v0.1"); no fear banners (A02).
- **FR-3** — Display a **live on-chain "wallets created" counter** read from the Passkey Index on Gnosis (`0xdd93420B…E9c3`) via an auto-failover public-RPC pool (F03/B08).
- **FR-4** — Link to the open-source repo (`github.com/mondaylabsltd/vela-wallet`) and the on-chain data so claims are checkable.
- **FR-5** — Keep roadmap items mapped to forward-requirements in the PRD set (see this index's roadmap section).

## 5. Non-functional requirements

- **NFR-1** — The counter degrades gracefully if RPCs are unavailable (no fake number).
- **NFR-2** — All status/roadmap copy preserves the mandated honesty/audit framing (A02).

## 6. UX / flow notes

Roadmap lives at `getvela.app/roadmap`; About (N01) carries status + verification links. The counter is a marketing-site element demonstrating the architecture.

## 7. Acceptance criteria

- [ ] **AC-1** — The wallets-created counter reads live from the Gnosis Passkey Index with RPC failover.
- [ ] **AC-2** — Roadmap accurately reflects shipped vs upcoming, without deadline promises.
- [ ] **AC-3** — Status shows "alpha," never a "tolerate bugs" banner (A02).

## 8. Out of scope / non-goals

- The Passkey Index upload mechanics — **B08**; brand phrasing rules — **A02**.

## 9. Dependencies, risks & open questions

- **Risk:** stale roadmap drifting from reality — keep it current with shipped work.
- **Open question:** None.

## 10. Source anchors

- `getvela.app/src/routes/+page.svelte:92-206` (on-chain counter), `getvela.app/src/routes/roadmap/+page.svelte`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 9, 16, 93.
