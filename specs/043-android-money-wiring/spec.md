# Feature Specification: Android Money Wiring — The Wallet Moves Money

**Feature Branch**: `043-android-money-wiring` (stacked on `042-android-merge-main`)

**Created**: 2026-09-12

**Status**: Draft

**Input**: User description: "Android Money Wiring — a signed-in person sends
one token to one address from the phone and watches it confirm. Mirrors web
026 (phases 1–4 and 6) and desktop 032 (phases 1–4 and 6). Enabling first:
the parallel space on Android — the core's fixed keyset behind a debug-only
gate, so every phase can be driven end to end over adb without a finger on
the fingerprint sensor; the real passkey path is the same code with one
substitution. Then the send machine in single mode … Not in scope:
split/sweep/batch, the camera scanner, dApp signing — those are 044–046.
Every phase is verified on the connected Xiaomi test device (founder's
rule)."

Program context: `docs/android/wiring-program-043-047.md`.

## Why

After 042 a person on Android sees their own money and cannot move it. The
send screens have been drawn since spec 021 — pick, form, confirm, receipt,
the fee-token and contact-pick sheets — and every rule behind them lives in
the core: the whole send controller as one machine (three modes, live
validation, string-exact Max, the same-asset fee ceiling, the treasury
pre-check, the sign→submit lifecycle with its cancel checkpoints), the fee
policy that prices it, and the tracker that follows it to a receipt. The
phone can already produce a passkey assertion (onboarding does it) and can
already reach the relay (the settings machine holds its endpoints). Nothing
is missing but the wiring: three executors and a flow host that reads the
core's view instead of a fixture.

**Why the parallel space comes first.** The founder's rule for this program
is that every feature is verified on the connected device. A send ends in a
biometric prompt, and a biometric prompt needs a finger. Desktop 032 solved
this in its first phase: the core's fixed keyset — the same scalars the web's
parallel space uses, deriving the same Safe — signs where a passkey would,
behind two gates, so every later phase could be driven and verified by a
machine. Android inherits the keyset for free and needs only the gates and
the door. Without it, every phase of this spec would end with "needs a
human"; with it, only the last one does.

