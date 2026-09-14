# Results — 052 iOS Money Wiring

Written as the work lands. **[tasks.md](./tasks.md) is the handoff** — it holds
the commands, what is next and the traps. This file holds what happened.

---

## Phase 0 — Baselines

Branch point: `051-ios-read-wiring` @ `6e693b26`.

| | |
|---|---|
| `@Test` functions | **356** (331 hermetic + 25 behind `-DVELA_LIVE_TESTS`) |
| XCUITest `func test` | **15** (11 acceptance, 2 sweep, 2 launch) |
| Swift files under `VelaWallet/` | 179 |
| Committed `vela_core_uniffi.swift` | **353,772 bytes** — and this cut expects it to end at exactly that |
| Literal-audit violations | **35** in 146 files (never green on `main`; the gate is "no new ones") |
| `// live in 052` markers | **3** |
| Device Debug dylib | measured at phase 1, when the first device build runs |

### The inventory that changed the cut's shape

Phase 0's first job was to stop guessing what the repository contains — 051's
most expensive mistake was an assertion from memory. What the count found:

| | |
|---|---|
| `bridge_object!` exports on this branch | **23 of 23** |
| …driven by Swift | **10** |
| `user_op_*` free functions exported | **8 of 8**, plus the five relay-classification helpers |
| Swift references to any of them | **zero** |

Android's 043–049 (merged to `main` as PR #194, which this branch already
carries) exported everything this cut needs. So **052 adds no bridge line and
regenerates no bindings** — the first iOS cut where the five-to-eight-minute
xcframework round trip is off the critical path entirely. SC-011 turns that
into a gate rather than an observation: if `vela_core_uniffi.swift` has a
diff at closeout, something was added that the plan says is unnecessary.

### The three markers this cut inherits

| File | Line | What it answers today |
|---|---|---|
| `Features/Settings/NetworkAdminExecutor.swift` | 263 | `clear_bundler_cache` → `bundler_cache_cleared`, a truthful no-op: there is no bundler client, so no cache |
| `Features/Contacts/ContactsExecutor.swift` | 108 | `load_send_history` → `history_failed`, **deliberately** not an empty list: an empty list would tell the core nobody has ever been paid and make every address wear the poisoning warning forever |
| `Features/Contacts/ContactsLive.swift` | 163 | the contact detail's 最近往来 renders the drawn empty state |

---

## Success criteria — verdicts

Filled at closeout. Every row says **device-verified** or **test-only**; a
simulator run is preparation, never proof.

| | Criterion | Verdict |
|---|---|---|
| SC-001 | dust leaves the golden Safe from the iPhone | |
| SC-002 | the same send from the founder's own passkey, one prompt | |
| SC-003 | submit, force-quit, relaunch — the row reaches confirmed | |
| SC-004 | cancel at the assertion; the prompt count is one | |
| SC-005 | every refusal driven and read; no raw units | |
| SC-006 | the fee token changes and the quote follows | |
| SC-007 | the live route reads no send fixture | |
| SC-008 | the drift gate is exhaustive for all three families | |
| SC-009 | the three markers are gone | |
| SC-010 | a Release archive carries no fixture symbol | |
| SC-011 | no core, corpus, bindings or sibling-client change | |
