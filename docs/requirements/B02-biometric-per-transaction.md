# B02 · Biometric-Per-Transaction (No Persistent Unlock Session)

| | |
|---|---|
| **Epic** | B — Identity: Passkeys & Account Model |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | B01 |
| **Related** | G02, H01, K05, J05 |

## 1. Summary

**Every transaction and message signature requires a fresh biometric verification** (Face ID /
fingerprint). There is no "unlock once, sign freely" session. Each signing operation invokes the
passkey credential anew, so approval is always an explicit, present-moment act.

## 2. Background & context

Session-based unlock is convenient but dangerous: a left-open app can be drained. Because the passkey
*is* the signer, requiring it per signature costs one biometric tap but removes the "authorized once,
abused later" window. This pairs with the never-unlimited-approval guard (J05) to bound exposure.

## 3. Users & stories

- As a **user**, I want each signature to require my biometric, so that nothing signs without my live consent.
- As a **user who steps away**, I want no lingering unlocked state, so that a picked-up phone can't spend.

## 4. Functional requirements

- **FR-1** — Producing any signature (send G02, dApp tx/message K05, batch K07) triggers a WebAuthn `get()` requiring user verification.
- **FR-2** — No global "unlock" toggle grants signing beyond a single operation.
- **FR-3** — A cancelled/failed biometric aborts that operation cleanly with no partial submission.
- **FR-4** — Read-only actions (balances, decoding, simulation) do **not** require biometric — only signatures do.

## 5. Non-functional requirements

- **NFR-1** — The biometric prompt is the last gate before signing; nothing is broadcast before it succeeds.
- **NFR-2** — Behavior is consistent across platforms; web uses the same WebAuthn `get()` (via B03/B04).

## 6. UX / flow notes

The signing sheet (I01) presents the decoded intent; confirming triggers the OS biometric. Slide-to-confirm (M04) precedes the biometric on high-consequence actions.

## 7. Acceptance criteria

- [ ] **AC-1** — Two consecutive sends each prompt for biometric independently.
- [ ] **AC-2** — Cancelling the biometric aborts with no tx submitted.
- [ ] **AC-3** — Viewing balances/simulations never prompts for biometric.

## 8. Out of scope / non-goals

- Signature encoding — **G02**; approval limits — **J05**.

## 9. Dependencies, risks & open questions

- **Risk:** platform quirks in re-prompting; must not silently cache a WebAuthn assertion for reuse.
- **Open question:** None.

## 10. Source anchors

- `src/hooks/use-dapp-signing.ts` — per-request signing path.
- `src/modules/passkey/index.ts` — WebAuthn `get()` per signature.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 5.
