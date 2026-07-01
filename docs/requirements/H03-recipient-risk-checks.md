# H03 · Recipient Risk Checks (Address-Poisoning Defense)

| | |
|---|---|
| **Epic** | H — Send & Receive |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F03 |
| **Related** | H01, H02, J05 |

## 1. Summary

Before a send, Vela runs two **cheap on-chain signals**: **first-interaction** (no prior outgoing tx to
this address → counters look-alike **address-poisoning**) and **is-contract** (`eth_getCode` → catches
sending to a token contract itself). Both are **best-effort**: an unreachable RPC produces **no false
alarm**.

## 2. Background & context

Address poisoning seeds a user's history with a look-alike address hoping they'll copy it later.
Flagging first-time recipients and contract addresses catches the two most common wrong-address mistakes
without a third-party service or heavy analysis.

## 3. Users & stories

- As a **user**, I want a warning when I'm about to send to a brand-new address, so that I double-check for poisoning.
- As a **user**, I want to be warned if I'm sending to a token contract, so that I don't burn funds.

## 4. Functional requirements

- **FR-1** — **First-interaction check:** flag if there's no prior outgoing tx from this account to the recipient.
- **FR-2** — **Is-contract check:** `eth_getCode` on the recipient; flag if it's a contract (esp. a token contract).
- **FR-3** — Both checks are **best-effort**: an RPC failure yields **no** warning (no false alarm), not a block.
- **FR-4** — Present warnings inline in the send flow (H01); they inform, they don't hard-block.

## 5. Non-functional requirements

- **NFR-1** — Cheap (single reads over the pool F03); must not slow the send meaningfully.
- **NFR-2** — A resolved name (H02) does **not** suppress these warnings.

## 6. UX / flow notes

`RecipientTrust` shows a "first time sending here" or "this is a contract" caution. Known contacts (H05) get a positive "known contact" badge that coexists with risk info.

## 7. Acceptance criteria

- [ ] **AC-1** — Sending to a never-before-used address shows the first-interaction caution.
- [ ] **AC-2** — Sending to a contract address shows the is-contract caution.
- [ ] **AC-3** — With RPC unreachable, no false warning appears.

## 8. Out of scope / non-goals

- Name resolution — **H02**; approval safety — **J05**.

## 9. Dependencies, risks & open questions

- **Risk:** legitimate first sends are common — warnings must inform, not nag/block.
- **Open question:** whether to escalate for exact-prefix/suffix look-alikes of known addresses.

## 10. Source anchors

- `src/services/recipient-risk.ts` — first-interaction + is-contract, best-effort.
- `src/components/contacts/RecipientTrust.tsx`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 27.
