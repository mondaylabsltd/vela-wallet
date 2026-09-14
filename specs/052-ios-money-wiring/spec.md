# Feature Specification: iOS Money Wiring — the Wallet Moves Money

**Feature Branch**: `052-ios-money-wiring` (stacked on `051-ios-read-wiring`)

**Created**: 2026-09-14

**Status**: Draft

**Input**: Founder description: "desktop 做了这么多 speckit 来实现功能，我们的 iOS 才
两刀。继续用 speckit 方式一直做到 057 来完成所有功能实现，可以参考 app-web /
app-desktop / app-android，以及考虑当前进展。"

Program context: [`docs/ios/wiring-program-052-057.md`](../../docs/ios/wiring-program-052-057.md)
(written in 056; until then the program table lives in this spec's **Program**
section below).

## Why

After 051 the iPhone shows the person's own money and **cannot move a cent of
it**. The send journey has been drawn since spec 021 — picker, form, confirm,
receipt, the fee-token, contact-pick and batch sheets — and every rule behind
it is already in the core and already exported to Swift:

| | |
|---|---|
| `send.rs` | 4,417 lines: three modes, live validation, string-exact Max, the same-asset fee ceiling, the treasury pre-check, the sign→submit lifecycle with its cancel checkpoints |
| `fee_policy.rs` | 2,322 lines: the tiers, the in-band markup, the Tempo split, the reserves |
| `tx_tracker.rs` | 1,056 lines: the poll cadence, the receipt classification, the abandon deadline |
| `SendCore` / `FeePolicyCore` / `TxTrackerCore` | already `bridge_object!`-exported and already in the committed `vela_core_uniffi.swift` — **zero Swift references** |

So this cut adds **no Rust at all**. What is missing is three executors, a
relay client, the user-operation spine, and a flow host that reads the core's
view instead of `WalletFlowFixtures`.

**What is different on a phone, and why the parallel space comes first.**
A send ends in a biometric prompt, and a biometric prompt needs a finger. The
founder's standing rule for this program is that every feature is verified on
the connected device (2026-09-12: "没有验证，容易写出错误的不符合需求的东西").
Desktop 032 and Android 043 each solved that in their first phase with the
core's fixed keyset — the same scalars, deriving the same Safe — signing where
a passkey would, behind a compile-time gate and a runtime door. iOS inherits
the keyset for free (`rust/crates/vela-dev-fixtures-uniffi`); what it does not
inherit is the packaging, because that crate has an Android build script and
no iOS one. Without the parallel space every phase here ends with "needs a
human"; with it, only one does.

**Money in flight must outlive the screen**, and iOS is stricter about that
than any shell before it. The web has a tab and the desktop has a window;
an iPhone suspends the process within seconds of a lock and may kill it
outright. So the pending row is written **before** tracking begins (the core's
ordering invariant), tracking resumes from storage on launch, and a
confirmation that lands while the app is away is delivered as a local
notification. Unlike Android's WorkManager, iOS grants no guaranteed window —
what it grants is ~30 seconds of grace and a background refresh it schedules
at its own discretion. This spec says that plainly rather than promising a
cadence the platform does not offer (FR-007).

## Program — where 052 sits

| Cut | Mirrors | Machines |
|---|---|---|
| **052 (this)** | Android 043, web 026 ph1–4/6, desktop 032 ph1–6 | `send`, `fee_policy`, `tx_tracker` |
| 053 | Android 044, web 027, desktop 032 ph13–33 | `dapp_permissions`, `explore_sites`, `browser_history`, `sign_request`, `clear_signing`, `approval_guard` |
| 054 | Android 045, desktop 033+034 | `send` split/sweep, `batch_import`, `contacts`+`contacts_io` |
| 055 | Android 046, desktop 035+036+037 | `token_trust::SimDeltasComputed`, `clear_signing` message rungs, the scanner |
| 056 | Android 047, web 028, 038 | `payment_request`, the preferences that have no machine, the two rulers |
| 057 | Android 048+049 | no new machines — the founder pass, the audits, the send proof |

