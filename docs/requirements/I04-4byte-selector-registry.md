# I04 · 4-Byte Selector Registry (3 DBs Merged)

| | |
|---|---|
| **Epic** | I — Clear Signing & Decoding |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | I01 |
| **Related** | I02, I06, I08 |

## 1. Summary

The **last-resort** decode layer merges **three 4-byte databases**: Sourcify 4byte + OpenChain
(spam-filtered, most-likely-first) lead, then `4byte.directory` (canonical order) fills gaps, with a 6s
timeout. Unknown calls decode as `bestEffort: true` with `risk: 'caution'`, and raw params are hidden
under "Advanced — view raw data."

## 2. Background & context

When no descriptor or standard method matches (I02), a 4-byte signature lookup still gives a plausible
function name. Merging three sources with spam-filtering improves the odds of a useful, non-misleading
guess — while clearly marking it as best-effort so it never reads as authoritative.

## 3. Users & stories

- As a **user on an unknown contract**, I want at least a best-effort function name, so that I have some context.
- As a **user**, I want best-effort decodes clearly marked as uncertain, so that I don't over-trust them.

## 4. Functional requirements

- **FR-1** — Merge Sourcify 4byte + OpenChain (spam-filtered, most-likely-first) as the lead source; fill gaps with `4byte.directory` (canonical order).
- **FR-2** — Apply a **6s timeout**; degrade to raw/blind (I01) if lookups don't resolve.
- **FR-3** — Mark results `bestEffort: true`, `risk: 'caution'` (floors risk at caution, I08).
- **FR-4** — Hide raw params under "Advanced — view raw data" (I01).

## 5. Non-functional requirements

- **NFR-1** — Lookups are bounded (6s) and don't block signing indefinitely.
- **NFR-2** — Best-effort never elevates to a confident/"safe" rendering.

## 6. UX / flow notes

Best-effort decodes show a caution treatment; the user can expand raw data. Never presented as a verified intent.

## 7. Acceptance criteria

- [ ] **AC-1** — A known 4-byte selector resolves to a plausible function name marked best-effort.
- [ ] **AC-2** — Lookups exceeding 6s degrade to raw/blind, not a hang.
- [ ] **AC-3** — Best-effort results carry `risk: 'caution'`.

## 8. Out of scope / non-goals

- Cascade — **I02**; ABI param decoding — **I06**; risk math — **I08**.

## 9. Dependencies, risks & open questions

- **Risk:** collisions/ambiguous selectors — spam-filter + most-likely-first mitigate; caution floor bounds trust.
- **Open question:** offline caching of frequent selectors.

## 10. Source anchors

- `src/services/selector-registry.ts` — 3-DB merge + timeout + best-effort marking.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 67.
