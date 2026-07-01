# I01 · Clear-Signing Model (Intent / Substance / Details, Risk Color)

| | |
|---|---|
| **Epic** | I — Clear Signing & Decoding |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | — |
| **Related** | I02–I08, J01, K05, K06 |

## 1. Summary

Vela decodes both **calldata** and **EIP-712 typed data** into human-readable **Intent / Substance /
Details**, color-coded by risk — replacing blind signing. Undecodable calls get an **explicit
blind-sign warning** instead of a fabricated summary. This model is the single render path for every
signing request (send, dApp tx, message) and its read-only replay (L03).

## 2. Background & context

Blind signing (raw hex) is how users get drained by requests they can't read. Clear signing (ERC-7730
lineage) makes the *meaning* of a transaction legible. Crucially, when Vela can't decode something, it
says so — a fake summary would be worse than honest uncertainty.

## 3. Users & stories

- As a **user**, I want to see what a transaction actually does in plain language, so that I sign with understanding.
- As a **user**, I want an honest warning when something can't be decoded, so that I'm not lulled by a fake summary.

## 4. Functional requirements

- **FR-1** — Render every signing request as **Intent** (what it does), **Substance** (key amounts/recipients), **Details** (advanced/raw), color-coded by risk (I08).
- **FR-2** — Decode both calldata (`eth_sendTransaction`) and EIP-712 typed data (I06).
- **FR-3** — When decoding fails at all layers (I02), show an **explicit blind-sign warning**, never a fabricated summary.
- **FR-4** — Raw params live under "Advanced — view raw data," not the default view.
- **FR-5** — The same `<SigningSheet>` renders production requests **and** read-only history replay (L03) and the harness scenarios (`clear-signing-test`).

## 5. Non-functional requirements

- **NFR-1** — A single render path (prod + harness + replay) so behavior can't diverge.
- **NFR-2** — i18n key depth ≤ 3 segments (a known gotcha); all copy localized (M05).

## 6. UX / flow notes

`SigningRequestModal` / `SigningSheet`. Risk color follows I08. Balance-change preview (J02) sits alongside the decoded intent. Slide-to-confirm (M04) + biometric (B02) gate the signature.

## 7. Acceptance criteria

- [ ] **AC-1** — A known token transfer renders a plain intent + amount + recipient.
- [ ] **AC-2** — An undecodable call shows a blind-sign warning, not a fake summary.
- [ ] **AC-3** — The same sheet renders a past signature in read-only replay (L03).

## 8. Out of scope / non-goals

- Descriptor cascade — **I02**; decoding internals — **I04–I07**; risk math — **I08**; simulation — **J01/J02**.

## 9. Dependencies, risks & open questions

- **Risk:** i18n keys deeper than 3 segments break (project gotcha) — keep keys shallow.
- **Open question:** wider decode coverage is 🚧 (I02).

## 10. Source anchors

- `src/services/clear-signing.ts`, `src/components/SigningRequestModal.tsx`, `src/components/signing/`.
- memory `project_clear_signing_rebuild`; `docs/clear-signing-design.md`; `docs/CONTENT-SOURCE-100-CLUES.md` — clue 12.
