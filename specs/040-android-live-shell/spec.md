# Feature Specification: Android Live Shell — Settings & Contacts on the Core

**Feature Branch**: `040-android-live-shell`

**Created**: 2026-09-05

**Status**: Draft

**Input**: User description: "在 app-android/vela-wallet 中接入 vela-core 业务逻辑。大部分页面/UI 组件已画好；有一部分可能缺少业务逻辑需要在 vela-core 重写（通讯录、explorer、clear signing）；用 GitHub speckit 工作方式，从 040 开始（02x/03x 留给同事）；代码简洁、高内聚、低耦合、可复用；理解产品与 UI/UX、交互、用户体验以及产品的核心价值；用 git worktree 基于 origin/main 开一个新工作区。"

## Why

Every business rule this wallet has already lives in Rust. `vela-core` on
`origin/main` carries **24 Crux machines, 39,630 lines** behind the `crux`
feature. The Android client runs **three** of them — create, login, session
(spec 019). Everything else a signed-in person touches on Android is a
**fixture**: a picture that forgets. `VelaNavHost` hands `WalletFixtures`,
`FlowFixtures`, `ExploreFixtures`, `SigningFixtures`, `ContactsFixtures` and
`SettingsFixtures` to six drawn surfaces, and not one of them reads the
person's own wallet.

**A correction to the premise of record.** The feature request assumed the
address book, the explorer and clear signing still need their logic written in
`vela-core`. They do not — verified on `origin/main`, not from memory:

| Machine | File | Lines | `impl App` |
| --- | --- | --- | --- |
| Contacts | `rust/crates/vela-core/src/app/contacts.rs` | 1,385 | ✅ |
| Clear signing | `…/clear_signing.rs` | 4,841 | ✅ |
| Explorer history | `…/browser_history.rs` | 471 | ✅ |
| dApp session | `…/dapp_session.rs` | 1,959 | ✅ |
| dApp permissions | `…/dapp_permissions.rs` | 1,341 | ✅ |
| Networks | `…/network_admin.rs` | 2,900 | ✅ |
| Display currency | `…/display_currency.rs` | 413 | ✅ |

So **no business rule is written on Android by this feature, or by the two
that follow it.** What Android is missing is a *shell*: for each machine, one
executor that performs side effects and one builder that turns the core's view
into the display models the drawn components already accept. That is the whole
job, and it is why this spec can be small while covering a surface this wide.

**Why these two screens first.** Settings and Contacts are governed by machines
that need **storage only — zero network**. Choosing them lets this feature pay
the one-time cost of the *paved road* (bridge, effect loop, key-value store,
wire-type drift gate) while the only genuinely new subsystem is a local
database. Spec 041 (read path: balances, activity, receive, tokens) and spec
042 (money path: send, signing, explorer) each add exactly one further class of
infrastructure on top of what this feature proves — the same three-step shape
web took in 024/025/026.

**The one cost that is genuinely new on Android.** The web bridge is a wasm
module already in the bundle; the desktop bridge is an in-process Rust call
that costs nothing. Android's bridge is `uniffi`, and every machine exported
across it is compiled code shipped in three ABIs. Spec 019 recorded the price
of the first three at **+785,864 stripped bytes (arm64-v8a)**. Adding the rest
must therefore be *measured before it is committed to*, not estimated after —
this feature's first task is a size probe, and its budget is a success
criterion.

**Standing scope note**: this feature moves no pixels. Every screen it makes
live is already drawn and already has a preview gallery. Where a drawn control
cannot express what the core allows — Android currently renders ten
`VelaUrlField`s that display a value and cannot be typed into — this feature
gives that control its input, because a settings page whose network form
cannot be filled in is not "wired", it is decorative.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Settings that remember (Priority: P1)

A signed-in person opens Settings, picks the currency their balances are quoted
in, adds a custom network, edits its RPC, removes it — and every one of those
choices is real. It survives leaving the screen, it survives the app being
killed, and what the screen shows afterwards is what the wallet core ruled, not
a staged picture.

