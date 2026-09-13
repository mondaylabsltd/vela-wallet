# Feature Specification: Android Send Parity, Batch and Contacts Parity — What the Web Can Do With Money and People, the Phone Can Too

**Feature Branch**: `045-android-send-contacts-parity` (stacked on `044-android-dapp-browser-signing`)

**Created**: 2026-09-12

**Status**: Draft

**Input**: User description: "Everything the web can do with money and
people that the phone still cannot, after 043 (single send) and 044 (dApp
signing). Mirrors desktop 033 (sweep, the treasury sheet's exit, editable
split rows) and 034 (recipient inspection on open, group membership) plus
the web's 026 US3 / 028 batch and contacts I/O. Every phase is verified on
the connected Xiaomi in the parallel space."

## Why

After 043 a person on Android sends one token to one address; after 044 a
page can ask them to sign. The web and the desktop do more with the same
core: one token to several people, several tokens to one address, a
payroll pasted or picked from a spreadsheet, a treasury pause with a way
out, a fee quote that refreshes instead of going stale; and with people —
a contact added and edited by hand, favourites, groups filled from both
sides, a book that travels in a file, and a contact that says what it is
when opened.

None of that needs a new rule. The send machine already speaks split and
sweep (the events cross the Android wire since 043 and were left inert:
"split is 045, the door stays shut"); the batch-import machine exists in
the core and has never crossed the Android bridge; the contacts machine
already saves, favourites, groups, imports, exports and inspects — the
phone drives none of it. Two things are drawn nowhere on the phone: the
add/edit contact form and the favourite control (040 found them missing;
spec 018 named the components). They are drawn first, then wired.

**What the desktop found by running, and this spec inherits**: the
"which rows can be ticked" flag is the shell's, everything else about a
sweep is the core's; the first tap pins the chain, other chains' rows dim
but never disappear ("those coins are still this person's"); "select all
valuable" is scoped to what is on screen; the treasury pause needed a
second exit that keeps the facts; the split rows' single whole-list event
must preserve ids and names when one row changes, and new rows carry no id
so the core mints one.

**Standing exclusions**: the camera scanner (046), fiat sources beyond the
core's rate for the batch editor, the accounts panel's per-account
holdings (047), remote sessions.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Split: one token to several people, as one operation (Priority: P1)

A person picks a token, opens split mode, adds recipients (typed, or from
the book), gives each an amount, and confirms once — one signature, one
operation, one fee.

**Why this priority**: it is the send flow's second mode and the one the
batch importer feeds.

**Independent Test**: a two-row split (dust to the founder's address and
to the Safe itself) reaches the confirm page with both rows, is signed
once, lands as one operation, and the feed shows one row per recipient.

**Acceptance Scenarios**:

1. **Given** the form in split mode, **When** the person adds a second
   recipient and types an amount on each, **Then** both rows keep their
   names and identities, the summary shows the total, and Continue is on
   exactly when the core says every row is complete.
2. **Given** two rows, **When** the person removes the first, **Then** the
   second row is unchanged byte for byte, and its picker still targets it.
3. **Given** the total exceeds the balance, **When** the person continues,
   **Then** the core's refusal is shown in its words and nothing is signed.
4. **Given** the confirm page, **When** the person slides, **Then** one
   signature is produced and one operation is submitted for both rows.

---

### User Story 2 - Sweep: several tokens to one address (Priority: P1)

A person opens the multi-token pick, ticks tokens, and sends them all to
one address at once.

**Why this priority**: the pick is drawn and inert; the rules are proven on
the desktop; the parallel space cannot exercise the sweep itself (the Safe
holds one token on its funded chain), so the pick is the device proof and
the sweep is the test's.

**Independent Test**: on the device, the first tick pins the chain and dims
the other chains' rows without hiding them; in tests, two ticked tokens
reach the confirm page and land as one operation.

**Acceptance Scenarios**:

1. **Given** the pick's "send several tokens" door, **When** tapped,
   **Then** tick boxes and "select all valuable" appear and the door text
   changes to the chain notice once a chain is pinned.
