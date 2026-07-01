# H01 · Send Flow & Max-Send (Recipient → Token → Amount → Review)

| | |
|---|---|
| **Epic** | H — Send & Receive |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | G01, G03 |
| **Related** | H02, H03, H04, E07, G05, I01 |

## 1. Summary

The core send flow: choose a **recipient** (address, contact, name, or QR), a **token**, and an
**amount** (token or fiat, E07), then review a clearly-decoded confirmation (I01) before a single
biometric signature (B02). **Max-send reserves native gas** (the EntryPoint prefund) so a "send
everything" can never revert for lack of gas.

## 2. Background & context

Send is the wallet's central job and must be both easy and safe. It composes identity resolution (H02),
risk checks (H03), clear-signing (I01), and the 4337 engine (G01). Max-send is subtle: sending the full
native balance must leave enough for gas, or the op fails (AA21-class errors).

## 3. Users & stories

- As a **user**, I want to send a token to someone in a few taps, so that paying is quick.
- As a **user**, I want "max" to send as much as possible while still paying gas, so that it doesn't revert.
- As a **user**, I want a clear review before signing, so that I know exactly what I'm doing.

## 4. Functional requirements

- **FR-1** — Recipient entry accepts raw address, saved contact (H05), resolved name (H02), or QR/EIP-681 (H04); show risk signals (H03).
- **FR-2** — Token selection from the account's tokens (D03); amount in token or fiat (E07).
- **FR-3** — **Max-send** reserves native gas (fee model G05) so the remaining sendable amount can't underfund gas.
- **FR-4** — Review step decodes the transaction via clear-signing (I01) and shows the split fee (G05).
- **FR-5** — Confirm requires a fresh biometric (B02); submit builds/sends a UserOp (G01) and writes a pending entry (D08).

## 5. Non-functional requirements

- **NFR-1** — Amount math is BigInt/base-unit correct at the token's decimals (D04).
- **NFR-2** — Errors (insufficient balance, doomed estimate G03, too-high quote G05) surface as clear, retryable messages.

## 6. UX / flow notes

`SendScreen` → recipient → token → amount → review (`GasFeeCard`, I01 sheet) → biometric. Known-contact badge (H05) and risk banners (H03) appear inline. Native gas reservation is transparent on "Max."

## 7. Acceptance criteria

- [ ] **AC-1** — A standard token send completes end-to-end and appears as pending then confirmed (D08/G09).
- [ ] **AC-2** — Max-send of the native coin leaves enough for gas and does not revert.
- [ ] **AC-3** — The review step shows the decoded intent and split fee before the biometric.

## 8. Out of scope / non-goals

- Identity resolution — **H02**; risk — **H03**; QR/EIP-681 — **H04**; batch/split/sweep — **H07/H08**.

## 9. Dependencies, risks & open questions

- **Risk:** stale price for fiat entry at submit (E07) — re-check at confirm.
- **Open question:** None.

## 10. Source anchors

- `src/screens/wallet/SendScreen.tsx`, `src/services/safe-transaction.ts` (send path), `src/components/ui/GasFeeCard.tsx`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 17, 26, 27.