**Why this priority**: Settings is the cheapest full-chain proof that a drawn
Android screen can go live — the route, the tab bar, the identity overlay and
the entire visual surface already exist (spec 023). Proving the pipeline here
de-risks every later screen, and "my preferences persist" is the first
believable signal to a person that this is an application and not a demo.

**Independent Test**: Launch the app on a device with a wallet, open Settings,
change the display currency, add + edit + remove a custom network, force-stop
the app between each step; every state reads back exactly as entered. No
network request is made by any of these actions.

**Acceptance Scenarios**:

1. **Given** a signed-in wallet, **When** the person selects a display currency
   from the currency sheet, **Then** the sheet closes with that currency shown
   as chosen, and after a force-stop and relaunch it is still chosen.
2. **Given** the add-network page, **When** the person types a chain id, a name
   and an RPC URL and confirms, **Then** the network appears in the networks
   list and survives a relaunch.
3. **Given** a custom network in the list, **When** the person opens it and
   edits its RPC URL, **Then** the new URL is what the detail page shows on the
   next visit.
4. **Given** a custom network, **When** the person removes it and confirms,
   **Then** it is gone from the list and stays gone after a relaunch.
5. **Given** a network form the core rejects (an id that collides with a
   built-in chain, a malformed URL), **When** the person confirms, **Then** the
   screen shows the core's refusal and writes nothing.

---

### User Story 2 - An address book that is actually mine (Priority: P2)

A person opens Contacts and sees the people *they* saved — grouped by letter,
searchable, with the groups they made — instead of six strangers. Opening a
contact shows that contact's name, address and groups. Deleting one deletes
that one.

**Why this priority**: Contacts is the second machine on the same road, which
is what makes the road worth paving: if the second machine costs an executor
and a builder and no new plumbing, the pattern is proven for the twenty that
follow. It also retires the most embarrassing thing an early tester can find —
an address book that shows other people's names.

**Independent Test**: With a wallet established, add a contact through
whichever writing surface exists, then relaunch: the list, the letter sections,
the search and the detail page all read from the stored book. Delete it and it
stays deleted.

**Acceptance Scenarios**:

1. **Given** a device with no contacts saved, **When** the person opens
   Contacts, **Then** they see the empty state, not fixture people.
2. **Given** saved contacts, **When** the person types in the search field,
   **Then** the list filters by the core's matching rule and the letter rail
   still lands on the right sections.
3. **Given** a saved contact, **When** the person opens it, **Then** the name,
   the full address, the identicon seed and the groups shown are that
   contact's — and the delete and copy actions act on that same contact.
4. **Given** a saved contact, **When** the person deletes it and confirms,
   **Then** it is gone after a relaunch.
5. **Given** contacts saved by the same wallet on another client, **When** the
   person opens Contacts on Android, **Then** they are there — the stored
   shapes are the ones the other clients read and write.

---

### User Story 3 - One shell, twenty-one machines still to come (Priority: P3)

A developer adds the next machine to Android and writes an executor and a
display-model builder — nothing else. No new bridge code, no new effect loop,
no new storage layer, no new type-safety scheme.

**Why this priority**: it is the difference between this feature and a rewrite.
The plumbing built here is used unchanged by every machine in 041 and 042; if
it is not reusable, those specs pay for it three times.

**Independent Test**: The third machine in this feature (`display_currency`)
is added with no changes to any shared file other than one registration line
and its own two files, and the diff proves it.

**Acceptance Scenarios**:

1. **Given** the paved road exists, **When** a machine is added, **Then** the
   change touches: one bridge registration, one executor, one builder, one
   test file — and no shared plumbing.
2. **Given** a wire type renamed in Rust, **When** the Android unit tests run,
   **Then** they fail, naming the field — a rename cannot silently degrade into
   a default value on a screen.

---

### Edge Cases

- **The store is unreadable or corrupt.** A truncated or hand-edited record
  reads as "nothing saved", never as a crash and never as a partial book with
  half the addresses missing.
