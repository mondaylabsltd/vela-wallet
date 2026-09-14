# Results — 054 iOS Send & Contacts Parity

## Baselines, taken at the branch point

| | at 054's start | now |
|---|---|---|
| hermetic test run | 426 | **447** |
| XCUITest methods | 29 | 29 |
| literal-audit violations | 35 | **35** |
| `vela_core_uniffi.swift` bytes | 353,772 | untouched |

## Phase 0 — the papers and the pure pieces

`XlsxMatrix` (against the desktop's real payroll fixture), `DocumentPorts`,
`BatchWire`, `BatchExecutor`, `SplitRows`, `SweepPick`, and `SendWire` gaining
`multi_specs`. Decisions D0–D10 in [research.md](./research.md).

### What the survey changed about the shape of this cut

**Two controls already ship that do nothing.** SD1's 「发送多个代币」 and SD2's
「+ 添加收款人」 both push a state `SendLive.flowState` can never return, so the
live router overrides it back to the screen the person is already on. They are
live defects, not work this cut creates — and they are why making
`flowState` reach the split and sweep states is the first thing rather than a
detail.

**The contact form is not unwired, it is absent.** The `+` menu's three rows
dismiss themselves, the detail's edit pencil is a literal `Button {}`, deleting
from the detail does nothing because the live builder passes `sheet: nil`, and
the search field is a `Text` so `contacts.query` is permanently `nil`.
`contacts.save`, `editTitle`, `nameLabel`, `invalidAddress` and
`sectionFavorites` exist in fourteen languages and are referenced by zero Swift
files.

## Phase 1 — split

`SendLive.flowState` can reach sd2b/sd2c/sd2d/sd3b/sd3c. The split row is
editable — address and amount — with the core's verdict **on the row**: a list
of six with one sentence underneath makes somebody count rows to find the bad
one.

Every keystroke sends the **whole list** back, because `recipients_changed` is
the event the core has: it reconciles ids, names and identities, and a delta
would be the shell deciding which parts of its own state the core may trust.
`SplitRows` keeps untouched rows byte-identical, which is what makes that cheap
— and what stops a name the core resolved from blinking out while somebody is
reading it.

The editable row follows 052's mode-not-a-type shape: `nil` bindings render
exactly as drawn, so the gallery and the screenshot sweep stay pixel-identical.

**447 hermetic tests. The send journey still passes on the iPhone — 2 of 2, no
regression from the router change.**

### Owed

**SC-001's device pass.** A two-row split landing as one operation on the phone
has not been driven. Everything under it is hermetic.

## Still ahead

Phases 2–8: sweep, the batch importer, the two exits, **drawing C7/C8/C9**,
wiring the form, the book travelling, closeout. The nine survey findings that
no phase covers yet are listed in [tasks.md](./tasks.md).
