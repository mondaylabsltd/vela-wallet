# A02 · Brand, Voice, Honesty & Alpha/Audit Posture

| | |
|---|---|
| **Epic** | A — Product Foundations & Cross-Cutting |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A01 |
| **Related** | A03, N01, N04, O04 |

## 1. Summary

Vela's brand stance is **candor over hype**: honest about trade-offs, honest about alpha status,
honest about the audit gap. This doc codifies the verbatim taglines, the voice constraints, and the
**mandated audit/alpha phrasing** so that no screen, doc, or store listing drifts into false comfort or
false alarm.

## 2. Background & context

Trust is the product's core differentiator ("we architecturally can't access your keys"). Scary
"Beta — tolerate bugs" banners undercut that trust; so does over-promising an audit that isn't
scheduled. The brand is a solo founder building fully in the open, which is itself a trust signal.

## 3. Users & stories

- As a **skeptical evaluator**, I want honest statements of limits, so that I can trust the confident claims.
- As a **new user in alpha**, I want status stated plainly without fear-mongering, so that I feel informed, not scared off.

## 4. Functional requirements

- **FR-1** — Approved taglines used **verbatim**: "Your keys. Your face." · "A wallet that does less — on purpose." · "We can't access your keys. Not 'we promise not to' — we architecturally can't." · "You're paying for convenience, not access." · "Don't trust us — verify." No improvised alternatives.
- **FR-2** — **Audit phrasing (mandatory):** Safe contracts are independently audited; **Vela's own integration is NOT audited and none is scheduled** — "a goal for when the project can fund one, not a commitment with a date." Never write "audit planned/coming."
- **FR-3** — **Alpha stated honestly** ("alpha · v0.1") — **no** "tolerate bugs / beta" disclaimer banners. Trust > disclaimers.
- **FR-4** — Canonical URLs are `getvela.app` (site) and `wallet.getvela.app` (wallet) only — treated as an anti-phishing fact.
- **FR-5** — "No token, ever" and "no tracking" messaging aligns with A03.

## 5. Non-functional requirements

- **NFR-1** — Voice: plain, specific, "we write down what, and why." No FOMO, no scarcity, no speculation hooks.
- **NFR-2** — All 15 locales (M05) must preserve the honesty framing, not soften or dramatize it.

## 6. UX / flow notes

Feedback is a quiet Settings row → prefilled GitHub bug report (N04), not a nag. About screen carries the founder/entity facts (O04). No modal fear banners anywhere.

## 7. Acceptance criteria

- [ ] **AC-1** — No screen/doc contains "audit planned/coming" or a "tolerate bugs" banner.
- [ ] **AC-2** — Taglines appear verbatim; no invented variants exist in the app or site.
- [ ] **AC-3** — Only the two canonical domains appear in official surfaces.

## 8. Out of scope / non-goals

- Marketing content generation — governed by `docs/CONTENT-SOURCE-100-CLUES.md`, not this PRD.

## 9. Dependencies, risks & open questions

- **Risk:** a well-meaning contributor adds a "Beta" banner or an audit promise — CI/review must catch phrasing drift.
- **Open question:** exact wording for an eventual audit *once funded* (out of scope until then).

## 10. Source anchors

- `getvela.app/src/content/docs/whitepaper.md`, `getvela.app/src/content/blog/vela-is-in-alpha.md`.
- `docs/store-submission/privacy-and-review.md`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — guardrails + clues 2, 9, 10, 16, 93, 94.
