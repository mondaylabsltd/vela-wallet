# H05 · Contacts (On-Device Address Book) & Known-Contact Badge

| | |
|---|---|
| **Epic** | H — Send & Receive |
| **Status** | 🚧 In progress (branch `feat/contacts-groups-payroll-batch`) |
| **Owner** | Shelchin |
| **Depends on** | A06 |
| **Related** | H06, H01, H03, H02 |

## 1. Summary

An **on-device address book**: users save contacts (address, name, avatar, kind, favorite) and see a
**"known contact" badge** during send/signing, reducing wrong-address risk. Contacts are stored locally
(A06), can be derived from prior sends, and feed the contact picker, groups (H06), and the payroll
importer (H08). Cross-device sync of the address book is a 🔜 roadmap item.

## 2. Background & context

Re-pasting addresses is error-prone and a poisoning vector (H03). A local, private address book with a
visible "known" signal makes repeat payments safer and faster — without a server (A03).

## 3. Users & stories

- As a **user**, I want to save people I pay, so that I don't re-paste addresses.
- As a **user**, I want a badge showing a recipient is a known contact, so that I trust the address.

## 4. Functional requirements

- **FR-1** — Save/edit/delete contacts with `{address, name, avatar, kind: 'eoa'|'account'|'unknown', favorite}`; addresses stored lowercased.
- **FR-2** — Persist contacts locally (A06); expose `getAllContacts`, `isSavedContact`, `toggleFavorite`.
- **FR-3** — Show a **known-contact badge** in send (H01) and signing (I01) when the recipient is saved.
- **FR-4** — Derive candidate contacts from prior **send** transactions (transfers only, never dApp contract calls).
- **FR-5** — Deleting a contact **cascades**: remove it from every group (H06) so no member dangles.

## 5. Non-functional requirements

- **NFR-1** — In-memory cache in front of AsyncStorage for fast lookups (A06).
- **NFR-2** — Address book is private/on-device; sync is a separate 🔜 feature.

## 6. UX / flow notes

`ContactsManager` / `ContactPicker` / `ContactAvatar`. Badge coexists with risk info (H03) — "known" doesn't suppress a contract/first-interaction caution.

## 7. Acceptance criteria

- [ ] **AC-1** — A saved contact shows the known-contact badge in send and signing.
- [ ] **AC-2** — Deleting a contact removes it from all groups (cascade).
- [ ] **AC-3** — Prior sends surface as suggested contacts; dApp calls do not.

## 8. Out of scope / non-goals

- Groups — **H06**; payroll importer — **H08**; cross-device sync — 🔜.

## 9. Dependencies, risks & open questions

- **Risk:** a "known" badge could over-reassure — keep risk checks (H03) always visible.
- **Open question:** conflict resolution once cross-device sync lands.

## 10. Source anchors

- `src/services/contacts.ts:25` (`Contact`), `:211` (delete + group cascade), `:263` (`getAllContacts`).
- `src/components/contacts/ContactsManager.tsx`, `ContactPicker.tsx`, `ContactAvatar.tsx`.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 99.
