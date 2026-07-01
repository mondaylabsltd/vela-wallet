# I02 · ERC-7730 Descriptor Cascade (Richest-First)

| | |
|---|---|
| **Epic** | I — Clear Signing & Decoding |
| **Status** | ✅ Shipped (wider coverage 🚧) |
| **Owner** | Shelchin |
| **Depends on** | I01 |
| **Related** | I03, I04, I05, I06, J05 |

## 1. Summary

For `eth_sendTransaction`, Vela resolves a human-readable meaning through a **richest-first cascade**:
built-in local descriptor (I03) → contract-specific **ERC-7730** descriptor
(`/erc7730/calldata/eip155-{chainId}/{to}.json`) → ERC-165-disambiguated standard token methods (I05)
→ ERC fallbacks (erc20/721/4626) → 4-byte best-effort (I04). Only if **all** fail does it blind-sign
(I01). **Wider coverage is a 🚧 roadmap goal.**

## 2. Background & context

Different contracts warrant different fidelity. A curated descriptor gives the best rendering; standard
detection covers common tokens; the 4-byte registry is the last resort. The cascade always prefers the
most specific source available, and never silently invents meaning.

## 3. Users & stories

- As a **user**, I want the clearest possible rendering for the contract I'm using, so that I understand the tx.
- As a **user on a niche contract**, I want a best-effort decode or an honest blind-sign warning, not a fake one.

## 4. Functional requirements

- **FR-1** — Cascade order: local descriptor (I03) → contract ERC-7730 JSON (`/erc7730/calldata/eip155-{chainId}/{to}.json`) → ERC-165 standard methods (I05) → ERC fallbacks (erc20/721/4626) → 4-byte best-effort (I04).
- **FR-2** — Stop at the first successful, sufficiently-specific decode.
- **FR-3** — If **all** layers fail, hand off to blind-sign warning (I01).
- **FR-4** — Feed the decoded fields into risk scoring (I08) and the approval guard (J05).
- **FR-5 (🚧 forward)** — Expand contract/chain descriptor coverage so fewer requests fall back to blind signing.

## 5. Non-functional requirements

- **NFR-1** — Descriptor fetch failures degrade to the next layer, never to a crash or a fake summary.
- **NFR-2** — Deterministic given the same inputs + available descriptors.

## 6. UX / flow notes

Transparent to the user — they just see the best available rendering (I01). "Advanced" reveals which layer/raw data was used.

## 7. Acceptance criteria

- [ ] **AC-1** — A contract with a local descriptor uses it over lower layers.
- [ ] **AC-2** — A standard ERC-20 transfer decodes via ERC-165/standard methods when no descriptor exists.
- [ ] **AC-3** — A fully-unknown call reaches the 4-byte/blind-sign path, not a fabricated summary.

## 8. Out of scope / non-goals

- Local descriptors — **I03**; 4-byte — **I04**; ERC-165 — **I05**; decoding — **I06**.

## 9. Dependencies, risks & open questions

- **Risk:** descriptor source availability per chain; coverage expansion is ongoing (🚧).
- **Open question:** hosting/update cadence for ERC-7730 descriptors.

## 10. Source anchors

- `src/services/clear-signing.ts:293` — cascade.
- `getvela.app/src/routes/roadmap/+page.svelte` — "Wider clear-signing coverage" (🚧).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 66.
