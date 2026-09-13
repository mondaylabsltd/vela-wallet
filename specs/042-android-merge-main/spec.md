# Feature Specification: Android Merge Main — The Android Line on the Post-Expo Base

**Feature Branch**: `042-android-merge-main` (stacked on `041-android-read-wiring`; carries `origin/main` at `aa91fab4`, the spec 039 merge)

**Created**: 2026-09-12

**Status**: Draft

**Input**: User description: "Bring the Android line onto the post-Expo base.
Merge origin/main (spec 039 retire-expo-tree, with the #188/spec-038 balance
behaviours and the 028 contacts I/O it carries) into the 041 Android
read-wiring line without losing either side: from 041 the working 通讯录 tab,
the live wallet body and the row-id flow entry; from main 探索 as a section of
the wallet route, the assets/i18n and docs/design paths, and the held-figure /
unreachable balance rules. Migrate the Kotlin unit tests from public/i18n to
assets/i18n, mirror the new BalanceView.unreachable field on Android so an
errored first load says unreachable rather than $0.00, rebuild the web wasm
fingerprint and the Kotlin bindings, and leave every CI gate (app, rust,
android) green on the merged tree."

## Why

The Android line branched from `main` at `28d25ae9` and has since taken
twenty-eight commits of its own (specs 040 and 041). In the same window `main`
took one hundred and ninety-nine: spec 039 retired the React Native / Expo tree
and moved the locale catalogs from `public/i18n` to `assets/i18n` and the design
sources under `docs/design`; specs 028 and 038 and issue #188 changed what the
shared core says about a balance (the figure holds still while chains stream
in; an errored first load is *unreachable*, never `$0.00`; Celo's native coin
is not counted twice) and what a contact book can do (import, export, A–Z
sections). None of that reaches a person on Android until the two lines meet.

The two lines touch the same five files, and three of them conflict for real:
the wallet route (main made 探索 a section of it; 041 made its body live and let
the 通讯录 tab navigate), the balance module (both sides ported the same
custom-token pricing rule independently), and the feature pointer. A merge
that picks a side loses a screen. This feature is the merge that keeps both,
plus everything the merge alone cannot do: the paths the Android tests read
from, the wire mirrors main widened, the artefacts whose fingerprints cover the
Rust that changed, and the record of what was decided.

**Standing exclusions**: sending money, signing, the dApp browser and the live
explore tab remain future Android work. Spec 041 pointed at "spec 042" for
them; that pointer now means the next number, and this spec says so rather than
leaving two documents that disagree.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Nothing 041 gave me is taken back (Priority: P1)

A signed-in person opens the app after the base moves and sees exactly what
they saw the day before: their own holdings and total, their own activity, a
通讯录 tab that opens *their* address book, and a tapped activity row that
opens *that* row's detail rather than somebody else's.

**Why this priority**: the merge exists to add, and a merge that regresses a
verified device result (spec 041 results.md, 2026-09-05) is worse than no
merge.

**Independent Test**: build the merged tree, sign in on the device used for
041, open the home: the total matches the block explorer, the feed is this
account's, 通讯录 shows the saved contacts, tapping the POL row opens the POL
transaction.

**Acceptance Scenarios**:

1. **Given** a signed-in account with holdings on two chains, **When** the home
   opens, **Then** the hero shows this account's total and the list its own
   holdings, in the display currency chosen in settings.
2. **Given** the home is showing, **When** the 通讯录 tab is tapped, **Then**
   the address book of this device opens, not a fixture.
3. **Given** two activity rows, **When** the second is tapped, **Then** the
   detail that opens is the second row's.
4. **Given** every unit test that passed on 041, **When** the suite runs on the
   merged tree, **Then** every one of them still passes.

---

### User Story 2 - Everything main learned reaches the phone (Priority: P1)

