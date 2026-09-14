# Implementation Plan — 054 iOS Send & Contacts Parity

**Branch**: `054-ios-send-contacts-parity`, stacked on `053-ios-dapp-browser-signing`.

## Summary

Three jobs: the send machine's split and sweep modes, a batch importer with a
document layer under it, and the contact form — drawn first.

## What the survey found, and what it changes

**Two controls already ship that do nothing.** On the live send screen, SD1's
「发送多个代币」 and SD2's 「+ 添加收款人」 both push a state that
`SendLive.flowState` can never return, so the screen re-renders as the one the
person was already on. They are not new work this cut creates — they are
**live defects today**, and they are why `sendStates` overriding the pushed
state is the first thing to fix rather than a detail.

**The contact form is not "unwired", it is absent.** The `+` menu's three rows
dismiss themselves; the detail's edit pencil is a literal no-op; deleting from
the detail does nothing because the live builder passes no sheet; and the
search field is a `Text`, so live search is permanently `nil`. `contacts.save`,
`editTitle`, `nameLabel`, `invalidAddress` and `sectionFavorites` exist in
fourteen languages and are referenced by zero Swift files.

**Four view fields are decoded by nobody**: `sections`, `import_failure` and
`export` on contacts, and `multi_specs` on send. The first is a deliberate
divergence (iOS derives its own A–Z with CJK transliteration, and that is
recorded); the other three are this cut's.

## Constitution Check

| Gate | How |
|---|---|
| Core decides, shell performs | Split-row edits are three pure functions over a list; the sweep's tap rules are a table; neither judges an amount, an address or a value. |
| Zero core / corpus / bindings / sibling change | Verified at closeout. Every word C7–C9 needs is already in the corpus. |
| Executor contract | `BatchExecutor` — three arms, three neutral twins, one drift test. |
| Drawn before wired | C7/C8/C9 through the screenshot sweep before an event is dispatched (FR-006). |
| Device verification | FR-011. |

## Phases

| # | What | Ends when |
|---|---|---|
| 0 | `BatchWire`, `BatchExecutor`, the drift test; `SendWire` gains `multi_specs` and the three missing fields | the wire is exhaustive and drift-gated; no screen changed |
| 1 | **Split**: `SplitRows`, the three events, SD2b/SD3b reachable, the receipt's sum | a two-row split lands as one operation on the phone |
| 2 | **Sweep**: `SweepPick`, SD1b/SD2d/SD3c reachable, the chain pin and the dimming | the pick's rules are right on the phone |
| 3 | **Batch**: `BatchStore`, SD2c live, the paste box a real field, the pickers | a CSV picked on the phone is previewed, applied and sent |
| 4 | The treasury's second exit and the stale re-quote | both driven in tests; recorded if the relay will not starve |
| 5 | **Draw** C7/C8/C9 and sweep them | three screenshots, nothing wired |
| 6 | **Wire** the form, the star, delete-from-detail, the search field, groups both ways, inspection | a contact saved on the phone survives a force-quit |
| 7 | Import and export through the document layer | a round trip on the phone reports existing-wins |
| 8 | Closeout | every SC marked |

Phase 1 is the gate for 2 and 3 — split's whole-list event is what the batch
importer seeds and what the sweep's confirm screen reuses.