2. **Given** no chain pinned, **When** the first token is ticked, **Then**
   its chain is pinned, rows on other chains dim and stop responding, and
   the button counts one.
3. **Given** a pinned chain, **When** "select all valuable" is tapped,
   **Then** only rows on screen that the core calls valuable are ticked.
4. **Given** the selection cleared, **When** the person ticks a token on
   another chain, **Then** that chain is pinned instead.

---

### User Story 3 - Batch import feeds the split rows (Priority: P2)

A person pastes rows or picks a spreadsheet or a CSV; the core parses it,
prices a fiat column with its rate, and the rows become split recipients.

**Why this priority**: the payroll case the web ships (028); the sheet is
drawn (SD2C).

**Independent Test**: a two-row CSV pushed to the phone is picked through
the system picker, previewed with the core's parse, applied, and the split
form shows two rows that send.

**Acceptance Scenarios**:

1. **Given** the batch sheet, **When** text is pasted, **Then** the preview
   shows the core's rows and errors line by line.
2. **Given** the picker, **When** a file is chosen, **Then** its content is
   parsed by the core, the rate resolved, and Apply fills the split rows.
3. **Given** a fiat unit, **When** the rate is edited or reset, **Then**
   the preview repriced by the core.
4. **Given** "save template", **When** tapped, **Then** a template file is
   created through the document creator.

---

### User Story 4 - The pauses have exits (Priority: P2)

The relay-treasury pause offers "not now" beside the core's retry without
losing the facts; a fee quote that goes stale on the confirm page is asked
again.

**Independent Test**: with the treasury pause staged in tests, both exits
exist and the facts remain; a stale quote on an open confirm page
re-quotes without the person doing anything.

**Acceptance Scenarios**:

1. **Given** the treasury pause, **When** "not now" is tapped, **Then** the
   send flow ends without a signature and the sheet's facts were on screen
   until then.
2. **Given** the confirm page open past the quote's life, **When** the
   quote goes stale, **Then** a fresh one replaces it and the slide reopens
   only with the fresh one.

---

### User Story 5 - Contacts by hand: the form and the favourite (Priority: P1)

A person adds a contact by name and address, edits it, and stars it; the
book shows it under its letter and favourites first.

**Why this priority**: 040 found these undrawn on the phone; without them
the book only grows from history and imports.

**Independent Test**: on the device, a contact saved through the form
appears in the list and its detail; editing renames it; the star moves it
to favourites; a force-stop keeps all of it.

**Acceptance Scenarios**:

1. **Given** the add form, **When** name and address are typed, **Then**
   Save is on only when the core's validation passes, the errors are the
   core's, and the saved contact is in the book.
2. **Given** a contact's detail, **When** edited, **Then** the same form
   opens filled and saves in place.
3. **Given** a contact, **When** starred, **Then** it is a favourite in the
   list, and un-starring undoes it.

---

### User Story 6 - The book travels; groups fill from both sides; a contact says what it is (Priority: P2)

Export the book through the share sheet, import one through the picker
(existing wins, a report shown); put a contact in groups and a group's
members from the group; open a contact and see whether the address is a
contract or a wallet and whether it has been paid before.

**Independent Test**: export → import back reports "existing wins" with the
counts; a group membership set from the group shows on the contact; a
contact opened shows its classification.

**Acceptance Scenarios**:

1. **Given** the book, **When** exported, **Then** a file in the core's
   format leaves through the share sheet.
2. **Given** a file, **When** imported, **Then** the core parses it, keeps
   existing contacts, and the report names added/kept/failed.
3. **Given** a group, **When** members are set, **Then** the contacts show
   the group and the group shows the members.
4. **Given** a contact's detail opens, **When** the inspection resolves,
   **Then** it reads contract or wallet, and first-time or paid before.

---

### Edge Cases

- A split row with an empty amount, an invalid address, a duplicate
  recipient: the core's verdicts, drawn per row.
- The batch file is not a spreadsheet, is huge, or has no valid row: the
  core's errors, no rows applied.
