# O02 · Engineering Rules & AI-Coding Accountability (`agent-rules/`)

| | |
|---|---|
| **Epic** | O — Ops, Testing, Store & Meta |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | — |
| **Related** | O01, B06, B07, A02 |

## 1. Summary

Vela codifies its engineering process in `agent-rules/`: **AI-coding accountability** ("AI can produce
code, but not accountability") with a **Low/Medium/High risk workflow** where any crypto/key/auth change
is **auto-High**; a red-team security-audit prompt ("prove this code is unsafe until you can't"); a
pre-launch hardening rulebook; and a build→test→fix loop with **30-cycle stability verification**. This
substantiates "built carefully, in the open."

## 2. Background & context

A solo founder using AI assistance needs guardrails so that speed never compromises the crypto surface.
Auto-High for any key/auth/crypto change forces the highest scrutiny exactly where a bug strands funds
(B06/B07). The rules are themselves open source — part of "verify, don't trust" (A02).

## 3. Users & stories

- As a **maintainer**, I want any crypto/auth change to trigger the strictest review, so that high-risk edits can't slip through.
- As an **evaluator**, I want the engineering process visible, so that I trust how the wallet is built.

## 4. Functional requirements

- **FR-1** — Classify changes Low/Medium/High; **auto-High** for any crypto/key/auth change.
- **FR-2** — Apply the red-team audit prompt ("prove this code is unsafe until you can't") to High-risk changes.
- **FR-3** — Follow the pre-launch hardening rulebook before releases (O03).
- **FR-4** — Run the build→test→fix loop with **30-cycle stability verification** for stability-critical work.
- **FR-5** — Rules live in-repo (`agent-rules/`) and are open for inspection.

## 5. Non-functional requirements

- **NFR-1** — Address-derivation / signing changes must pass golden vectors (B06/B07) as a gate.
- **NFR-2** — The process is documented and repeatable, not tribal knowledge.

## 6. UX / flow notes

No user-facing UI; this is process. It underpins the "audited Safe + careful integration" and honest-audit-posture messaging (A02).

## 7. Acceptance criteria

- [ ] **AC-1** — A change touching crypto/key/auth is treated as High-risk with red-team review.
- [ ] **AC-2** — Address/signing changes are gated by golden vectors.
- [ ] **AC-3** — Stability-critical changes pass the 30-cycle loop.

## 8. Out of scope / non-goals

- Store submission — **O03**; test environment — **O01**.

## 9. Dependencies, risks & open questions

- **Risk:** process only helps if followed — keep it lightweight enough to actually use.
- **Open question:** None.

## 10. Source anchors

- `agent-rules/AI-CODING-RULES.md`, `SECURITY-AUDIT.md`, `LAUNCH_AUDIT.md`, `CLAUDE-AUTO-TEST.md`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 92.