- **A write fails** (storage full, permission revoked): the core is still
  answered, so the screen reports what happened instead of freezing on a
  request nobody will ever answer.
- **The app is killed mid-write.** The next launch sees either the old value or
  the new one, never a half-written record.
- **Two writers.** A screen re-entered while a write is in flight must not
  interleave views: the order the person sees is the core's order.
- **The same wallet on another client.** Keys and record shapes are the ones
  web and desktop already use; a person who saved contacts on the web opens
  Android and finds them.
- **Process death and rotation.** The core, not the composable, is the source
  of truth; a rotation mid-form does not lose what was typed or resurrect a
  dismissed sheet.
- **1.35× text scale and the dark theme.** Every state this feature makes live
  still renders at the accessibility scale the galleries pin.
- **A machine exported across the bridge but never rendered** must not cost
  binary size silently: the size delta is measured per slice and recorded.

## Requirements *(mandatory)*

### Functional Requirements

**The bridge**

- **FR-001**: The bridge MUST carry the three machines this feature wires
  (`contacts`, `network_admin`, `display_currency`) in addition to the three it
  already carries, using the existing registration mechanism — one line per
  machine, no per-machine bridge code.
- **FR-002**: The binary-size cost of the added machines MUST be **measured**
  on `arm64-v8a`, stripped, before and after, and recorded in the feature's
  results. A measurement that exceeds the recorded budget is a decision point,
  not a silent commit.
- **FR-003**: The shell MUST NOT re-derive any bridge semantics: effect ids,
  unknown-id handling and cancellation stay exactly as spec 019's contract
  defines them for all four clients.

**The paved road**

- **FR-004**: The Crux plumbing currently living under the onboarding feature
  (driver, bridge abstraction, JSON shell) MUST be promoted to a shared,
  feature-neutral location and reused unchanged by the new machines. Onboarding
  MUST keep working through the promoted code, not a copy.
- **FR-005**: A single key-value storage layer MUST serve every machine that
  needs one, with **key names and record shapes byte-compatible** with the web
  and desktop clients.
- **FR-006**: Every executor MUST obey the four shell rules already normative
  for the other clients: answer every operation exactly once; never throw a
  failure the core has a variant for; hold no business `if`; match the
  operation set exhaustively so a Rust change is a **compile** error.
- **FR-007**: A machine's long-lived state MUST outlive the screens that show
  it, so that leaving a screen and coming back does not re-run its boot or
  flash a loading state over settled data.

**Type safety**

- **FR-008**: Every wire shape Android parses MUST be checked against the
  generated mirrors of the Rust types committed in the repository, by a test
  that fails on a renamed, removed or newly-required field.
- **FR-009**: Parsers MUST be tolerant of fields they do not need and
  intolerant of values they do not understand — an unknown enum variant fails
  loudly rather than rendering as a default.

**Settings**

- **FR-010**: The settings surface MUST read its display currency, its custom
  networks, its network configurations, its RPC providers and its service
  endpoints from the core, and write every change back through the core.
- **FR-011**: Text inputs the core needs in order to accept a change MUST be
  typable — at minimum the add-network form, the network-detail RPC and
  explorer fields, the RPC-provider and service-endpoint fields, and the search
  fields that filter them.
- **FR-012**: A change the core refuses MUST surface the core's own refusal on
  the screen, and MUST NOT be written.
- **FR-013**: Surfaces that need the network layer this feature does not build
  (RPC health, latency, chain probes, fiat rates, storage figures) MUST remain
  visibly honest — they show their unknown state, marked in code as pending
  spec 041, and MUST NOT display a fixture value as if it were measured.

**Contacts**

- **FR-014**: The contacts surface MUST read the person's own contacts,
  groups and dismissal state from the core: list, letter sections, search
  results, group rows, contact detail and group detail.
- **FR-015**: Every action the drawn surface can reach and the core can honour
  MUST act on the entity the screen is showing — with the identity of the
  target taken from the model that was rendered, never from a separately-held
  index.
