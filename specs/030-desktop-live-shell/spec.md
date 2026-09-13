# Feature Specification: Desktop Live Shell — Settings and Contacts on the Core

**Feature Branch**: `030-desktop-live-shell` (stacked on `029-native-repair`)

**Created**: 2026-09-04

**Status**: Draft

**Input**: Founder description: "把 desktop 客户端的设置与通讯录接到 vela-core 的三台存储机（network_admin / display_currency / contacts）上：core 决策、壳层执行，fixtures 仍是画廊正典。"

## Why

The desktop client has a fully live onboarding spine — create, sign in, session,
route guard, sign-out — and, past that point, a wallet made entirely of fixtures. A
person can sign in with a security key and then read somebody else's address book.

Spec 024 did this same layer on web and proved the shape: three machines, no network,
storage only. This is that layer on the second client, and it is deliberately the
*storage* layer first for the same reason 024 was — it is the only cut that can be
finished without inventing a network stack, so the plumbing gets proven before
anything depends on it.

### What desktop starts with that web had to build

The asymmetry is the point, and it is why this cut is smaller than 024 was:

| 024 web had to invent | Desktop already has | Where |
|---|---|---|
| 311 generated TypeScript wire types | nothing — the types are Rust structs | — |
| `loadCore()` + 24 wasm class exports | nothing — `Core<A>` is a struct | — |
| `effect-loop.ts` + `json-shell.ts` (126 + 44 lines) | `CoreHost<A>` — generic, in-process, no JSON | `src/core_host.rs` (157) |
| `services/storage.ts` (a new IndexedDB KV) | an atomic JSON document with a `VELA_STATE_DIR` pin, corrupt→empty, and a write lock | `src/executor/storage.rs` (479) |
| a fetch-with-timeout primitive | `ureq` + `proxy::agent()` — rustls, SOCKS5h, GNOME/WinINET proxy discovery | `src/executor/proxy.rs` (329) |
| an `operationFailure` twin per machine | **not needed** — every `match` arm returns a result on both paths, so a missing mapping is a compile error | `src/executor/mod.rs:24-32` |
| a `/contacts` route, and un-swallowing the settings tab | both are already clickable destinations | `src/wallet/page.rs` |

So the residue is exactly three things: **a resident-core host for gpui**, **three
executors**, and **a display-model seam** so `fixtures.rs` stays the gallery's canon.

## Design Authority

`design/contacts/` and `design/settings/` remain the visual authority; this feature
draws no new screen. The behavioural authority is the three Rust machines, and the
**porting** authority is web's 024, whose executors are the same operations answered
against different I/O:

| Web source (at `f9bcb278`) | Lines |
|---|---|
| `settings/core/network-admin-executor.ts` | 568 |
| `settings/core/currency-executor.ts` | 73 |
| `settings/live.ts` | 404 |
| `contacts/core/contacts-executor.ts` | 276 |
| `contacts/live.ts` | 216 |

## Out of Scope

- **The read layer.** No `rpc_pool`, no balances, no activity, no prices. Spec 031.
- **The money layer.** Spec 032.
- **Explore and signing staying fixtures.** 029 made them reachable; wiring them to
  `browser_history` / `dapp_session` / `clear_signing` is 032 and later.
- **Any change under `rust/`.** No core, no bindings, and — while spec 026 is open —
  **no corpus regeneration** (FR-010). That collision is what cost the repo spec 022's
  wiring; doing it twice would be the joke writing itself.

## User Scenarios & Testing

### User Story 1 — A network a person adds is still there tomorrow (Priority: P1)

Somebody opens 设置 → 网络 on the desktop, adds a chain, and finds it after a
relaunch — with the compatibility verdict the core reached, not a fixture's.

**Why this priority**: It is the largest machine (2,900 lines, 15 operations) and the
only one in this cut whose operations must touch the network, so it proves both the
storage seam and the "operation-local HTTP" pattern 031 will generalise. It is also
the MVP: land only this and the settings section is real.

**Independent Test**: Add a real chain against a live endpoint, quit, relaunch, look.

**Acceptance Scenarios**:

1. **Given** a signed-in desktop wallet, **When** a chain is added through the wizard,
   **Then** the core's probes run against the real endpoint and 添加 enables **only**
   when the core says the chain is compatible.
2. **Given** an added network, **When** the app is relaunched, **Then** it is still
   listed, with every field intact.
3. **Given** an RPC field pointed at a different chain's endpoint, **When** it blurs,
   **Then** the core's chain-mismatch refusal is shown and nothing is written.
4. **Given** a network written by the web client, **When** desktop reads it, **Then**
   it renders correctly — and the reverse also holds.

---

### User Story 2 — The address book is the person's own (Priority: P1)

Contacts, groups, favourites and deletions are the signed-in account's, persist across
a relaunch, and interoperate byte-for-byte with the other clients.

**Why this priority**: The tab is reachable today and shows a stranger's contacts,
which is the most directly wrong thing in the client. Independently shippable.

**Independent Test**: Create, edit, group, favourite and delete; relaunch; then read a
`vela.contacts` written by web.

**Acceptance Scenarios**:

1. **Given** the contacts section, **When** a contact is added, edited, favourited or
   deleted, **Then** the change survives a relaunch.
2. **Given** a deleted history-derived contact, **When** the app is relaunched,
   **Then** it stays deleted — the core's tombstone is honoured, not re-derived.
3. **Given** a signed-out and re-signed-in wallet, **When** contacts loads, **Then**
   it shows that account's book — `AccountSwitched` carries the address, and missing
   it crosses two accounts' books.

