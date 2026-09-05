# Tasks — 050 iOS Live Shell

Seven phases, each one commit ending on a green gate. `[P]` = parallelisable with its
siblings. Gate at every boundary: `xcodebuild build`, `xcodebuild test` (count
strictly increased), `audit-literals.mjs`, the screenshot sweep, and an empty diff
under `rust/crates/vela-core/src/app/` and `src/i18n_catalogs/`.

---

## Phase 0 — Baselines on record

Nothing is claimed later that was not measured here.

- **T001** Record the branch point, the Swift line counts by area, and the committed
  test count. → results.md
- **T002** Record the build baseline: `xcodebuild build` green on the iPhone 15 Pro
  simulator (`id=84146B7B-…`, iOS 17.5 — `OS:latest` does not match it), and
  `xcodebuild test` green with its count.
- **T003** Record the xcframework baseline: `build-ios-xcframework.sh` regenerates
  `vela_core_uniffi.swift` **byte-identically** to the committed copy, so any later
  diff there is genuinely this feature's.
- **T004** Record the 27-operation inventory and the live/fail-closed split, as
  [contracts/shell-operations.md](./contracts/shell-operations.md) states it.
- **T005** Commit a **shared scheme** (`xcshareddata/xcschemes/VelaWallet.xcscheme`).
  `project.pbxproj` is tracked and no scheme is — verified by
  `git ls-tree -r origin/main app-ios/ | grep xcscheme` returning nothing — so a
  fresh checkout depends on Xcode auto-creating one. That is a reproducibility hole in
  every later phase's gate, and closing it costs one file.
- **T006** Record the screenshot-sweep baseline output, which is what FR-003 is
  measured against.

---

## Phase 1 — The road, proven by the machine that needs no network

Contacts is first **because** it needs no HTTP: it isolates every pipeline bug from
every network bug. Landing this alone already turns a dead tab into a real screen.

### The shared road

- **T010** `rust/crates/vela-core-uniffi/src/onboarding_bridge.rs`: add
  `bridge_object!(ContactsCore, vela_core::app::contacts::Contacts)`. Nothing else.
- **T011** Run `build-ios-xcframework.sh`; commit the regenerated
  `vela_core_uniffi.swift`. Never hand-edit it.
- **T012** `Core/VelaStore.swift` — `vela.*` keys, values as JSON **text**, corrupt →
  empty, never throws. `AccountStore` rebased onto it with its public API and its
  existing tests unchanged (research D4).
- **T013** [P] ~~`Core/CoreExecutor.swift` — the executor protocol and the shared
  tagged-operation helpers.~~ **Not built.** Three executors turned out to share no
  behaviour worth a protocol: each one's `switch` is over its own operation names,
  its own coercion helpers and its own fail-closed variants. A protocol over that is
  a name for a coincidence. The rule it was meant to carry — *an unrecognised tag
  must still answer* (FR-002) — lives in each executor's `default:` branch instead,
  with a test per machine that proves it.
- **T014** [P] `Core/CoreStore.swift` — the resident holder: owns a `CoreBridge` and a
  `CoreDriver`, decodes the view into a `Decodable` mirror, publishes it
  `@Observable`. Constructed eagerly, booted lazily from the screen's `.task`
  (research D1).
- **T015** [P] `Core/CoreHTTP.swift` — `URLSession`, GET JSON + POST JSON-RPC,
  per-call timeout, failures classified onto the core's variants. Unused this phase;
  landed with the road so phase 2 adds no shared file. *(If this reads as dead code
  at review, move it to phase 2 — the road is measured by phase 4, not by this file.)*

### The machine

- **T016** `Features/Contacts/ContactsWire.swift` — `Decodable` mirrors of
  `ContactsView`, `Contact`, `ContactGroupView`, decoded with
  `.convertFromSnakeCase`.
- **T017** `Features/Contacts/ContactsExecutor.swift` — the four storage operations
  live, the three network ones fail-closed and marked. The **tombstone pivot**
  (stored object ⇄ wire list) and the omit-don't-null rule live here
  (data-model §3).
- **T018** `Features/Contacts/ContactsLive.swift` — `ContactsView` → `ContactsScene`,
  a sibling of `ContactsFixtures`. Carries the two documented presentation
  judgements: A–Z sectioning over the core's order, and search narrowing.
- **T019** `Features/Contacts/ContactsStore.swift` — the resident contacts machine;
  dispatches `AccountSwitched` from the session's address, which is what keeps two
  people's address books apart.
- **T020** `App/RootView.swift` — `.contacts` added to `WalletSection`;
  `case .contacts: break` replaced. Contact detail and group detail push within the
  section (research D8).

### Tests

- **T021** [P] Executor: every operation answered; corrupt store → empty; the
  tombstone pivot round-trips; optionals omitted, never `null`.
- **T022** [P] Live builder: sectioning preserves core order within a letter; an
  unnamed contact introduces itself by short address; empty view → the drawn C3 state.
- **T023** [P] View-decode drift: drive the real core, take `view()`, decode into the
  mirror, assert on a field that only exists if decoding happened (research D3).
- **T024** [P] `VelaStore` / `AccountStore`: the account API is unchanged, and the
  multi-key `Account` record still round-trips **whole**.

