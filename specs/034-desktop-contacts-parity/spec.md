# Feature Specification: Desktop Contacts & Accounts Parity — the facts the desktop never asked for

**Feature Branch**: `034-desktop-contacts-parity` (stacked on `033-desktop-send-parity`)

**Created**: 2026-09-09

**Status**: Draft

**Input**: Founder direction (2026-09-09): whatever the web can do, native should
do too; new spec numbers, stacked branches, stop at 039.

## Why this is not the signing spec it was going to be

033's handover named 034 as "the signing cluster: transaction simulation and
recipient risk". The reachability check retired the first half before a line was
written, and the evidence is worth keeping:

- The web **runs** `eth_simulateV1`, judges the deltas through `token_trust`,
  and writes the result into `SendView::sim_json` and onto the stored signing
  record as `assetChanges`. **Nothing reads either back.** `kind: 'balances'`
  — the block that would draw them — appears only in `signing/fixtures.ts`, on
  the web exactly as it appears only in `signing/fixtures.rs` here.
- So the balance-change preview is drawn in two galleries and wired in neither.
  Building it on the desktop is a new feature (and a good one — it is the one
  part of a signing sheet a malicious site cannot author), not parity, and it
  needs a simulation engine the desktop does not have.

The desktop's own comment on that operation is already the honest reading:
*"No simulation engine on the desktop yet: the confirm surface stays empty,
which is the honest reading of best effort."* It stays true.

**Recipient risk survives the check, and it is not the send screen's.** The
desktop's `send` executor has resolved risk since 032. What it never does is
ask the CONTACTS machine to inspect an address — the web does, every time a
contact is opened.

## Scope

| # | What | Core event / view fields the desktop does not read |
|---|---|---|
| 1 | **Inspect a contact when it opens** | `contacts::InspectRecipient`; `ContactDetailView::identity` `is_contract` `first_interaction` |
| 2 | **The account switcher's per-account totals** | `balance_dashboard::SwitcherOpened` / `SwitcherClosed`; `switcher` view |
| 3 | **Setting a contact's groups / a group's members as a set** | `contacts::SetContactGroups` `SetGroupMembers` |

## Out of Scope

- Transaction simulation and the balance-change block (retired above; a new
  feature for both shells, and it needs an engine).
- The per-leg batch approval editor (`GuardView.batch`) — drawn nowhere.
- Any change to a machine's rules under `rust/crates/vela-core/src/app/`.
- New corpus keys.

## User Scenarios & Testing

### User Story 1 — Opening a contact says what it is (Priority: P1)

A person opening an address book entry is told whether that address is a
contract or a person, whether they have ever paid it before, and where its name
came from.

**Independent Test**: opening a contact dispatches `InspectRecipient`; the
detail panel renders `is_contract` and `first_interaction` from the view, and
`None` renders as nothing — never as a false alarm.

**Acceptance Scenarios**:

1. **Given** an address never paid before, **When** its contact opens, **Then**
   the first-interaction tell is shown, because that is the address-poisoning
   signal and it costs nothing to state.
2. **Given** the chain could not be reached, **Then** `is_contract: None`
   renders as silence — the core's invariant ⑦ is "never a false alarm", and a
   shell that guessed would be the alarm.

### User Story 2 — The switcher shows what each account holds (Priority: P2)

Opening the account list shows each account's total, not just its address.

**Independent Test**: opening the accounts panel dispatches `SwitcherOpened`
with every listed address; each row's figure comes from the core's switcher
view, and closing dispatches `SwitcherClosed`.

## Requirements

- **FR-341**: the shell asks; the core answers. No risk verdict, no total and
  no identity is computed here.
- **FR-342**: `None` is silence, never a warning.
- **FR-343**: zero new corpus keys.
- **FR-344**: no machine changes under `rust/crates/vela-core/src/app/`.

## Success Criteria

- **SC-341**: opening a contact shows its classification and its
  first-interaction tell, and an unreachable chain shows neither.
- **SC-342**: the accounts panel shows a per-account figure that the core
  produced, and stops asking for them when it closes.
- **SC-343**: `cargo test` counts strictly increase; fmt, the gallery sweep and
  the Windows check stay green.