- A sweep with a token whose balance is dust: valuable or not is the
  core's call; the row is tickable only if it says so.
- Import of a file that is not a book: refused with the core's failure.
- The form's address is one of the person's own accounts or already in
  the book: the core's validation says so.
- Process death mid-form: the draft is gone, the book intact.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001 (Split)**: split mode MUST be driven by the send machine's
  events; the whole-list recipients event MUST preserve untouched rows
  byte for byte, keep names and identities, and hand new rows to the core
  without an id.
- **FR-002 (Sweep)**: the multi-token pick MUST pin the chain on the first
  tick, dim (never hide, never tap) rows on other chains, scope "select
  all valuable" to the visible rows, and leave "which is valuable" and
  "how much moves" to the core; the tick-mode flag is the shell's.
- **FR-003 (One operation)**: a split or a sweep MUST land as one user
  operation through 043's spine, with one signature.
- **FR-004 (Batch import)**: the batch-import machine MUST cross the bridge;
  file picking and template saving MUST go through the platform's
  document picker/creator; parsing and pricing MUST stay in the core.
- **FR-005 (Exits)**: the treasury pause MUST offer "not now" beside the
  core's retry, keeping the facts on screen; a stale fee quote on an open
  confirm page MUST be re-asked.
- **FR-006 (The form, drawn then wired)**: the add/edit contact form and
  the favourite control MUST be drawn from spec 018's component vocabulary
  with the corpus's words, in the gallery with their states, before they
  are wired to Save/ToggleFavorite; validation is the core's.
- **FR-007 (The book travels)**: export MUST leave through the share sheet
  in the core's format; import MUST read through the document picker, be
  parsed by the core, keep existing contacts, and show the core's report.
- **FR-008 (Groups both ways)**: membership MUST be settable from a
  contact and from a group, through the contacts machine.
- **FR-009 (Inspection)**: opening a contact MUST ask the core to inspect
  the address and draw contract-or-wallet and first-time-or-not.
- **FR-010 (No Kotlin judgement)**: no shell code decides what a row is
  worth, whether an amount is valid, how a file is parsed, or what a
  contact is.
- **FR-011 (Drift and device)**: the batch wire passes the drift gate;
  every phase is verified on the device and recorded.

### Key Entities

- **Recipient row**: id (core-minted), name, address, amount; the list is
  the event's payload.
- **Multi selection**: the pinned chain, the ticked ids, the valuable ids,
  the specs the core computed.
- **Batch preview**: rows with per-row verdicts, the unit, the rate and
  its status.
- **Contact draft**: name, address, groups, favourite — validated by the
  core.
- **Import report**: added / kept / failed counts and reasons.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001** (device): a two-row split lands as one operation with one
  signature; two feed rows.
- **SC-002** (device): the sweep pick pins the chain on the first tick and
  dims other chains' rows without hiding them; (test) two tokens land as
  one operation.
- **SC-003** (device): a two-row CSV picked through the system picker is
  previewed, applied and sent.
- **SC-004** (test): the treasury pause has two exits with the facts kept;
  a stale quote is re-asked.
- **SC-005** (device): a contact saved through the form is in the book,
  editable, starrable, and survives a force-stop.
- **SC-006** (device): export through the share sheet and import back
  reports existing-wins with counts.
- **SC-007** (device): a contact's detail shows contract-or-wallet and
  first-time-or-not.
- **SC-008**: the drift gate covers the batch wire; tests grow.
- **SC-009**: no send state and no contacts state falls to a fixture on a
  live route.

## Assumptions

- The parallel space's Safe funds the device sends (dust, Gnosis); sweep's
  device half is the pick only.
- The system document picker is driven on the device by pushing a file to
  the phone's Downloads and tapping it in the picker; the share sheet
  is driven by choosing "save to files".
- The 018 vocabulary and the corpus's `contacts.*` words suffice for the
  form; if a key proves missing it is added by the corpus procedure.
- The desktop's rules for split/sweep (033) are the phone's.
