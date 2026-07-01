# D03 · Token List & Add/Remove Custom Tokens

| | |
|---|---|
| **Epic** | D — Balances, Portfolio & Activity |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | D04 |
| **Related** | D02, D06, E01, H01 |

## 1. Summary

Users see a per-account token list and can **add custom ERC-20 tokens** by address (with metadata
resolved on-chain, D04) and remove them. Added tokens join the portfolio aggregation (D02), the send
token selector (H01), and the received-transfer allowlist (D05).

## 2. Background & context

Vela curates well-known tokens (D04's KNOWN_TOKENS) but users hold long-tail tokens too. Adding by
address, with trustworthy on-chain metadata, lets the wallet show any ERC-20 without a central token
list — and the allowlist coupling (D05) means only known + user-added tokens surface as deposits, so
airdrop spam can't inject itself.

## 3. Users & stories

- As a **user**, I want to add a token by its address, so that I can see and send a token Vela doesn't ship.
- As a **user**, I want to remove tokens I don't care about, so that my list stays clean.

## 4. Functional requirements

- **FR-1** — Add a custom token by contract address; resolve `symbol`/`decimals` on-chain (D04), never assuming 18.
- **FR-2** — Persist custom tokens per account/chain (A06).
- **FR-3** — Custom tokens participate in portfolio aggregation (D02), the send selector (H01), and the deposit allowlist (D05).
- **FR-4** — Allow removal; removal drops the token from lists but not on-chain balance.
- **FR-5** — Guard against duplicates and invalid addresses.

## 5. Non-functional requirements

- **NFR-1** — Metadata resolution is cached (D04) to avoid repeated on-chain reads.
- **NFR-2** — Unknown 6-decimal tokens render correctly (never "+0 tokens" — the bug D04 fixes).

## 6. UX / flow notes

`AddTokenScreen` / `AddTokenSheet` accept an address, preview resolved symbol/decimals, and confirm. Token rows show logo (fallback to generated), symbol, balance, fiat value (E01/E06).

## 7. Acceptance criteria

- [ ] **AC-1** — Adding a valid ERC-20 by address shows correct symbol/decimals and balance.
- [ ] **AC-2** — A 6-decimal token displays its real amount, not an inflated/zero value.
- [ ] **AC-3** — Removing a token hides it without affecting other tokens.

## 8. Out of scope / non-goals

- Metadata resolution internals — **D04**; deposit detection — **D05**.

## 9. Dependencies, risks & open questions

- **Risk:** malicious token metadata (fake symbol) — treated with caution; value still derived on-chain (E01).
- **Open question:** None.

## 10. Source anchors

- `src/screens/wallet/AddTokenScreen.tsx`, `src/components/ui/AddTokenPanel.tsx`, `src/services/tokens.ts`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 78, 100.