**What is different on a phone.** Money in flight must outlive the screen:
the person will lock the phone, switch apps, or have the process killed
between submit and confirm, and the web's tab and the desktop's window are
gentler than that. So the pending row is written before tracking begins
(the core's ordering invariant), tracking resumes from storage on reopen,
and a confirmation that lands while the app is away is delivered as a system
notification — the one capability here the other shells never needed.

**Standing exclusions**: split and sweep modes, the payroll importer, the
camera scanner (the `Scan` screen keeps its fixture), and dApp signing are
044–046. Adding a network from the send flow (the machine offers it) is
deferred with the scanner, which is its only entry.

## User Scenarios & Testing *(mandatory)*

### User Story 0 - The parallel space on Android (Priority: P1, enabling)

A developer with the phone on a cable opens a debug build through its
developer door and gets the real app — real chains, real relay, real
storage, every screen — signed for by the core's fixed keyset. The wallet it
shows is the one the web's and the desktop's parallel spaces show, with the
same address and the same balance. A release build has no such door and no
such keys.

**Why this priority**: it is what lets every other story be verified on the
device without a finger, and it is the founder's own test environment on a
third client.

**Independent Test**: open the door; the address on the receive screen is
the fixture Safe the other clients derive; the badge that marks the space is
visible on every screen; a release build's package contains neither the
door nor the keys.

**Acceptance Scenarios**:

1. **Given** a debug build, **When** the developer door is opened, **Then**
   the app signs in as the fixture account and every screen carries the
   parallel-space badge.
2. **Given** the parallel space, **When** a send reaches the signing step,
   **Then** no biometric prompt appears and the assertion the relay
   receives verifies on chain.
3. **Given** a release build, **When** its package is inspected, **Then** no
   fixture key material and no developer door are present.

---

### User Story 1 - Send a token to someone (Priority: P1)

A signed-in person taps 发送, picks a token they hold, types or pastes an
address (or picks a contact), enters an amount or taps Max, continues, reads
a confirmation that states the recipient, the amount, the fee and the fee
token, confirms, and lands on a receipt that first says *submitted* and then
*confirmed*, with a link to the explorer. The home feed shows the row the
moment it is submitted.

**Why this priority**: it is the program's reason to exist, and the
founder's revenue model rides on transactions.

**Independent Test**: in the parallel space, send dust from the fixture Safe
to a known address; the receipt reaches *confirmed*, the explorer shows the
transaction, the feed row appears at submit and flips at confirm.

**Acceptance Scenarios**:

1. **Given** the token list, **When** a token is picked, **Then** the form
   opens with that token, its balance and its chain, and the amount field
   validates as it is typed.
2. **Given** a valid recipient and amount, **When** Continue is tapped,
   **Then** the fee quote and the relay pre-check run together, and the
   confirm screen shows exactly the fee that will be signed.
3. **Given** the confirm screen, **When** the person confirms and the
   assertion succeeds, **Then** the relay accepts the operation, the receipt
   shows *submitted* with a hash, and the feed shows the pending row.
4. **Given** a submitted send, **When** the receipt lands, **Then** the
   receipt and the feed row both say *confirmed* (or *failed*, with the
   reason) and the balance refreshes.
5. **Given** Max on a token that also pays the fee, **When** the amount is
   filled, **Then** it is the balance less the fee reserve, to the last
   unit, and the send succeeds.

---

### User Story 2 - Money in flight outlives the screen (Priority: P1)

The person confirms a send, locks the phone, and comes back a minute later.
The receipt is still there, now *confirmed*. If they had killed the app
instead, the feed still shows the row and it still reaches *confirmed*. If a
confirmation lands while they are in another app, the phone tells them.

**Why this priority**: a send that vanishes when the screen does is money the
person believes is lost.

**Independent Test**: submit in the parallel space, force-stop the process,
reopen: the pending row is there; wait: it confirms; the notification
arrived while the app was stopped.

**Acceptance Scenarios**:

1. **Given** a send submitted, **When** the process is killed and the app
   reopened, **Then** the pending row is in the feed and tracking resumes
   without any action from the person.
2. **Given** a submitted send and the app in the background, **When** the
   receipt lands, **Then** a system notification names the outcome and opens
   the receipt when tapped.
3. **Given** a send whose receipt reports failure, **When** the feed is
   read, **Then** the row says *failed* and the reason the core gives, never
   *pending* forever.

---

### User Story 3 - The screen says what the core refuses (Priority: P2)

Every refusal the core has a word for reaches the screen as that word: an
amount above the balance, an amount that leaves nothing for the fee, an
address that is not one, a chain with no relay coverage, a relay whose
treasury cannot front the fee, a quote that could not be obtained, a submit
the relay rejected, a cancel at any checkpoint. None of them is a spinner
that never stops, and none of them is a second biometric prompt.

**Why this priority**: 041 and 040 both found that the costly bugs on this
client were silences, not crashes.

**Independent Test**: drive each refusal in the parallel space and read the
screen; cancel at confirm and at the assertion and count the prompts.

**Acceptance Scenarios**:

1. **Given** an amount above the balance, **When** it is typed, **Then** the
   form says so inline and Continue is disabled.
2. **Given** a relay whose treasury is empty for this chain, **When**
   Continue is tapped, **Then** the sheet the core prescribes opens with its
   guidance, and nothing is signed.
3. **Given** the assertion in progress, **When** the person cancels,
   **Then** the send returns to confirm and no second prompt appears —
   ever, for that attempt.
4. **Given** a relay rejection at submit, **When** it arrives, **Then** the
   receipt says *failed* with the relay's reason and the balance is
   untouched.

---

### User Story 4 - The sheets the flow needs, and the arms 042 left (Priority: P2)

The fee-token sheet lists the fee coins the chain accepts with their
balances; the contact-pick sheet lists the person's own book and fills the
recipient; the add-token sheet adds a token by address and it appears in the
list and in the picker. Two arms marked "live in 042" answer for real: a
recipient that is a name resolves through the naming waterfall, and the
relay cache clears when the settings ask it to.

**Why this priority**: each is a drawn surface that today either shows a
fixture or does nothing, and each blocks a common send.

**Independent Test**: pick a different fee token and see the quote change;
pick a contact and see the address fill; add a token and send it.

**Acceptance Scenarios**:

1. **Given** the confirm screen, **When** the fee token is changed,
   **Then** the quote re-runs and the confirm shows the new fee in the new
   token.
2. **Given** the form, **When** a contact is picked, **Then** the recipient
   field carries that contact's address and name.
3. **Given** an address of a token the person holds, **When** it is added,
   **Then** it appears in the holdings and can be sent.

---

### Edge Cases

- A cancel at any checkpoint kills the pre-sign pipeline; a passkey prompt
  never resurrects after Cancel (the core's invariant, ported deliberately
  from a TypeScript flag that was written and never read).
- The quoted fee handed to submit is built from the very estimate the
  confirm screen rendered: displayed equals signed.
- The pending row is persisted before tracking starts; a process death
  between the two leaves a row, never a tracked ghost.
- A second tap on confirm while the first is in flight does nothing (the
  single-flight lock with generation tokens).
- A chain with no native coin pays its fee in a stablecoin; the fee-token
  sheet shows only what that chain accepts.
- A rate-limited endpoint mid-quote is *busy*, not *broken*: the quote
  retries through the pool, and the screen does not blame the chain.
- The parallel space disables relay sponsorship by design; the fixture Safe
  must hold the fee token, and the spec's device sends are dust.
- Hidden balance stays hidden on the send form and the receipt.
- The notification permission is asked once, at the first submit, never at
  launch; a refusal degrades to the in-app receipt only.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A debug build MUST offer a developer entrance to the parallel
  space; a release build MUST contain neither the entrance nor the fixture
  key material.
- **FR-002**: In the parallel space every signing step MUST be answered by
  the core's fixed keyset, deriving the same account the web and desktop
  parallel spaces show, and every screen MUST carry the space's badge.
- **FR-003**: Outside the parallel space the same signing step MUST be
  answered by the person's passkey through the existing assertion path,
  with no other difference in the flow.
- **FR-004**: The send flow in single mode MUST run on the core's send
  machine end to end: token pick, form validation, Max, Continue's parallel
  quote and relay pre-check, confirm, sign, submit, persist, track.
- **FR-005**: The fee shown on confirm MUST be the fee that is signed; the
  fee-token sheet MUST offer only the fee coins the chain accepts, each with
  its balance, and changing it MUST re-quote.
- **FR-006**: The pending row MUST be written to the activity store before
  tracking begins, and the home feed MUST show it from that moment.
- **FR-007**: Tracking MUST resume from storage when the app reopens, and a
  receipt landing while the app is away MUST surface as a system
  notification that opens the receipt.
- **FR-008**: Every refusal the core reports MUST reach the screen as the
  core's wording; no refusal may leave a spinner or a disabled button with
  no explanation.
- **FR-009**: A cancel at any checkpoint MUST end that attempt; no second
  biometric prompt may follow a cancel.
- **FR-010**: The contact-pick sheet MUST read the person's own book and
  fill the recipient; the add-token sheet MUST add a token that then
  appears in holdings and in the picker.
- **FR-011**: The two arms marked "live in 042" MUST answer for real:
  recipient name resolution and relay-cache clearing.
- **FR-012**: Every wire family this feature adds MUST pass the drift gate:
  operations, results and closed error families exhaustive, views subsets.
- **FR-013**: Every phase MUST be verified on the connected test device
  before it is called done, with a screenshot in the record; the results
  document MUST say for every success criterion whether it was device-
  verified or test-only.
- **FR-014**: No Kotlin may decide what the core already decides: fee math,
  validation, checkpoints, receipt classification and lock discipline stay
  in the core; the shell contributes transport, storage, the assertion, the
  clock and the notification.

### Key Entities

- **Send attempt**: one pass through the machine from token pick to a
  terminal receipt or a cancel; owns a generation token so a stale answer
  cannot land on a newer attempt.
- **Fee quote**: the estimate the confirm screen renders and the submit
  signs; carries the fee token, the amount, and its expiry.
- **Pending record**: the row written at submit, keyed by the operation
  hash, that the feed shows and the tracker resolves to *confirmed* or
  *failed*.
- **Parallel space**: the real app with one substitution, entered through a
  debug-only door, marked by a badge, deriving the shared fixture account.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001** (device): in the parallel space, dust leaves the fixture Safe
  from the phone and the explorer shows the transaction; the receipt reaches
  *confirmed* within the chain's normal confirmation time.
- **SC-002** (device, founder): the same send once from the founder's own
  passkey wallet, with exactly one biometric prompt.
- **SC-003** (device): submit, force-stop, reopen — the pending row is
  present and reaches *confirmed*; a notification arrived while stopped.
- **SC-004** (device): cancel at the assertion; the prompt count for that
  attempt is one and the flow returns to confirm.
- **SC-005** (device): each of the refusals in User Story 3 is driven and
  its wording read on screen; zero refusals end in a silent spinner.
- **SC-006** (device): change the fee token on confirm; the fee re-quotes in
  the new token and the send succeeds paying in it.
- **SC-007**: the flow host reads no send fixture; a grep for the send
  fixture builders in the live route returns zero.
- **SC-008**: the drift gate is exhaustive for the send, fee, tracker and
  manage-tokens families; unit tests grow and none are deleted.
- **SC-009**: the two "live in 042" markers are gone from the executors.
- **SC-010**: a release build's package contains no fixture key material.

## Assumptions

- The relay and its treasury behave as on the web: sponsorship is off in
  the parallel space, and the fixture Safe holds the fee token on at least
  one chain (the shared Gnosis Safe the other clients used for their dust).
- The two gates from the desktop are the right shape here: a compile-time
  gate (the keyset exists only in debug builds) and a runtime door, plus a
  badge that answers to neither.
- The device sends are dust on one chain; every chain the core supports is
  exercised by tests, not by money.
- A system notification is the phone's right channel for a confirmation
  that lands while the app is away; permission is asked at first submit.
- The founder performs SC-002 with a finger; everything else is driven over
  the cable.
- Text for every screen this feature turns live already exists in the
  corpus from spec 021; no new corpus keys are expected. If one proves
  missing, it is added by the corpus procedure, not hard-coded.
