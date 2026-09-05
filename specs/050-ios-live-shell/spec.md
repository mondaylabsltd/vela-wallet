# Feature Specification: iOS Live Shell — Settings and Contacts on the Core

**Feature Branch**: `050-ios-live-shell` (based on `origin/main` @ `28d25ae9`)

**Created**: 2026-09-05

**Status**: Draft

**Input**: Founder description: "在 app-ios/VelaWallet 中接入 vela-core 业务逻辑。大部分页面已经画好；保持代码简洁、高内聚、低耦合、可复用；理解产品与 UI/UX，而不只是接线。"

## Why

iOS has had a live onboarding spine since spec 019: create, sign in, session, route
guard, sign-out, and three authenticator families driven by real Crux machines. Past
that point the app is 25,423 lines of Swift over **fixtures** — the home, the address
book, the settings surface, Explore, the signing sheet and the four wallet flows.

The client is in the worst version of that state, and it can be pointed at exactly.
This is the whole of the signed-in shell's navigation:

```swift
// App/RootView.swift — the wallet's onSelectTab, in full
if tab == .settings { router.path.append(.settings) }
```

Four tabs are drawn. **One of them goes anywhere.** 通讯录 and 探索 are both in the
tab bar, both tappable, and both do nothing at all. Behind 通讯录 sits `contacts.rs`
— 1,385 lines of decided behaviour, already exported to the web client, already
driving a shipped address book there.

This is the **third** client to take this cut, and both predecessors took it first for
the same reason. Web's 024 and desktop's 030 wired settings and contacts before
anything else because it is the only layer that can be finished without inventing a
network stack: three machines, one JSON store, no chain reads. The plumbing gets
proven where a bug costs a wrong label rather than a wrong balance.

### What iOS starts with that web and desktop had to build

| Web 024 / desktop 030 had to invent | iOS already has | Where |
|---|---|---|
| an effect loop with correlation + cancellation rules | `CoreDriver` — generic, `@MainActor`, 154 lines, already driving three machines | `Features/Onboarding/Core/CoreDriver.swift` |
| a shell-side thread model (`Answer::{Now,Blocking,After}` on desktop) | `async`/`await` — `perform` suspends, `Task.sleep` is the timer | — |
| a KV store with a cross-client key contract | `AccountStore` — `UserDefaults`, `vela.*` keys, records kept **whole** | `Features/Onboarding/Core/AccountStore.swift` |
| a fetch primitive with timeout + error classification | `RegistryClient` — `URLSession`, 408 lines, already talking to production | `Features/Onboarding/Core/RegistryClient.swift` |
| a settings surface to un-swallow | ST1–ST16 + SR1–SR5 drawn: networks, network detail, add-network wizard (**with 兼容/不兼容 states**), RPC providers, endpoints, storage, about, and every picker sheet | `Features/Settings/` (2,186 lines) |

### What iOS must build that neither predecessor did

| | Why iOS is different |
|---|---|
| **Bridge exports.** 22 machines reach wasm via `bridge_class!`; only **3** reach Swift via `bridge_object!` (`CreateWalletCore`, `LoginCore`, `SessionCore`). | uniffi cannot export a generic object, so every machine costs one macro line — and, unlike wasm, a regenerated `.xcframework` and a regenerated committed `vela_core_uniffi.swift`. |
| **Typed views without a generator.** Web reads `ts-rs`-generated types; desktop reads Rust structs directly. Swift gets **JSON**, and `ts-rs` has no Swift backend. | The bridge hands `[String: Any]`. Thirty view models read by subscript is thirty silent `nil`s waiting to happen. |
| **Residency.** `CoreDriver` today is created and disposed with a screen. | A settings machine that dies when the sheet closes re-probes the network on every open. |

### The residue

Four things, and no more: **the bridge exports**, **a resident core store**, **three
executors over one storage and one HTTP primitive**, and **a live display-model seam**
so `*Fixtures.swift` stays the gallery's canon.

## Design Authority

This feature draws **one** new control and no new screen.

- **Contacts**: `design/contacts/` — C1–C6 and the two SPEC 动效 sheets, the same
  authority spec 018 built against.
- **Settings**: `design/settings/` was referenced by spec 023 and **was never
  committed** (verified: `git ls-tree origin/main design/` lists contacts, icon,
  onboarding, onboarding-new, wallet). The surviving authority is therefore
  `Features/Settings/SettingsFixtures.swift` (784 lines, ST1–ST16 + SR1–SR5) and the
  gallery that renders it — which is what 023 actually shipped, and what every
  screenshot sweep compares against.
- **Porting authority**: web's 024 at `origin/main`, whose executors are these same
  operations answered against different I/O:

| Web source | Lines |
|---|---|
| `settings/core/network-admin-executor.ts` | 568 |
| `settings/core/currency-executor.ts` | 73 |
| `settings/live.ts` | 463 |
| `contacts/core/contacts-executor.ts` | 276 |
| `contacts/live.ts` | 216 |
| `core/effect-loop.ts` + `core/json-shell.ts` | 126 + 44 |
| `services/storage.ts` | 95 |

