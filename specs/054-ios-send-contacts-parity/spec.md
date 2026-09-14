# Feature Specification: iOS Send & Contacts Parity — What the Other Clients Can Already Do

**Feature Branch**: `054-ios-send-contacts-parity` (stacked on `053-ios-dapp-browser-signing`)

**Created**: 2026-09-14

**Status**: Draft

**Input**: Founder description: "desktop 做了这么多 speckit 来实现功能，我们的 iOS 才
两刀。继续用 speckit 方式一直做到 057 来完成所有功能实现。"

## Why

052 made the iPhone able to pay **one** person. 053 let a web page ask it to
pay. What the other three clients can do and this one still cannot is pay
**several** people at once, empty a wallet into one address, read a payroll
file, and — the smallest and most conspicuous gap — **add a contact**.

The address book has been drawn since spec 018 and wired since 050, and to this
day there is no way to put a name in it from the phone. The core has had
`Save`, `ToggleFavorite`, `ImportFile` and `ExportRequested` the whole time.

| | |
|---|---|
| `send.rs` split/sweep | the modes, the per-row validation, the chain lock, one MultiSend user operation |
| `batch_import.rs` | 1,698 lines: paste, file, the fiat→token rate, the editable matrix |
| `contacts.rs` | `Save`, `ToggleFavorite`, `GroupSave`, `SetGroupMembers`, `SetContactGroups`, `ImportFile`, `ExportRequested` |
| `contacts_io.rs` | 503 lines of pure I/O: JSON and CSV both ways, the filename, the failure kinds |

**What is new on this cut, and why it is a platform job.** Everything here
ends in a *file*: a CSV somebody was sent, an XLSX from a finance team, a
template they want to keep, an address book they want off the phone. iOS has
no document picker, no share sheet, and no XLSX reader in this app today. That
is the one piece of genuinely new platform work, and it is also what 056 reuses
for the share card.

**And one drawing job, first.** The founder's ruling (2026-09-14): the contact
form and the favourite control follow Android 045 — **drawn as gallery states
C7/C8/C9 before they are wired**. A form invented at wiring time is a form
nobody reviewed.

## Program — where 054 sits

| Cut | Mirrors | Machines |
|---|---|---|
| 052 ✅ | Android 043 | `send`, `fee_policy`, `tx_tracker` |
| 053 ✅ | Android 044 | the browser and the signing six |
| **054 (this)** | Android 045, desktop 033+034, web 026 US3 + 028 | `send` split/sweep, `batch_import`, `contacts` + `contacts_io` |
| 055 | Android 046, desktop 035+036+037 | simulation, message depth, the scanner |
| 056 | Android 047, web 028, 038 | `payment_request`, the preferences, the two rulers |
| 057 | Android 048+049 | no new machines — the founder pass and the audits |

## Scope

**In**: split (one token to many people) and sweep (many tokens to one
address); the payroll importer with its fiat→token rate; the contact form,
the favourite, groups both ways, and import/export; a document-port layer that
054 writes and 056 reuses.

**Out of scope, and named so nobody has to guess**:

- **The camera scanner.** Spec 055. `send::OpenScanner` / `ScanResolved` stay
  undispatched and `s1` keeps its inert surface.
- **Simulation and message depth.** Spec 055.
- **`payment_request`.** Spec 056.
- **Any machine change under `rust/crates/vela-core/src/app/`.** Zero lines,
  and zero `vela-core-uniffi` changes.
- **No corpus regeneration.** Specs 018 and 021 drew these screens with their
  text; C7/C8/C9 are assembled from the vocabulary 018 already has.

## User Scenarios & Testing

### User Story 1 — Pay several people at once (Priority: P1)

One token, a list of people, one signature.

**Acceptance**
1. The split form takes rows; each row shows the core's verdict for **that**
   row — an empty amount, an invalid address, a duplicate.
2. Editing one row leaves every other row **byte for byte** as it was,
   including names the core resolved.
3. A new row goes to the core with **no id**; the core mints it.
4. Changing a row's address clears that row's name — the name belonged to the
   old address.
5. The confirm page counts the people and lists them; the receipt's title uses
   the **sum**, not the single-send scalar.
6. It lands as **one** user operation.

### User Story 2 — Empty a wallet into one address (Priority: P1)

**Acceptance**
1. The first tick pins the chain.
2. Rows on other chains are **dimmed and not tappable** — a tappable row is an
   invitation the wallet will not honour.
3. Unticking the last ticked token **releases** the chain, so the next tick can
   pin a different one.
4. "Select all valuable" acts on the **visible** rows only, and which rows are
   valuable is the core's.
5. The CTA counts what is ticked.

### User Story 3 — A list arrives as a file (Priority: P1)

**Acceptance**
1. Paste and pick both reach the same core parse.
2. A `.xlsx` is flattened to a cell grid by the shell; CSV and TXT go as text.
   **The extension decides**, not the MIME the system guessed.
3. The fiat→token rate comes from this wallet's own waterfall and is **never a
   defaulted 1** — an unpriceable currency answers "I do not know" and the core
   refuses to price.
4. The preview shows a verdict per row and refuses as a whole when it must.
5. 获取模板 saves a CSV somewhere the person chose.
6. Apply seeds the split rows.