## Scope

**In**: `send` in **single mode** end to end, `fee_policy` as its quote
session, `tx_tracker` as an app-resident, the parallel space, the fee-token
and contact-pick sheets, and the three `// live in 052` arms 050 and 051 left
behind.

**Out of scope, and named so nobody has to guess**:

- **Split and sweep modes, and the payroll importer.** Spec 054. The multi
  family of events is not dispatched, and `sd1b`/`sd2b`/`sd2c`/`sd2d`/`sd3b`/
  `sd3c` keep their fixtures.
- **The camera scanner.** Spec 055. `send`'s `OpenScanner`/`ScanResolved`
  are not dispatched and `s1` keeps its inert surface. `AddNetwork` is the
  scanner's only entry, so it answers `Error` here — as Android 043 did.
- **dApp signing.** Spec 053. `SimulateCalls` answers `sim_json: null`, which
  is what the core reads as "no simulation", and the signing sheet stays a
  gallery.
- **Any machine change under `rust/crates/vela-core/src/app/`.** Zero lines.
  Unlike 051, this cut expects **zero `vela-core-uniffi` changes too**: every
  export it needs is already there. The one Rust edit it does make is a
  `crate-type` line on `vela-dev-fixtures-uniffi`, which adds no code.
- **No corpus regeneration.** Spec 021 drew these screens with their text.

## User Scenarios & Testing

### User Story 0 — The parallel space on iOS (Priority: P1, enabling)

A developer with the phone on a cable launches a Debug build through its
developer door and gets the real app — real chains, real relay, real storage,
every screen — signed for by the core's fixed keyset. The wallet it shows is
the one the web's `/parallel` and the desktop's `VELA_PARALLEL_SPACE=1` show,
at the same address. A Release build has no such door and no such keys.

**Why this priority**: it is what lets every other story be verified on the
device without a finger, and it is the founder's own test environment on a
fourth client.

**Independent Test**: open the door; the receive screen shows the golden
multi-key Safe every other client derives; the badge is visible; an archived
Release build contains no fixture symbol.

**Acceptance Scenarios**:

1. **Given** a Debug build, **When** the door is opened, **Then** the app is
   signed in as the fixture account and every screen carries the badge.
2. **Given** the parallel space, **When** a send reaches signing, **Then** no
   biometric prompt appears and the assertion the relay receives verifies on
   chain.
3. **Given** the door is closed again, **When** the app relaunches, **Then**
   the person's **own** wallet is back, unharmed — the fixture record is
   removed and nothing else in `vela.accounts` was touched.
4. **Given** a Release archive, **When** its binary is inspected, **Then** no
   fixture key material and no door are present.

---

### User Story 1 — Send a token to someone (Priority: P1)

A signed-in person taps 转账, picks a token they hold, types or pastes an
address (or picks a contact), enters an amount or taps 最大, continues, reads a
confirmation that states the recipient, the amount, the fee and the fee token,
confirms, and lands on a receipt that says *submitted* and then *confirmed*,
with a link to the explorer. The home feed shows the row the moment it is
submitted.

**Why this priority**: it is the program's reason to exist, and the founder's
revenue rides on transactions.

**Independent Test**: in the parallel space, send dust from the golden Safe on
Gnosis; the receipt reaches *confirmed*, the explorer shows the transaction,
and the feed row appears at submit and flips at confirm.

**Acceptance Scenarios**:

1. **Given** the token list, **When** a token is picked, **Then** the form
   opens with that token, its balance and its chain, and the amount validates
   as it is typed.
2. **Given** a valid recipient and amount, **When** 继续 is tapped, **Then** the
   fee quote and the treasury pre-check run together and the confirm screen
   shows exactly the fee that will be signed.
3. **Given** the confirm screen, **When** the person confirms and the assertion
   succeeds, **Then** the relay accepts the operation, the receipt shows
   *submitted* with a hash, and the feed shows the pending row.