The same person also gets what the shared core and the other clients gained
while the Android line was away: 探索 is a real tab that swaps the wallet body
and that Back leaves for 钱包 rather than for Welcome; a refresh does not make
the total climb through partial sums; a first load that cannot read anything
says so instead of showing zero; the add-network wizard can report that a
chain's checks failed; a Celo holding is counted once.

**Why this priority**: these are the behaviours the founder asked for on the
other clients (#188, spec 038 findings) and there is one core. Two clients
that disagree about what a balance is would be a defect in the core's
promise, not a difference in taste.

**Independent Test**: cut the network before first sign-in on a fresh install:
the hero shows a skeleton with a reason, never `$0.00`. Restore the network and
pull to refresh with two chains answering seconds apart: the figure changes
once. Tap 探索, then Back: the wallet body returns and the app is still signed
in. Add a network whose checks fail: the wizard says so rather than crashing
or hanging.

**Acceptance Scenarios**:

1. **Given** a fresh install with no cached balance, **When** the first fetch
   fails on every chain, **Then** the hero is a skeleton with an explanation
   and shows no number.
2. **Given** a settled total, **When** a refresh is in flight and chains answer
   one by one, **Then** the displayed figure holds until the refresh settles
   and then changes at most once.
3. **Given** the wallet body is showing, **When** 探索 is tapped and then the
   system Back is pressed, **Then** the wallet body returns and no sign-out
   happens; **When** the device is rotated in 探索, **Then** 探索 is still the
   body afterwards.
4. **Given** the add-network wizard is open, **When** the core answers that the
   chain's checks failed, **Then** the wizard renders that verdict; no message
   the core can send is one the phone cannot decode.
5. **Given** a Celo account holding the native coin, **When** the balance walk
   runs, **Then** the holding appears once and the total counts it once.

---

### User Story 3 - The gates are green on the merged tree (Priority: P2)

Whoever opens the pull request sees every check pass: the tooling gate, the
Rust gate (format, lint, tests, the web artefact's source fingerprint, the
generated wire types), and the Android gate (compile and unit tests against
freshly generated bindings). Nothing is skipped and nothing is marked
"known red".

**Why this priority**: a green suite is how the next person knows the merge
kept its promise without re-doing the device test; and spec 039's own gates
(no Expo residue, native reachability) exist to catch exactly the kind of path
drift a merge across it introduces.

**Independent Test**: run each gate locally as CI would; every exit code is
zero.

**Acceptance Scenarios**:

1. **Given** the Android unit tests read locale catalogs from disk, **When**
   they run on the merged tree, **Then** they read from the directory main
   moved them to and pass.
2. **Given** Rust sources changed under the Android line, **When** the web
   artefact's fingerprint check runs, **Then** it matches, because the
   artefact was rebuilt from the merged sources.
3. **Given** the Kotlin wire mirrors, **When** the drift test compares them to
   the generated types, **Then** every exhaustive family (operations, results,
   the wizard's error kinds) covers every variant the core has.

---

### User Story 4 - The record says what was decided (Priority: P3)

A reader six months on can see, in one place, which conflicts the merge had,
how each was resolved and why, what had to change beyond the conflicts, and
which of main's behaviours were wired on Android versus recorded as still
owed.

**Why this priority**: the repository's handover documents are the authority
for the next person; a merge with real conflict resolutions that is not
written down is a decision nobody can audit.

**Independent Test**: read `specs/042-android-merge-main/results.md`; every
conflict file is named with its resolution; every deviation from "pure merge"
is listed with a reason.

**Acceptance Scenarios**:

1. **Given** the results document, **When** it is read against the merge
   commit, **Then** every conflicted file in the commit is accounted for.

---

### Edge Cases

- A balance that is unreachable but has a cached total from a previous run is
  *not* unreachable: the cached figure shows, with the stale notice.
- The wizard's error kind the phone has never seen must fail loudly in a test,
  not silently at runtime; the drift gate is exhaustive for that family on
  purpose.
- 探索 is a section, not a route: a deep link cannot land there before a
  wallet exists, and signing out from settings must clear it.
- The 通讯录 tab is reachable from 探索 as well as from 钱包; Back from the
  address book returns to whichever body was showing.
- Both sides ported the same custom-token pricing rule with the same semantics;
  the merged tree keeps one copy and the four tests written for it.
- Locale catalogs on disk moved directory; nothing on Android may still name
  the old one, in code or in build wiring.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The merged tree MUST keep every 041 behaviour verified on the
  device: live holdings and total, live activity feed, the 通讯录 tab opening
  this device's book, and row-identified flow entry.
- **FR-002**: The merged tree MUST keep every main behaviour that reaches the
  Android shell: 探索 as a section of the wallet route with Back and rotation
  semantics; the held figure during refresh; *unreachable* instead of zero on
  an errored first load; the wizard's "checks failed" verdict; Celo's native
  coin counted once.
- **FR-003**: Every Kotlin wire family the drift gate checks exhaustively
  MUST carry every variant the core has on the merged tree; view mirrors MAY
  be subsets but MUST NOT name a field the core lacks.
- **FR-004**: The wallet hero MUST render the core's *unreachable* verdict as
  a skeleton with an explanation and never as a number.
- **FR-005**: Every Android unit test that reads locale catalogs from disk
  MUST read them from `assets/i18n`; no Android source or build file may name
  `public/i18n`.
- **FR-006**: The committed web artefact MUST carry the source fingerprint of
  the merged Rust; the Kotlin bindings MUST be regenerated from the merged
  core before the Android gate runs.
- **FR-007**: The tooling, Rust and Android CI gates MUST pass on the merged
  tree with no step skipped or waived.
- **FR-008**: The feature MUST record, in `results.md`, each conflicted file
  with its resolution, and each change beyond the conflicts with its reason
  and whether it wires a main behaviour or records it as owed.
- **FR-009**: Spec 041's forward pointer ("spec 042" for send, sign, browser
  and live explore) MUST be reconciled in the record so the two documents do
  not disagree.

### Key Entities

- **Merge commit**: the single commit that joins `origin/main` at `aa91fab4`
  to the Android line at `d35ab422`; its three conflicted files and their
  resolutions are the heart of the record.
- **Wire mirror**: a Kotlin declaration of a core type; either a subset
  (views) or exhaustive (operations, results, closed error families).
- **Balance verdict**: the core's description of what a total is — known,
  partial, unknown, hidden, or unreachable — which the hero must render
  without blurring.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every unit test that passed on 041 passes on the merged tree;
  zero tests deleted or disabled to achieve it.
- **SC-002**: The drift gate reports zero missing variants across every
  exhaustive family and zero unknown fields across every view.
- **SC-003**: The web artefact fingerprint check, the Expo-residue check and
  the native-reachability check each exit zero on the merged tree.
- **SC-004**: On the device used for spec 041, the four home checks (total,
  feed, 通讯录, row detail) match the 2026-09-05 result, and the three main
  checks (探索 round trip, held figure, unreachable skeleton) pass.
- **SC-005**: A reader can list every conflicted file and its resolution from
  `results.md` alone, without opening the merge commit.

## Assumptions

- "042" holds the merge, by the founder's instruction on 2026-09-12; spec
  041's pointer to "spec 042" for send/sign/browser/explore therefore means
  the next Android number, and the record says so.
- Main's behaviours that live entirely in the core (the held figure, the
  switcher figure) reach Android by the merge alone; only the ones that need a
  shell reading (unreachable, the wizard verdict, Celo) need Android changes.
- The Celo rule is honoured by the shell's balance walk asking the core;
  whether the Android walk currently lists a wrapped-native slot at all is
  established during implementation and recorded either way.
- The web gate (`pnpm build`) is main's concern and is unchanged by this
  feature beyond the rebuilt artefact; it is run locally once as a courtesy,
  not owned here.
- Device verification uses the Xiaomi test device from 041; no new device is
  introduced.
