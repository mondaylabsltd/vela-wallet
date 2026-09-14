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
