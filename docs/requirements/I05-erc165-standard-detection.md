# I05 · ERC-165 Standard Detection (ERC-20 vs ERC-721)

| | |
|---|---|
| **Epic** | I — Clear Signing & Decoding |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | I01 |
| **Related** | I02, I06, I07 |

## 1. Summary

`transferFrom` / `approve` selectors **collide** between ERC-20 and ERC-721, so a naive decode can
confuse an **amount** with a **tokenId**. `detectTokenStandard` probes `supportsInterface(0x80ac58cd)`
(ERC-721) / `0xd9b67a26` (ERC-1155) **in parallel** and caches **only definitive verdicts** — a transient
RPC `null` re-probes next time rather than poisoning the cache.

## 2. Background & context

Rendering an ERC-721 `transferFrom` as if the third arg were an amount (or vice versa) is a real
security-surface error. ERC-165 interface detection disambiguates before the field is labeled, so the
signing sheet (I01) shows "tokenId" vs "amount" correctly.

## 3. Users & stories

- As a **user**, I want NFT vs token transfers labeled correctly, so that I don't misread a tokenId as an amount.
- As a **maintainer**, I want only definitive verdicts cached, so that a flaky RPC doesn't lock in a wrong standard.

## 4. Functional requirements

- **FR-1** — For colliding selectors (`transferFrom`/`approve`), run `detectTokenStandard` before labeling fields.
- **FR-2** — Probe `supportsInterface(0x80ac58cd)` (721) and `0xd9b67a26` (1155) **in parallel**.
- **FR-3** — Cache **only definitive** verdicts; a transient `null` triggers a re-probe next time.
- **FR-4** — Feed the resolved standard into field labeling (I06) and amount/decimals handling (I07).

## 5. Non-functional requirements

- **NFR-1** — Parallel probes are bounded; unreachable → treat as unresolved (caution, I08), not a wrong guess.
- **NFR-2** — No permanent caching of an indeterminate result.

## 6. UX / flow notes

Correct "amount" vs "tokenId" labels in the signing sheet (I01). An unresolved standard floors risk at caution (I08).

## 7. Acceptance criteria

- [ ] **AC-1** — An ERC-721 `transferFrom` labels the third arg as tokenId, not amount.
- [ ] **AC-2** — An ERC-20 `transferFrom` labels an amount with correct decimals (I07).
- [ ] **AC-3** — A transient RPC null re-probes rather than caching a wrong verdict.

## 8. Out of scope / non-goals

- ABI decoding — **I06**; decimals/amount rendering — **I07**; risk math — **I08**.

## 9. Dependencies, risks & open questions

- **Risk:** contracts that lie about ERC-165 — combine with descriptor/standard-method context (I02).
- **Open question:** ERC-1155 amount+id rendering depth.

## 10. Source anchors

- `src/services/clear-signing.ts:187-208` — `detectTokenStandard`, parallel probes, definitive-only cache.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 68.