4. **Given** a submitted send, **When** the receipt lands, **Then** the receipt
   and the feed row both say *confirmed* (or *failed*, with the reason) and the
   balance refreshes.
5. **Given** 最大 on a token that also pays the fee, **When** the amount is
   filled, **Then** it is the balance less the fee reserve, to the last unit,
   and the send succeeds.

---

### User Story 2 — Money in flight outlives the screen (Priority: P1)

The person confirms a send and locks the phone. Coming back, the receipt is
still there and now says *confirmed*. Had they force-quit instead, the feed
still shows the row and it still reaches *confirmed*. If a confirmation lands
while they are in another app, the phone tells them — when iOS lets it.

**Why this priority**: a send that vanishes when the screen does is money the
person believes is lost.

**Independent Test**: submit in the parallel space, force-quit, relaunch: the
pending row is there and reaches *confirmed*.

**Acceptance Scenarios**:

1. **Given** a send submitted, **When** the app is force-quit and relaunched,
   **Then** the pending row is in the feed and tracking resumes with no action
   from the person.
2. **Given** a submitted send and the app backgrounded, **When** the receipt
   lands inside the grace window, **Then** a local notification names the
   outcome and opens the receipt when tapped.
3. **Given** a receipt that lands while the process is suspended, **When** the
   app is next opened, **Then** the row reaches its verdict immediately — late,
   never lost.
4. **Given** a send whose receipt reports failure, **When** the feed is read,
   **Then** the row says *failed* with the core's reason, never *pending*
   forever.

---

### User Story 3 — The screen says what the core refuses (Priority: P2)

Every refusal the core has a word for reaches the screen as that word: an
amount above the balance, an amount that leaves nothing for the fee, an
address that is not one, a chain with no relay coverage, a relay whose
treasury cannot front the fee, a quote that could not be obtained, a submit the
relay rejected, a cancel at any checkpoint. None of them is a spinner that
never stops, and none of them is a second biometric prompt.

**Why this priority**: 050 and 051 both found that the costly defects on this
client were silences — a health pill that was a fixture, a receive address
belonging to nobody — not crashes.

**Independent Test**: drive each refusal in the parallel space and read the
screen; cancel at the assertion and count the prompts.

**Acceptance Scenarios**:

1. **Given** an amount above the balance, **When** it is typed, **Then** the
   form says so inline and 继续 is disabled.
2. **Given** a relay whose treasury is empty for this chain, **When** 继续 is
   tapped, **Then** the sheet the core prescribes opens with its guidance and
   nothing is signed.
3. **Given** the assertion in progress, **When** the person cancels, **Then**
   the flow returns to confirm and no second prompt appears for that attempt.
4. **Given** a relay rejection at submit, **When** it arrives, **Then** the
   receipt says *failed* with the relay's reason and the balance is untouched.

---

### User Story 4 — The sheets, and the arms 050/051 left (Priority: P2)

The fee-token sheet lists the fee coins the chain accepts with their balances;
the contact-pick sheet lists the person's own book and fills the recipient.
Three arms marked `// live in 052` answer for real: the contact detail's
recent-dealings list reads the transaction store, the contacts machine's send
history answers from it rather than refusing, and the settings' relay-cache
clear reaches a cache that now exists.

**Why this priority**: each is a drawn surface that today shows a fixture or
does nothing, and the three markers are this cut's inherited contract.

**Acceptance Scenarios**:

1. **Given** the confirm screen, **When** the fee token is changed, **Then**
   the quote re-runs and the confirm shows the new fee in the new token.
2. **Given** the form, **When** a contact is picked, **Then** the recipient
   field carries that contact's address and name.
3. **Given** a contact who has been paid from this device, **When** their
   detail opens, **Then** 最近往来 lists those transfers rather than the drawn
   empty state.

---

### Edge Cases

- A cancel at any checkpoint kills the pre-sign pipeline; a passkey prompt
  never resurrects after a cancel.
