# H06 · Contact Groups (e.g. Payroll)

| | |
|---|---|
| **Epic** | H — Send & Receive |
| **Status** | 🚧 In progress (branch `feat/contacts-groups-payroll-batch`) |
| **Owner** | Shelchin |
| **Depends on** | H05 |
| **Related** | H08, H07 |

## 1. Summary

A **contact group** (e.g. "Payroll") is a **named set of contacts** — a first-class registry object, not
an ad-hoc selection. A whole group can be picked as the recipient set for a batch/split send (H07) or
the payroll importer (H08). Groups store lowercased member addresses; deleting a contact cascades out of
every group (H05).

## 2. Background & context

Recurring multi-recipient payments (salaries, grants, reimbursements) need a reusable list. Modeling a
group as a first-class object with a stable id lets it be selected atomically and edited over time,
rather than re-picking members each payday.

## 3. Users & stories

- As a **team lead**, I want a "Payroll" group, so that I can pay everyone by selecting one group.
- As a **user**, I want groups to stay consistent when I edit contacts, so that members never dangle.

## 4. Functional requirements

- **FR-1** — `ContactGroup { id, name, members: string[] (lowercased), color? }` persisted under `vela.contactGroups` (A06).
- **FR-2** — Create/update via `saveGroup`; **new groups get a stable, collision-free id** (one past the largest numeric suffix).
- **FR-3** — List groups via `getGroups` returning **copies** (safe for caller mutation).
- **FR-4** — Optional accent hue per group (a `color.*` token name or hex) for its chip.
- **FR-5** — Deleting a contact (H05) removes it from every group's `members` (cascade).
- **FR-6** — A group can be selected as the recipient set for split send (H07) and the payroll importer (H08).

## 5. Non-functional requirements

- **NFR-1** — Group operations are consistent with the contacts cache (H05); no orphaned members.
- **NFR-2** — Ids are stable and collision-free across edits.

## 6. UX / flow notes

Group chips (with optional accent color) in the contact picker; selecting a group prefills recipients for H07/H08.

## 7. Acceptance criteria

- [ ] **AC-1** — Creating a group assigns a stable, unique id.
- [ ] **AC-2** — Selecting a group as recipients prefills all members for a batch send.
- [ ] **AC-3** — Deleting a member contact removes it from the group.

## 8. Out of scope / non-goals

- Contacts CRUD — **H05**; batch execution — **H07**; table import — **H08**.

## 9. Dependencies, risks & open questions

- **Risk:** large groups vs MultiSend gas ceiling — bound at execution (H07/H08).
- **Open question:** group-level metadata (default token/amount) — not yet.

## 10. Source anchors

- `src/services/contacts.ts:52` (`ContactGroup`), `:63` (`GROUPS_KEY`), `:159` (`nextGroupId`), `:322` (`getGroups`), `:337` (`saveGroup`).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 98, 99.