### User Story 4 — The two exits and the stale quote (Priority: P2)

**Acceptance**
1. The treasury pause offers 暂不 beside the core's retry, and the facts stay
   on screen.
2. A fee quote that goes stale while the confirm page is open and idle is asked
   again — **once per expiry**, not in a loop.

### User Story 5a — Draw the form (Priority: P1, gating)

C7 (add, empty), C8 (edit, filled, address locked), C9 (detail, star lit) exist
as gallery states, built from 018's vocabulary and existing components, and go
through the screenshot sweep **before** anything is wired.

### User Story 5b — Wire it (Priority: P1)

**Acceptance**
1. 保存 puts a contact in the book; it survives a force-quit.
2. Editing changes the name and **locks the address** — a different address is
   a different person.
3. The star is the core's `ToggleFavorite`, and the contact moves to 收藏.
4. Validation is the core's; the form shows the core's words.
5. Typing is echoed **locally** and the value still round-trips through the
   core — a field bound straight to a machine drops characters.

### User Story 6 — The book travels (Priority: P2)

**Acceptance**
1. 导出 produces the core's file and hands it to the share sheet; the view's
   `export` is released afterwards **whether or not the sheet succeeded**.
2. 导入 reads a file, the core parses it, **existing contacts win**, and the
   report says how many were added, kept and refused.
3. A file that is not an address book is refused with the core's failure and
   **nothing is written**.

### User Story 7 — Groups and inspection (Priority: P2)

**Acceptance**
1. Membership is settable from a contact and from a group.
2. Opening a contact asks the core to inspect the address, and the detail says
   contract-or-wallet and first-time-or-not.
3. A judgement the core did not make is **not** rendered as its opposite: an
   unjudged address is never called a wallet.

### Edge Cases

- **An imported group does not gain existing contacts.** The core creates a
  group only for the members it newly added — existing-wins on the contact is
  existing-wins on the membership. Android found this on the device and it is
  the core's rule, not a defect.
- A batch file that is huge, not a spreadsheet, or has no valid row: the core's
  error, and **no rows applied**.
- A form address that is one of the person's own accounts, or already in the
  book: the core's validation says so.
- Process death mid-form: the draft is gone, the book is intact.
- A dust-balance token is tickable only if the core calls it valuable.

## Requirements

- **FR-001 (Split)** Split mode MUST be driven by the send machine. The
  whole-list event MUST preserve untouched rows unchanged, keep names and
  identities, and hand a new row over **without an id**.
- **FR-002 (Sweep)** The first tick pins the chain; other chains' rows are
  dimmed and **not tappable**; "select all valuable" is scoped to the visible
  rows; unticking the last one releases the chain. Which rows are valuable and
  how much moves are the core's. **The picking flag is the shell's**, because
  the core's own mode flips only at confirm.
- **FR-003 (One operation)** A split or a sweep MUST land as one user
  operation through 052's spine, with one signature.
- **FR-004 (Batch import)** `batch_import` MUST be driven from the shell; file
  picking and template saving go through the document layer; parsing and
  pricing stay in the core. **A missing document layer is a failure, not a
  cancel.**
- **FR-005 (The rate is never assumed)** An unpriceable currency MUST answer
  "unknown". A defaulted 1 is a display fallback elsewhere and would arrive
  here as "the rate really is 1".
- **FR-006 (Drawn, then wired)** C7/C8/C9 MUST exist as gallery states and pass
  the screenshot sweep before a single event is dispatched.
- **FR-007 (The book travels)** Export leaves through the share sheet in the
  core's format and releases the view's file afterwards regardless; import
  reads a file, is parsed by the core, keeps existing contacts, and shows the
  core's report.
- **FR-008 (Groups both ways)** Membership settable from a contact and from a
  group.
- **FR-009 (Inspection)** Opening a contact asks the core to inspect the
  address. An unjudged address is never drawn as a wallet.
- **FR-010 (No Swift judgement)** No shell code decides what a row is worth,
  whether an amount is valid, how a file is parsed, or what a contact is.
- **FR-011 (Drift and device)** The batch wire passes the drift gate; every
  phase is verified on the connected iPhone and recorded.

## Success Criteria

- **SC-001** (device) A two-row split lands as one operation with one
  signature, and the feed gains two rows.
- **SC-002** (device for the pick, test for the send) The sweep pick pins the
  chain on the first tick and dims other chains without hiding them; two tokens
  land as one operation.
- **SC-003** (device) A two-row CSV picked through the system picker is
  previewed, applied and sent.
- **SC-004** (test) The treasury pause has two exits with the facts kept; a
  stale quote is re-asked once per expiry.
- **SC-005** (device) A contact saved through the form is in the book,
  editable, starrable, and survives a force-quit.
- **SC-006** (device) Export through the share sheet and import back reports
  existing-wins with counts.
- **SC-007** (device) A contact's detail shows contract-or-wallet and
  first-time-or-not.
- **SC-008** The drift gate covers the batch wire; hermetic tests grow.
- **SC-009** No send state and no contacts state falls back to a fixture on a
  live route.
- **SC-010** No core, corpus, bindings or sibling-client change.
