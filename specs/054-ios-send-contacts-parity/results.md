# Results — 054 iOS Send & Contacts Parity

## Baselines, taken at the branch point

| | at 054's start | now |
|---|---|---|
| hermetic test run | 426 | **473** |
| XCUITest methods | 29 | **33** |
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

## Phase 2 — sweep

「发送多个代币」 turns the tick boxes on; confirming hands the selection to the
core. The picking flag is the SHELL's (`@State sweepPicking`), because the
core's `multi_select_mode` flips at confirm and so cannot answer "are we
picking". 全选有价值代币 is scoped to the rows on screen. The sweep form reads
`multi_specs` and never the balances: **no spec, no figure**, because printing
the balance would print a number the operation does not carry.

One self-inflicted defect found and deleted: a second token id
(`chainId:address`) invented here when the core's is `network_address_symbol`,
already on the wire with a comment calling it "the only spelling of it". A tick
would never have matched its token.

## Phase 3 — the payroll importer

`BatchStore` hosts `batch_import` beside the send machine; `SendLive.batchImport`
builds SD2c; `FlowHost` passes the four callbacks `BatchImportBody` has always
accepted, and — the defect that took a device run to find — **forwards them to
`FlowSheetHost`**, which it had not.

導入表格 is a CORE flag, not a push: a pushed SD2c is overridden straight back to
the form by the live router, which is the dead-button shape this flow has now
produced three times.

The paste box is a real `TextEditor` (a sheet whose purpose is pasting a list
had a read-only `Text`), the rate is a real field with 自动 beside it once
somebody types their own, and the rate block disappears in token mode — when the
file's figures ARE token amounts there is no rate, and "1 xDAI = " with nothing
after it is worse than nothing.

The fiat column prices through the DISPLAY machine's own waterfall
(`SettingsStore.usdRate`), never a second resolver: that resolver falls back to
1 so a screen always has something to print, and a 1 in a payroll sends 5,000 of
a currency worth a fraction of a dollar as 5,000 tokens. The cap passed to the
machine is **60** — `BATCH_MAX_RECIPIENTS`, the same number every other client
passes.

## Phase 4 — the confirm page's two exits

The confirm page had no notice slot at all, so a depleted relayer, a submit the
relay refused and a passkey prompt already up were all invisible: the CTA simply
went inert. It now carries the core's sentence and the right buttons under it —
retry for all three, plus **暂不** for the treasury pause alone
(`dismiss_treasury_sheet`, which had no call site anywhere in the client). A
refused submit gets no 暂不: suggesting the attempt is still alive would be a
lie. `sendCtaDisabled` learned the same three reasons, so the button and the
line explaining it cannot disagree.

A quote that expires while somebody reads the page is re-asked once per expiry,
only while the page is idle. The core keeps the request that was priced, so
`requote` re-runs THAT one; asking during a submit would move the fee under a
signature already being made.

## Phase 5 — C7 / C8 / C9, drawn

Per the founder's ruling (2026-09-14): drawn first, in the gallery, before any
of it was wired. The add form (empty, Save shut), the edit form (filled, address
**locked** — in an address book the address is the identity of the person, so
typing a different one names somebody else), and the detail with the star lit and
the core's two neutral tags beside the name. One `ContactFormSheet` serves both
forms; they differ by exactly two facts.

New `starSolid` glyph: a favourite that is on must differ by more than a tint,
because colour alone is invisible to somebody who cannot see it.

Found by LOOKING at the gallery rather than by an assertion: the editable address
field had no well (`FlowMonoInput` fills with `bgRaised`, which is the sheet's
own background — the field was invisible).

C10 followed in phase 6, for the same reason and the same way: a membership
picker drawn from 018's existing vocabulary before being relied on.

## Phase 6 — the address book's write half

Nine drawn things that did nothing now do it, each with a machine already
waiting for it in `vela-core`: the form's 保存 (one event for new and edit, and
the CORE validates), the star, 删除联系人 on the detail page, the search field
(it was a `Text`, so `ContactsLive.home(query:)` — which has had a narrowing test
since 050 — reached for a query nothing could set), the C5 menu's three rows, the
group menu's import and export, import and export themselves, the two tags from
`view.recipient`, and membership in both directions.

`set_contact_groups` and `set_group_members` had **zero call sites in the whole
client**. C10 answers both, and it commits a SET rather than a stream of taps —
the core replaces membership outright, and a per-tap sheet would leave a
half-applied grouping behind if somebody closed it midway.

A picked file's TEXT goes straight to the core, which sniffs JSON from CSV and
refuses a bad file before ANY write. The shell does not parse it: a second reader
of the same file eventually disagrees with the first. `import_failure` is TAGGED
(`{"type": "malformed_json"}`), not bare — reading it as a string is exactly the
drift this gate exists to catch, and it failed that way on the first run.