- **FR-016**: Actions the core supports but no drawn control can reach yet
  (an add/edit form, favourites) MUST be recorded as missing **artwork**, not
  silently absorbed into this feature's scope, and MUST NOT be faked with a
  fixture.

**Honesty of the whole**

- **FR-017**: After this feature, no signed-in route MUST reach a fixture for
  the Settings or Contacts surfaces. Developer galleries keep their fixtures
  and stay reachable only through the developer entry points.
- **FR-018**: This feature MUST make no network request from any screen it
  wires, and that MUST be verifiable.
- **FR-019**: No business rule (validation, ordering, grouping, formatting
  decisions, refusal logic) MUST be written in Kotlin. Where the shell appears
  to need one, the rule is in the core already or the gap is recorded for the
  core.

### Key Entities

- **Machine**: one Crux state machine in `vela-core` — the sole owner of a
  rule. Exposed to Android over the bridge; identified by name.
- **Operation**: something a machine asks the shell to do (read a key, write a
  key, start a timer). Answered exactly once, with a result the machine defines.
- **Executor**: the Kotlin half that performs operations for one machine and
  holds no rules.
- **Display model**: the already-existing Kotlin data classes the drawn
  components accept. A *builder* turns one core view into one display model.
- **Store**: the on-device key-value space, shared with the other clients by
  key name and record shape.
- **Wire type**: a Rust type that crosses the bridge; its generated mirror in
  the repository is the reference the drift gate checks Kotlin against.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A display currency chosen on the device is still chosen after the
  app is force-stopped and relaunched — verified on a real device.
- **SC-002**: A custom network can be added, edited and removed on the device,
  each step surviving a force-stop; a network the core refuses is refused on
  screen and absent from storage.
- **SC-003**: Contacts, groups and contact detail render the device's own
  stored book; with an empty store the person sees the empty state, and no
  fixture person appears on any signed-in route.
- **SC-004**: Deleting a contact deletes the contact that was on screen —
  demonstrated with at least three saved contacts, deleting the one that is
  neither first nor last.
- **SC-005**: Zero network requests are issued by the Settings and Contacts
  surfaces, demonstrated by inspection of the executors' operation coverage and
  by a device run with the network disabled behaving identically.
- **SC-006**: The added bridge machines' stripped `arm64-v8a` size delta is
  measured and recorded, with the per-machine average reported so specs 041 and
  042 can budget from data instead of estimate.
- **SC-007**: Adding the third machine touches no shared plumbing file beyond
  one registration line — shown as a diff in the results.
- **SC-008**: Renaming a field in a Rust wire type turns an Android test red;
  demonstrated once, deliberately, and recorded.
- **SC-009**: The existing Android test suite (118 tests on this base) stays
  green, and the suite grows by the tests this feature adds; `assembleDebug`
  and the unit-test task both pass from a clean checkout after the documented
  binding-generation step.
- **SC-010**: Every gallery state that existed before this feature still
  renders after it — the fixture galleries are not collateral damage of going
  live.

## Assumptions

- **The core is complete for these three machines.** No `vela-core` change is
  expected. If one proves necessary, it is a rule, it goes in Rust, and it
  brings the repository's regeneration gates with it.
- **A person reaching Settings or Contacts is signed in.** Both surfaces sit
  behind the session guard that already exists; this feature does not change
  routing.
- **Storage is device-local and unencrypted at the app level**, exactly as the
  account list already is — the wallet's secrets live in the platform
  authenticator, not in this store.
- **Contact writing surfaces are limited by artwork, not by the core.** Whatever
  add/edit affordance is missing on Android is missing as a drawing; it is
  recorded, not invented here.
- **The base is `origin/main`.** The Android navigation repair that made the
  explore and signing surfaces reachable lives on an unmerged branch and is
  therefore *not* present on this base; those two surfaces stay unreachable
  until spec 042 wires them, and this feature does not change that.
- **Out of scope, by construction**: balances, activity, receive, tokens (041);
  send, signing, clear signing, the explorer and dApp sessions (042); the
  camera; any change to the drawn visual language.