### The one new control, and why it closes a gap rather than making a design decision

The add-network wizard is drawn in full — ST10, ST10b (兼容), ST10c (不兼容) — and
contains **no editable field**: `grep -rn "TextField" Components/Settings Features/Settings`
returns nothing. Every string on it is a fixture constant. So the wizard can be shown
and cannot be used, and `network_admin`'s 16 operations have no way to receive a
chain id or an RPC URL from a person.

A text field is the atom the drawing is missing, not a screen the drawing never had.
Adding it is in scope. **Composing** a screen that was never drawn is not — see below.

## Out of Scope

- **The read layer.** No `rpc_pool`, no balances, no activity, no prices, no token
  list. Spec 051.
- **Money.** No send, no signing, no bundler. Spec 052.
- **Explore and the dApp surface.** `browser_history`, `dapp_session`,
  `dapp_permissions`, `ext_cache`. Spec 053.
- **Two surfaces blocked on drawings that do not exist** (founder decision,
  2026-09-05): the **add/edit contact form** and the **favourite control**. `contacts.rs`
  implements both; `design/contacts/C5` is a menu of choices (添加与导入导出), not a
  form, and no mock anywhere carries a favourite affordance. They are recorded as
  blocked and wired the moment a drawing exists.
- **Any machine change under `rust/crates/vela-core/src/app/`.** Zero lines. The
  `vela-core-uniffi` bridge does change — see FR-010.
- **No corpus regeneration.** No new i18n keys; existing keys only.
- **The other four clients.** Zero lines under `app-web/`, `app-desktop/`,
  `app-android/`, `app-browser-extension/`.

## User Scenarios & Testing

### User Story 1 — The address book is the person's own (Priority: P1)

Somebody signed in on their iPhone taps 通讯录 and arrives somewhere. What they see is
their own address book, and what they change to it is still there tomorrow.

**Why this priority**: it is the tab that currently does nothing, it is the smallest
machine in the cut, and it is independently shippable — land only this and one dead
tab becomes a real screen.

**Independent Test**: sign in on the device, open 通讯录, delete a contact through the
row swipe and its confirm sheet, force-quit, relaunch, look.

**Acceptance Scenarios**:

1. **Given** a signed-in wallet, **When** 通讯录 is tapped, **Then** the contacts home
   opens on that account's book — not on `ContactsFixtures.buildMobileState(.c1)`,
   and not on nothing at all.
2. **Given** a contact, **When** it is deleted through the swipe + confirm, **Then**
   it is gone after a relaunch, and a history-derived contact stays gone — the core's
   tombstone is honoured, never re-derived.
3. **Given** a signed-out and re-signed-in wallet, **When** contacts loads, **Then**
   it shows *that* account's book; `AccountSwitched` carries the address, and missing
   it crosses two people's address books.
4. **Given** a `vela.contacts` written by the web client, **When** iOS reads it,
   **Then** it renders correctly — and the reverse also holds.
5. **Given** no contacts yet, **When** contacts opens, **Then** the drawn empty state
   (C3) shows — never fixture contacts posing as the visitor's own.

---

### User Story 2 — A network a person adds is still there tomorrow (Priority: P1)

Somebody opens 设置 → 网络, types a chain id and an RPC URL, and 添加 enables only
when the core says the chain is compatible. After a relaunch the chain is still there.

**Why this priority**: it is the largest machine in the cut (2,900 lines, 16
operations) and the only one whose operations must reach the network, so it proves
both the storage seam and the operation-local HTTP pattern 051 generalises.

**Independent Test**: add a real chain against a live endpoint on the device, quit,
relaunch, look.

**Acceptance Scenarios**:

1. **Given** the add-network wizard, **When** a chain id and RPC URL are typed,
   **Then** the core's probes run against the real endpoint and 添加 enables **only**
   on the core's compatibility verdict — ST10b and ST10c are reached by the core,
   not by a fixture switch.
2. **Given** an RPC field pointed at a different chain's endpoint, **When** it is
   committed, **Then** the core's chain-mismatch refusal is shown and nothing is
   written.
3. **Given** an added network, **When** the app is relaunched, **Then** it is listed
   with every field intact.
4. **Given** a network written by the web or desktop client, **When** iOS reads it,
   **Then** it renders correctly.

---

### User Story 3 — The third machine costs nothing shared (Priority: P2)

`display_currency` is wired **third**, and doing so touches no shared plumbing.

**Why this priority**: it is the *measurement*, not a feature. 024 proved the web's
road was paved this way and 030 proved the desktop's; this is the iOS twin, and its
value is evidence for specs 051–053 rather than for a person.

**Independent Test**: change the display currency, relaunch, and read the diffstat of
that phase's commit.

**Acceptance Scenarios**:

1. **Given** that phase's commit, **When** its diffstat is read, **Then** it changes
   **zero lines of shared logic** in `CoreDriver.swift`, `CoreStore.swift`,
   `VelaStore.swift`, `CoreHTTP.swift` and `RootView.swift`.
2. **Given** the currency picker (ST5), **When** a currency is chosen, **Then** it
   survives a relaunch.