**Recorded, not fixed:** the sweep form's per-row Max chip is gone from the live
screen. The only event behind it is `tap_max`, which acts on the single selected
token — tapping it on the third row of a sweep would silently change a different
amount. A sweep already moves the maximum of every row. The drawing keeps the
chip; Android draws it and drops the index.

## Phase 7 — driven on a screen

`ParityAcceptanceTests` drives the address book behind `VELA_PAGE=contacts-live`
and the send journey in the parallel space. Writing it turned up five defects
invisible to every test that already existed:

1. **Contact rows did not respond to taps. At all.**
   `.gesture(dragGesture, including: swipe == nil ? .none : .all)` —
   `GestureMask.none` disables EVERY gesture in the subtree, including the row's
   own button. Only C1s carries a swipe model, so on every real screen the list
   was inert: the element reported itself hittable, the tap synthesised, nothing
   happened. Shipped in 018.
2. **The batch machine was never booted.** `CoreStore` drops events sent before
   `boot`, so the importer rendered its empty view forever — "解析结果 · 0 条"
   over a paste box with two perfectly good lines in it.
3. **The contact form lost characters as they were typed** — "Vela 054 探针"
   arrived as "Va 054 探针", an address lost its `0x`. Optional-chained bindings
   into a struct rebuilt on every keystroke; plain `@State` strings now.
4. **`send.recipientCount` is not a key** (only `_one` / `_other`), so the split
   summary printed the literal string "send.recipientCount" on screen. Found in
   an accessibility dump, not by an assertion.
5. **The add menu opened by itself** on arrival, because `onAppear` keyed on
   `sheet != nil` and the live home had just started carrying the menu.

Plus: 删除联系人 on the detail raised an EMPTY sheet (the live builder passed
`sheet: nil`), and every contacts screen now presents ONE sheet whose content
changes rather than two or three `.sheet` modifiers on one view — the
2026-07-06 stacked-modal lesson.

## Success criteria

| | claim | verdict |
|---|---|---|
| SC-001 | a two-row split lands as ONE user operation | **test-only** — hermetic through `SplitRows` + the wire; no device send (it spends money and is owed with the 052 live sends) |
| SC-002 | the sweep pins a chain, dims the rest, and counts | **device-verified** — iPhone 11, `testTheSweepPickPinsAChainAndDimsTheRest`: 发送多个代币 turns the boxes on, the first tick pins Gnosis, the CTA reads 发送 1 个 · Gnosis → |
| SC-003 | a pasted payroll is parsed, priced and applied | **device-verified** — iPhone 11, `testAPastedPayrollIsParsedAndSeedsTheSplit`: two rows pasted, priced at the phone's own display currency (按 CNY 计价, 1 xDAI = 6.706 自动, a LIVE rate), both ticked and converted, applied into a two-row split |
| SC-004 | the treasury pause has a second exit | **test-only** — `ConfirmNoticeTests` drives all three notice states; a depleted relayer cannot be summoned on demand |
| SC-005 | C7 / C8 / C9 drawn before being wired | **device-verified** (as drawings) — gallery screenshots in the commit for 054 phase 5 |
| SC-006 | a contact typed on the phone is saved, survives a relaunch, and can be deleted from its own page | **device-verified** — iPhone 11, `testAContactTypedOnThePhoneIsSavedAndThenDeleted` |
| SC-007 | the star flips and the core owns the flip | **test-only on the device run** — `testTheStarOnAContactsPageFlips` skips when the book is empty, which it is after SC-006 cleans up; covered hermetically by `ContactsWiringTests.theStarFlips` |
| SC-008 | an address book exports to a file and imports back | **test-only** — `ContactsWiringTests` drives the core both ways; the share sheet and the document picker need a finger |
| SC-009 | membership commits as a set, both directions | **test-only** — `ContactsFixturesTests` + the core; needs a finger on the phone |

**Device evidence:** `/tmp/parity-dev.xcresult` — 3 of 3 passed on the iPhone 11
(`00008030-001A75961445802E`), screenshots attached.

### Two harness facts worth keeping

**The phone refuses automation mode intermittently.** "Timed out while enabling
automation mode" for three runs, then fine. When it happens the fix is a person
at the phone: unlock it, and check 设置 ▸ 隐私与安全性 ▸ 开发者模式.

**A `TextEditor` takes a newline from a real keyboard and not from the
simulator's.** The device parses two pasted rows; the simulator parses one. The
test asserts on the count being the core's rather than on which count it is.

## Still ahead

Phase 8's remaining device work needs a finger: the export share sheet, the
document picker, the group pickers, and SC-001's live two-row send (which spends
money and belongs with the 052 live sends). The nine survey findings are all
addressed — see phase 6 and phase 7.
