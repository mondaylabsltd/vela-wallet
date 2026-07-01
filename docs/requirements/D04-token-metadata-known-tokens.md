# D04 · Token Metadata & Known-Tokens Registry

| | |
|---|---|
| **Epic** | D — Balances, Portfolio & Activity |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F03 |
| **Related** | D02, D03, D05, I07 |

## 1. Summary

Vela resolves ERC-20 `symbol()`/`decimals()` **robustly on-chain** through a 3-layer cache
(static known-tokens → memory → AsyncStorage → on-chain), decodes legacy `bytes32` symbols (MKR) and
multibyte UTF-8 ("USD₮0"), and treats decimals as security-critical (never assumed 18). A single
consolidated **`KNOWN_TOKENS`** registry is the canonical source for well-known symbol/decimals.

## 2. Background & context

A 6-decimal token rendered as 18 overstates an amount by 1e12 — a security surface, not a cosmetic bug
(this feature exists because an unknown stablecoin once rendered as "+0 tokens"). Token facts were
previously duplicated in three places; they're now consolidated so there's one source of truth (clue
100).

## 3. Users & stories

- As a **user**, I want token amounts to be exactly right, so that I never misjudge what I'm sending/receiving.
- As a **maintainer**, I want one canonical token registry, so that facts don't drift across the codebase.

## 4. Functional requirements

- **FR-1** — Resolve `symbol`/`decimals` via cache cascade: static `KNOWN_TOKENS` → memory → AsyncStorage → on-chain read.
- **FR-2** — Decode legacy `bytes32` symbols and multibyte UTF-8 symbols correctly.
- **FR-3** — **Never assume 18 decimals**; unresolved decimals fall back to 18 but flag `unverified` (floors risk at caution in signing, I07).
- **FR-4** — Negative lookups are **session-only** (a transient RPC failure doesn't poison the cache permanently).
- **FR-5** — `KNOWN_TOKENS` (`src/services/tokens.ts`) is the single source for well-known symbol/decimals; other modules reference it, not private copies.

## 5. Non-functional requirements

- **NFR-1** — BigInt math for amounts; `10**decimals` never computed as a JS number (precision loss past ~23 decimals).
- **NFR-2** — Batched reads (40/call via D02) to minimize RPC.

## 6. UX / flow notes

No direct UI; underpins D02/D03 token rows and I07 amount rendering. Unknown tokens show `unverified` treatment rather than confident-but-wrong values.

## 7. Acceptance criteria

- [ ] **AC-1** — MKR's `bytes32` symbol and a multibyte symbol both render correctly.
- [ ] **AC-2** — A 6-decimal token's amount is exact.
- [ ] **AC-3** — A transient RPC null does not permanently cache a negative result.

## 8. Out of scope / non-goals

- Add/remove UX — **D03**; amount rendering in signing — **I07**.

## 9. Dependencies, risks & open questions

- **Risk:** an out-of-date KNOWN_TOKENS entry — keep it as the sole authority and correct there.
- **Open question:** None.

## 10. Source anchors

- `src/services/token-metadata.ts`, `src/services/tokens.ts` (`KNOWN_TOKENS`), `src/services/abi.ts:268-307`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 78, 100; clue 69 (decimals-as-security).
