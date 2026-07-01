<!--
Standard Vela requirement doc (PRD). Target length ~60–90 lines. Keep every functional
claim tied to a source anchor. Delete these HTML comments when filling in.
Filename: <ID>-<kebab-slug>.md   e.g. G04-gas-price-oracle-tiers-parity.md
-->

# <ID> · <Title>

| | |
|---|---|
| **Epic** | <Letter — Epic name> |
| **Status** | ✅ Shipped / 🚧 In progress / 🔜 Next / 🧭 Exploring |
| **Owner** | Shelchin |
| **Depends on** | <IDs, or "—"> |
| **Related** | <IDs> |

## 1. Summary

<2–4 sentences: what this requirement delivers and why it exists. Lead with the user-
or security-facing outcome, not the implementation.>

## 2. Background & context

<Why this is needed. The problem, the constraint, or the prior failure that motivated it.
Cite the guardrail or design decision if relevant.>

## 3. Users & stories

- As a **<persona>**, I want **<capability>** so that **<outcome>**.
- As a **<persona>**, I want **<capability>** so that **<outcome>**.

## 4. Functional requirements

- **FR-1** — <testable behavior>.
- **FR-2** — <testable behavior>.
- **FR-3** — <edge case / failure behavior>.

## 5. Non-functional requirements

- **NFR-1** — <performance / security / reliability / privacy / i18n / a11y bound>.
- **NFR-2** — <…>.

## 6. UX / flow notes

<Key screens, states (loading / empty / error / degraded), copy constraints, haptics.
Skip for pure-infra docs, or note "No direct UI — consumed by <IDs>.">

## 7. Acceptance criteria

- [ ] **AC-1** — <observable pass/fail condition>.
- [ ] **AC-2** — <…>.
- [ ] **AC-3** — <negative / abuse case handled>.

## 8. Out of scope / non-goals

- <what this doc explicitly does NOT cover, with the ID that does if applicable>.

## 9. Dependencies, risks & open questions

- **Depends on:** <upstream IDs / services / contracts>.
- **Risk:** <what could break; cross-repo coupling; stale-fact traps>.
- **Open question:** <unresolved decision, or "None">.

## 10. Source anchors

- `path/to/file.ts:line` — <what lives there>.
- `docs/…` / clue #NN — <fact source>.