---

## Phase 2 — `network_admin` reads, and the first real HTTP

- **T030** `bridge_object!(NetworkAdminCore, …)`; regenerate.
- **T031** `Features/Settings/SettingsWire.swift` — `Decodable` mirrors of `NetView`.
- **T032** `NetworkAdminExecutor.swift`, read half: `read_store` (through
  `AccountStore` for `vela.serviceEndpoints` — data-model §5), `start_search_debounce`,
  `fetch_search_index`, `fetch_chain_info`. Wire ⇄ stored codecs for
  `NetCustomNetwork` and `NetNetworkConfig`, initialisms included (`rpcURL`, not
  `rpcUrl`).
- **T033** `SettingsLive.swift` — the networks list and network detail (ST9, ST9b),
  sibling of `SettingsFixtures`.
- **T034** `SettingsStore.swift` — resident; booted from the settings route's `.task`.
- **T035** Wire the settings route to the live builder for `.networks` /
  `.networkDetail` only. Everything else still renders fixtures — and says so.
- **T036** [P] Tests: the codec round-trips a record written by web; a corrupt store
  reads empty; the debounce is cancelled rather than answered when superseded.
- **T037** [P] **Regression test for the two-writer trap**: a saved
  `passkeyIndexURL` survives a `write_service_endpoints`.

---

## Phase 3 — The one new control, and the wizard that can finally be used

- **T040** The one new control — built from `SettingsPrimitives` and `Tokens`; no
  literals. *(Shipped as an optional `text: Binding<String>?` on the existing
  `SettingsUrlField` rather than a new file: `nil` renders exactly as drawn, so
  every fixture call site is unchanged.)* The founder's decision of
  2026-09-05 scopes it to this one control.
- **T041** Wire the add-network wizard's fields to core events: chain id, RPC URL,
  and the blur/commit that triggers the probes.
- **T042** `NetworkAdminExecutor` probe half: `probe_rpc`, `probe_reachable`,
  `rpc_get_code`, `rpc_call_p256`, `fetch_service_health` over `CoreHTTP`.
  `fetch_fiat_rates`, `invalidate_pools`, `clear_bundler_cache` answer fail-closed
  and carry their markers.
- **T043** The write half: `write_custom_networks`, `write_network_configs`,
  `write_rpc_providers`, `write_service_endpoints`. A cleared provider key is
  **removed**, not blanked.
- **T044** ST10b/ST10c reached by the **core's** verdict rather than a fixture
  switch; the chain-mismatch refusal renders and writes nothing.
- **T045** [P] Tests: the probe decodes a real `eth_chainId` reply; a timeout answers
  `reported_chain_id: null` with a latency; the add gate stays shut without a verdict.
- **T046** Device check: add a real chain, relaunch, confirm. → US2, SC-002.

---

## Phase 4 — `display_currency`, and the measurement this cut exists to produce

The third machine. Its value is the diffstat.

- **T050** `bridge_object!(DisplayCurrencyCore, …)`; regenerate.
- **T051** `DisplayCurrencyExecutor.swift` — three operations live,
  `resolve_rate` fail-closed. `read_device_currency` answers from
  `Locale.current.currency?.identifier` (a carried debt on desktop; a platform call
  here).
- **T052** The currency picker (ST5) and every fiat figure render from the core —
  including the **degraded** presentation when there is no rate.
- **T053** [P] Test: `rate: null` never renders as a 1:1 conversion (FR-008).
- **T054** **SC-004**: paste this phase's `git diff --stat` into results.md.
  `CoreDriver.swift`, `CoreStore.swift`, `VelaStore.swift`, `CoreHTTP.swift` and
  `RootView.swift` must be untouched. Declarations Swift's module system forces are
  named and counted, never scored as zero.

---

## Phase 5 — Contacts writes, end to end

- **T060** Row-swipe delete → the drawn confirm (C1s → delete-confirm) → the core's
  delete event → tombstone written.
- **T061** Group membership and the group-detail actions the drawings cover.
- **T062** The `AccountSwitched` boundary: sign out, sign in as another account,
  confirm the book changed.
- **T063** [P] Tests: a deleted history-derived contact stays deleted across a reload;
  two accounts' books never mix.
- **T064** Device check: CRUD, force-quit, relaunch. → US1, SC-001.

---

## Phase 6 — Closeout

- **T070** Device verification of every P1 scenario, with evidence. → SC-008.
- **T071** Cross-client check: a `vela.contacts` written by web renders on iOS, and
  the reverse.
- **T072** The **blocked-surface table**: the add/edit contact form and the favourite
  control — core complete, drawing absent. State exactly what a drawing would need to
  contain for the wiring to follow.
- **T073** The **debts → 051 table**: every `// live in 051` marker, the
  `CoreHTTP`/`RegistryClient` duplication, and the `CoreDriver` relocation.
- **T074** SC-001…SC-008 verdicts, each with its evidence or an honest failure.
- **T075** results.md, and the PR.

---

## What is deliberately not in this list

The add/edit contact form, the favourite control, and any Explore, signing, balance,
activity or send wiring. The first two are blocked on drawings; the rest are specs
051–053. A task list that quietly grew them would be the plan lying about its size.
