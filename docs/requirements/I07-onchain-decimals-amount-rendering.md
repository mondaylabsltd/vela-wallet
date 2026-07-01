# I07 · On-Chain Decimals & BigInt Amount Rendering

| | |
|---|---|
| **Epic** | I — Clear Signing & Decoding |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | D04 |
| **Related** | I01, I05, I08, M03 |

## 1. Summary

In signing, token amounts are rendered using **decimals fetched on-chain, never assumed 18** — a
6-decimal token shown as 18 would overstate an amount by **1e12** on a security surface. Unresolved
decimals fall back to 18 but flag `unverified` (flooring risk at caution, I08). All amount math is
**BigInt** because `10**decimals` as a JS number loses precision past ~23 decimals.

## 2. Background & context

The signing sheet is where a wrong amount is most dangerous. Decimals therefore come from the robust
metadata layer (D04), and any uncertainty is surfaced rather than hidden. Float math is banned here for
correctness.

## 3. Users & stories

- As a **user**, I want the exact amount I'm approving/sending shown, so that I never misjudge magnitude.
- As a **user**, I want uncertainty (unknown decimals) flagged, so that I don't trust a possibly-wrong number.

## 4. Functional requirements

- **FR-1** — Resolve token decimals via the metadata layer (D04) — on-chain, cached; never assume 18.
- **FR-2** — If decimals can't be resolved, fall back to 18 but mark the field `unverified` (floors risk at caution, I08).
- **FR-3** — Use **BigInt** for all amount scaling; never compute `10**decimals` as a JS number.
- **FR-4** — Render amounts via the atomic display path (M03) with correct grouping (M02).
- **FR-5** — Combine with ERC-165 detection (I05) so tokenId vs amount is never confused.

## 5. Non-functional requirements

- **NFR-1** — Exact at any realistic decimals (6, 8, 18, and beyond).
- **NFR-2** — Unverified decimals never render as a confident value.

## 6. UX / flow notes

Amounts in the signing sheet (I01) show the token symbol and, when decimals are unverified, a caution marker.

## 7. Acceptance criteria

- [ ] **AC-1** — A 6-decimal approval amount renders exactly (no 1e12 inflation).
- [ ] **AC-2** — An unknown-decimals token shows `unverified` and caution (I08).
- [ ] **AC-3** — Amount math uses BigInt end-to-end.

## 8. Out of scope / non-goals

- Metadata resolution — **D04**; risk math — **I08**; atomic display — **M03**.

## 9. Dependencies, risks & open questions

- **Risk:** a spoofed `decimals()` — treated with the same caution as unresolved.
- **Open question:** None.

## 10. Source anchors

- `src/services/clear-signing.ts:365,1179` — decimals handling + BigInt amount math.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 69.