- The quoted fee handed to submit is built from the very estimate the confirm
  screen rendered: **displayed equals signed**.
- The pending row is persisted before tracking starts; a process death between
  the two leaves a row, never a tracked ghost.
- A second tap on confirm while the first is in flight does nothing.
- A chain with no native coin (Tempo) pays its fee in a stablecoin; the
  fee-token sheet shows only what that chain accepts.
- A rate-limited endpoint mid-quote is *busy*, not *broken*: the quote retries
  through the pool and the screen does not blame the chain (051's rule).
- The parallel space disables relay sponsorship by design; the golden Safe must
  hold the fee token, and this spec's device sends are dust.
- A hidden balance stays hidden on the send form and the receipt.
- The notification permission is asked once, at the first submit, never at
  launch; a refusal degrades to the in-app receipt.
- A nonce that cannot be read refuses **before** the prompt: a passkey ceremony
  on an operation the relay must reject is a ceremony wasted.

## Requirements

- **FR-001 (The door and the keys are Debug-only)**: a Debug build MUST offer a
  developer entrance to the parallel space; a Release build MUST contain
  neither the entrance nor the fixture key material, and the proof MUST be a
  symbol inspection of an archived binary rather than a reading of the source.
- **FR-002 (The fixture signs, the app does not change)**: in the parallel
  space every signing step MUST be answered by the core's fixed keyset,
  deriving the account the other three clients show, and every screen MUST
  carry the space's badge. Outside it the same step MUST be answered by the
  person's passkey through `PasskeyExecutor.assert`, with no other difference
  in the flow.
- **FR-003 (The door never harms the real wallet)**: entering and leaving MUST
  upsert and remove exactly one record in `vela.accounts` and touch nothing
  else. Replacing the list — which is what the read-path seed does — is
  forbidden here, because this door is opened on a phone that holds the
  founder's own wallet.
- **FR-004 (One send, end to end, on the core)**: the send flow in single mode
  MUST run on `send` from token pick to receipt: validation, Max, 继续's
  parallel quote and treasury pre-check, confirm, sign, submit, persist, track.
- **FR-005 (Displayed equals signed)**: the fee shown on confirm MUST be the
  fee that is signed; the fee-token sheet MUST offer only the fee coins the
  chain accepts, each with its balance, and changing it MUST re-quote.
- **FR-006 (Pending at submit)**: the pending row MUST be written to
  `vela.transactionHistory` before tracking begins, and the home feed MUST show
  it from that moment.
- **FR-007 (Tracking outlives the screen, honestly)**: tracking MUST resume
  from storage on launch, MUST continue through the background grace the
  platform grants, and MUST reach its verdict on the next launch when the
  platform grants nothing. A receipt landing while the app is away MUST surface
  as a local notification that opens the receipt. The spec claims **no
  guaranteed background cadence**; what it claims is that no verdict is lost.
- **FR-008 (The core's words)**: every refusal the core reports MUST reach the
  screen as the core's wording, formatted by the shell. No refusal may leave a
  spinner or a disabled button with no explanation, and no raw unit (wei) may
  appear in a sentence meant for a person.
- **FR-009 (One prompt per attempt)**: a cancel at any checkpoint MUST end that
  attempt; no second biometric prompt may follow a cancel. This is a counted
  test on the signer seam, not a hope.
- **FR-010 (The sheets read real state)**: the fee-token sheet MUST read the
  chain's accepted fee assets with the person's balances; the contact-pick
  sheet MUST read the person's own book and fill the recipient.
- **FR-011 (The three markers come down)**: `contacts::load_send_history`,
  `ContactsLive`'s empty activity list and `network_admin::clear_bundler_cache`
  MUST answer for real, and the table of them is the handoff contract.
- **FR-012 (Core decides, shell performs)**: no Swift may decide what the core
  decides — fee math, validation, checkpoints, receipt classification, lock
  discipline and rejection classification stay in Rust. The shell contributes
  transport, storage, the assertion, the clock and the notification.
- **FR-013 (Every operation answered exactly once)**: 050's rule, unchanged,
  including the loud answer for an unrecognised tag. Each of the three machines
  gets a `neutralAnswer` twin and a drift test.
- **FR-014 (Fixtures stay canon)**: no `*Fixtures.swift` loses or alters a
  constant; the galleries and the screenshot sweep render unchanged.
- **FR-015 (Device verification)**: every phase MUST be exercised on the
  founder's iPhone before it is called done, and results.md MUST say for every
  success criterion whether it was **device-verified** or **test-only**.
- **FR-016 (No machine changes)**: zero lines under
  `rust/crates/vela-core/src/app/`; zero corpus delta; no other client touched.

## Key Entities

- **Send attempt** — one pass through the machine from token pick to a terminal
  receipt or a cancel; carries a generation token so a stale answer cannot land
  on a newer attempt.
- **Fee quote** — the estimate the confirm screen renders and the submit signs;
  carries the fee token, the amount, the recipient and its expiry.
- **Pending record** — the row written at submit, keyed by the operation hash,
  that the feed shows and the tracker resolves to *confirmed* or *failed*.
- **Parallel space** — the real app with one substitution, entered through a
  Debug-only door, marked by a badge, deriving the shared fixture account.
- **The spine** — one user-operation assembly serving a person's transfer today
  and a dApp's transaction in 053, so the sheet that says what will happen and
  the code that makes it happen cannot become two opinions.

## Success Criteria

- **SC-001** (device): in the parallel space, dust leaves the golden Safe from
  the iPhone and the explorer shows the transaction; the receipt reaches
  *confirmed* within the chain's normal confirmation time.
- **SC-002** (device, founder): the same send once from the founder's own
  passkey wallet, with exactly one biometric prompt.
- **SC-003** (device): submit, force-quit, relaunch — the pending row is
  present and reaches *confirmed*.
- **SC-004** (device): cancel at the assertion; the prompt count for that
  attempt is one and the flow returns to confirm.
- **SC-005** (device): each refusal in User Story 3 is driven and its wording
  read on screen; zero refusals end in a silent spinner, and no sentence
  contains a raw unit.
- **SC-006**: change the fee token on confirm; the fee re-quotes in the new
  token. Device where the golden Safe holds two fee assets on some chain,
  test-only otherwise — and results.md says which.
- **SC-007**: the live send route reads no send fixture; a grep for
  `WalletFlowFixtures.send` outside the gallery returns nothing.
- **SC-008**: the drift gate is exhaustive for the `send`, `fee_policy` and
  `tx_tracker` operation families; the Swift test count strictly increases and
  none are deleted.
- **SC-009**: the three `// live in 052` markers are gone.
- **SC-010**: an archived Release build's binary contains no fixture symbol and
  no parallel-space string.
- **SC-011**: zero lines under `rust/crates/vela-core/src/app/`, zero corpus
  delta, zero lines under the other four clients; `vela_core_uniffi.swift`
  unchanged (this cut adds no export).

## Assumptions

- The relay behaves as it does for the other clients: sponsorship is off in the
  parallel space, and the golden Safe `0x88cCA0EeDbF2C4426110bbFc998F048689266894`
  holds xDAI on Gnosis — the chain every other client used for its dust.
- The two gates from the desktop and Android are the right shape here: a
  build-configuration gate (the keyset links only in Debug) and a runtime door,
  plus a badge that answers to neither.
- The device sends are dust on one chain; every other chain is exercised by
  tests, not by money.
- A local notification is the phone's right channel for a confirmation that
  lands while the app is away, and iOS's discretionary scheduling is a fact to
  be described rather than a defect to be worked around.
- The founder performs SC-002 with a finger; everything else is driven over the
  cable.
- Text for every screen this cut turns live already exists in the corpus from
  spec 021. If one proves missing, it is added by the corpus procedure, never
  hard-coded.