3. **Given** no rate source in this cut, **When** an amount is shown, **Then** it
   renders the core's *degraded* presentation — **never a fabricated conversion**,
   because `rate: null` is not `rate: 1`.

## Requirements

- **FR-001 (Core decides, shell performs)**: no business rule — validation, dedup,
  ordering, tombstones, refusal wording, compatibility verdicts, currency formatting
  — may be implemented or duplicated in Swift. Executors contain no business `if`.
  Two presentation judgements are explicitly the shell's, as they are on web:
  A–Z sectioning of the core-ordered contact list, and search-box narrowing.
- **FR-002 (Every operation is answered exactly once)**: a skipped operation leaves
  the core waiting forever. Failures are answered with the core's own failure
  variants; nothing propagates outward. Swift has no exhaustiveness check over a
  JSON tag, so **an unrecognised operation must answer** — a loud, modelled failure,
  never silence.
- **FR-003 (Fixtures stay canon)**: no `*Fixtures.swift` may lose or alter a
  constant. Live builders are **siblings** producing the same display models. Every
  gallery state and every `VELA_PAGE=`/`VELA_STATE=` route renders exactly as today,
  proven by the existing screenshot sweep.
- **FR-004 (Storage bytes are a cross-client contract)**: keys and camelCase field
  names must match what web, desktop and the Expo client read and write —
  `vela.contacts`, `vela.contacts.dismissed`, `vela.contactGroups`,
  `vela.customNetworks`, `vela.networkConfig`, `vela.rpcProviders`,
  `vela.serviceEndpoints`, `vela.displayCurrency`. Absent optionals are **omitted,
  never written as `null`**.
- **FR-005 (One store, one loop, one client)**: all three machines drive through one
  resident host over the existing `CoreDriver`, one `VelaStore`, one HTTP client.
  Per-machine code is limited to: an executor, a live builder, and the screen's
  bindings.
- **FR-006 (Records whole, views typed)**: the `AccountStore` invariant is
  generalised, not weakened. **Records** that round-trip through storage stay
  `[String: Any]` and are written back whole — a field-by-field copy is how a
  multi-key account silently becomes a different Safe. **Views** that only render are
  decoded into `Decodable` mirrors, so a wire change is a thrown error at the seam
  rather than a `nil` on a screen.
- **FR-007 (The network-flavoured line)**: an operation goes **live** in this cut when
  it is its own self-contained HTTP call *and* the core gates a user-visible outcome
  on its answer — which is `network_admin`'s probes. Everything whose infrastructure
  is 051's RPC pool or 052's transaction store is answered **fail-closed** with the
  core's modelled empty/unknown variant and carries a `// live in 051` marker.
- **FR-008 (`rate: null` is not `1`)**: the shell may never substitute a missing rate
  with 1. It is the core's rule, and the defect class `app/money.rs` exists to prevent.
- **FR-009 (Waiting is not fixtures)**: before a core has ruled, the screen shows a
  neutral surface — never fixture data posing as the visitor's own.
- **FR-010 (`rust/` changes are exports only)**: zero lines under
  `rust/crates/vela-core/src/app/`. `vela-core-uniffi` gains **three `bridge_object!`
  lines and nothing else**; `vela_core_uniffi.swift` is regenerated by the committed
  script, never hand-edited. Zero corpus delta.
- **FR-011 (One new control)**: a text field, built from the existing settings
  primitives and design tokens, used by the add-network wizard. No other new drawing.

## Success Criteria

- **SC-001**: 通讯录 opens. Contacts edit/delete, groups and a tombstone all survive a
  relaunch **on the founder's iPhone**; a `vela.contacts` written by the web client
  renders correctly on iOS.
- **SC-002**: a network added against a real endpoint, typed on the device, survives a
  relaunch with every field intact; the chain-mismatch refusal fires and writes
  nothing.
- **SC-003**: display currency survives a relaunch and renders degraded rather than
  fabricated when no rate exists.
- **SC-004 (the paved-road measurement)**: wiring the **third** machine changes zero
  lines of shared logic. `CoreDriver.swift`, `CoreStore.swift`, `VelaStore.swift`,
  `CoreHTTP.swift` and `RootView.swift` are untouched by that phase's commit, measured
  by `git diff --stat` and pasted into results.md. Declarations that Swift's module
  system forces are counted and named rather than scored as zero.
- **SC-005 (fixtures stay canon)**: the XCUITest screenshot sweep is green at every
  phase boundary, **and** the branch's cumulative diff of `SettingsFixtures.swift` and
  `ContactsFixtures.swift` is additive only — no existing constant's value changes.
- **SC-006**: the Swift test count strictly increases; `xcodebuild test` is green at
  every phase boundary.
- **SC-007**: zero lines changed under `rust/crates/vela-core/src/app/`; zero corpus
  delta; zero lines under the other four clients.
- **SC-008 (verified on the device, not asserted)**: every P1 acceptance scenario is
  confirmed on `shelchin's iPhone` (iPhone 15 Pro, paired), with the evidence recorded
  in results.md. A simulator run is preparation, never proof.
