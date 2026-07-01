# I08 · Layered Risk Scoring (Floor Uncertainty at Caution)

| | |
|---|---|
| **Epic** | I — Clear Signing & Decoding |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | I01 |
| **Related** | I02, I04, I05, I07, J05 |

## 1. Summary

Risk color is computed in **layers** with a golden rule: **uncertainty can never read as "safe."** Any
warning field (e.g. unlimited approval) → **danger**; an intent base (approve/permit → caution;
stake/deposit/claim → safe); then any partial / unverified / expired field **floors** a safe/normal
result at **caution**. Deploy/permit/expiry edge cases render calmly, not as scary "Unknown."

## 2. Background & context

A single wrong "green/safe" on a malicious request is catastrophic. Layered scoring ensures the most
dangerous signal dominates and that missing information degrades to caution — the opposite of
optimistic defaults. This is what makes clear signing safe rather than merely pretty.

## 3. Users & stories

- As a **user**, I want the riskiest aspect of a transaction to set the color, so that danger is never hidden by a benign label.
- As a **user**, I want "we're not sure" to look cautious, not safe, so that uncertainty prompts a second look.

## 4. Functional requirements

- **FR-1** — Any **warning field** (e.g. unbounded approval, J05) → **danger**, overriding everything.
- **FR-2** — Intent base risk: approve/permit → **caution**; stake/deposit/claim → **safe**.
- **FR-3** — Any **partial / unverified / expired** field (I04 best-effort, I05 unresolved standard, I07 unverified decimals) **floors** a safe/normal result at **caution**.
- **FR-4** — Deploy shows "Deploy contract" (calm), Permit2 "no expiry" sentinels are omitted (not "Invalid Date"), past deadlines render `(expired)` — never a scary generic "Unknown."
- **FR-5** — The final risk drives the signing sheet color (I01).

## 5. Non-functional requirements

- **NFR-1** — Monotonic toward danger: adding a risky field can only worsen, never improve, the score.
- **NFR-2** — Deterministic and testable via the clear-signing scenarios harness.

## 6. UX / flow notes

Color-coded Intent/Substance/Details (I01). Danger requires an extra-deliberate confirm; caution informs. No false "safe."

## 7. Acceptance criteria

- [ ] **AC-1** — An unlimited approval renders danger regardless of other fields.
- [ ] **AC-2** — A best-effort/unverified decode floors at caution, never safe.
- [ ] **AC-3** — A contract deploy renders "Deploy contract" calmly, not "Unknown."

## 8. Out of scope / non-goals

- Approval guard enforcement — **J05**; decoding layers — **I02–I07**.

## 9. Dependencies, risks & open questions

- **Risk:** mis-weighted intents; keep stake/deposit/claim "safe" only when fully verified.
- **Open question:** None.

## 10. Source anchors

- `src/services/clear-signing.ts:1236` (risk layering), `:128` (deploy/permit/expiry rendering).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 71, 74.
