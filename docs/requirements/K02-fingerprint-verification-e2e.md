# K02 · MITM-Resistant Fingerprint Verification & E2E Badge

| | |
|---|---|
| **Epic** | K — dApp Connect (WalletPair) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | K01 |
| **Related** | K03, K08 |

## 1. Summary

Before joining the encrypted channel, pairing requires **visual fingerprint confirmation** —
out-of-band verification that both sides see the same short fingerprint, defeating a man-in-the-middle
on the relay. Once joined, the connected card shows a green **"E2E" lock badge** so the user has a
persistent trust signal.

## 2. Background & context

A relay could attempt a MITM by pairing with each side separately. Comparing a human-checkable
fingerprint out-of-band (on both screens) ensures the two endpoints share the same key, not an
interposed one. The E2E badge keeps that assurance visible.

## 3. Users & stories

- As a **user**, I want to confirm I'm connected to the real dApp, so that a relay can't MITM me.
- As a **user**, I want an ongoing E2E indicator, so that I trust the live session.

## 4. Functional requirements

- **FR-1** — Present a fingerprint on both the wallet and dApp side; require the user to confirm they match before joining.
- **FR-2** — Refuse to complete the session if the fingerprint isn't confirmed.
- **FR-3** — Show a green **"E2E" lock badge** on the connected card while the encrypted session is active.
- **FR-4** — Bound the join with a deadline (`CONFIRM_JOIN`, K08) so a stalled pairing doesn't hang.

## 5. Non-functional requirements

- **NFR-1** — Fingerprint is short enough to compare but strong enough to resist collision.
- **NFR-2** — The E2E badge reflects real encryption state, not a static label.

## 6. UX / flow notes

`ConnectionFlowStates` drives the pairing UI: fingerprint compare → confirm → connected (E2E badge). A mismatch is a clear, safe abort.

## 7. Acceptance criteria

- [ ] **AC-1** — Joining requires an explicit fingerprint-match confirmation.
- [ ] **AC-2** — A declined/mismatched fingerprint aborts the pairing.
- [ ] **AC-3** — The connected card shows the E2E badge while active.

## 8. Out of scope / non-goals

- Pairing transport — **K01**; session lifecycle — **K03**; resilience deadlines — **K08**.

## 9. Dependencies, risks & open questions

- **Risk:** users clicking through without comparing — copy must make the compare step meaningful.
- **Open question:** None.

## 10. Source anchors

- `src/components/ConnectionFlowStates.tsx`, `src/models/dapp-connection.tsx`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 90.