---

### User Story 3 — The third machine costs nothing shared (Priority: P2)

**`contacts` is wired third, and doing so touches no shared plumbing at all.**

(Amended during phase 1. The draft named `display_currency` as the probe because it
is smallest; it shipped *first* instead, to prove the road with the least code and to
avoid a commit whose only artifact is eighteen dead-code warnings. The probe's power
is being **third**, not smallest — it measures whether the road was paved, and any
third machine measures that. It also improves the order: `network_admin` becomes
second, and it is the one machine here that must do HTTP, so a road that needs
changing is found at machine two rather than hidden behind an easy third.)

**Why this priority**: It is the *measurement*, not a feature. 024's SC-008 proved the
web's road was paved by wiring its third machine and diffing; this is the desktop
twin. It is P2 because the value is evidence for specs 031–038, not for a person.

**Independent Test**: Change the display currency, relaunch, and read the diffstat of
that phase's commit.

**Acceptance Scenarios**:

1. **Given** the contacts phase's commit, **When** its diffstat is read, **Then** it
   touches zero lines in `core_host.rs`, `resident.rs`, `executor/mod.rs`,
   `executor/storage.rs`, `executor/proxy.rs` and `main.rs`.
2. **Given** the currency picker, **When** a currency is chosen, **Then** it survives
   a relaunch.
3. **Given** no rate source in this cut, **When** an amount is shown, **Then** it
   renders the core's *degraded* presentation — **never a fabricated conversion**,
   because `rate: null` is not `rate: 1`.

## Requirements

- **FR-001 (Core decides, shell performs)**: No business rule — validation, dedup,
  ordering, tombstones, refusal wording, compatibility verdicts — may be implemented
  or duplicated in desktop code. Executors contain no business `if`.
- **FR-002 (Every operation is answered exactly once)**: A skipped operation leaves
  the core waiting forever. Failures are answered with the core's own failure
  variants; nothing propagates outward.
- **FR-003 (Fixtures stay canon)**: No `fixtures.rs` may lose or alter a constant.
  Live builders are **siblings** producing the same display models. Every gallery
  state and every `VELA_PAGE=` fixture route renders exactly as it does today.
- **FR-004 (Storage bytes are a cross-client contract)**: Keys and camelCase field
  names must match what web and the Expo client read and write:
  `vela.contacts`, `vela.contacts.dismissed`, `vela.contactGroups`,
  `vela.customNetworks`, `vela.networkConfig`, `vela.rpcProviders`,
  `vela.serviceEndpoints`, `vela.displayCurrency`. Absent optionals are **omitted,
  never written as `null`**.
- **FR-005 (One shared host, not three)**: All three machines drive through one
  generic gpui resident host. Per-machine code is limited to: an executor, a live
  display-model builder, and the bindings in the screen.
- **FR-006 (The network-flavoured line)**: An operation goes **live** in this cut when
  it is its own self-contained HTTP call *and* the core gates a user-visible outcome
  on its answer — which is `network_admin`'s probes, exactly as 024 revised its own
  D1 to do. Everything whose infrastructure is 031's RPC pool or 032's transaction
  store is answered **fail-closed** with the core's modelled empty/unknown variant,
  and carries a `// live in 031` marker.
- **FR-007 (`rate: null` is not `1`)**: The shell may never substitute a missing rate
  with 1. It is the core's rule, and the defect class `app/money.rs` exists to prevent.
- **FR-008 (Waiting is not fixtures)**: Before a core has ruled, the screen shows a
  neutral surface — never fixture data posing as the visitor's own.
- **FR-009 (Nothing under `rust/`)**: Zero files changed under `rust/`.
- **FR-010 (No corpus regeneration while 026 is open)**: See Out of Scope.

## Success Criteria

- **SC-001**: A network added against a real endpoint survives a relaunch with every
  field intact; the chain-mismatch refusal fires and writes nothing.
- **SC-002**: Contacts CRUD, groups, favourites and a tombstone all survive a
  relaunch; a `vela.contacts` written by the web client renders correctly on desktop.
- **SC-003**: Display currency survives a relaunch and renders degraded rather than
  fabricated when no rate exists.
- **SC-004 (the paved-road measurement)**: Wiring the **third** machine (`contacts`)
  changes **zero lines of shared logic**: `src/core_host.rs`, `src/resident.rs`,
  `src/executor/proxy.rs` and `src/main.rs` are untouched, and `src/executor/mod.rs`
  and `src/executor/storage.rs` gain **declarations only** — no function added,
  changed, or called differently. Measured by `git diff --stat` of that phase's
  commit, pasted into results.md.

  *(Amended after measuring. The draft said "zero lines" across all six files, which
  Rust cannot satisfy: a module must be declared to exist, so `pub mod contacts;` is
  the irreducible cost of adding a file. The three other lines are key constants in
  the cross-client contract registry `storage.rs` deliberately centralises. Restating
  the criterion to what it was always measuring — shared logic — is honest; quietly
  scoring 4 changed lines as "zero" would not be, and neither would scattering the
  keys to their machines purely to win the number.)*
- **SC-005 (fixtures stay canon)**: `scripts/sweep-gallery.sh` is green at every phase
  boundary, **and** the branch's cumulative diff of `settings/fixtures.rs` and
  `contacts/fixtures.rs` is additive only — no existing constant's value changes.
- **SC-006**: `cargo test` count strictly increases; `fmt` and the desktop CI job are
  green at every phase boundary.
- **SC-007**: Zero files changed under `rust/`; zero corpus delta.
