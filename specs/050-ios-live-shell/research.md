# Phase 0 Research — 050 iOS Live Shell

**Date**: 2026-09-05 · **Branch**: `050-ios-live-shell` · **Base**: `origin/main` @ `28d25ae9`

Every decision below was reached by reading the tree, not by recalling it. Where a
claim is checkable, the command that checks it is written next to the claim.

---

## D1 — Where a live machine lives, and how long

**Decision**: one `@Observable final class` per machine (a *core store*), constructed
once in `RootView.init` beside `SessionController` and `OnboardingModel`, and passed
into screens as an init parameter. Construction is eager; **booting is lazy** — the
store dispatches its first event from the screen's `.task`, not from `init`.

**Why not per-screen (today's `CoreDriver` usage)**: `network_admin` probes endpoints
and holds a search debounce. A machine that dies with the settings sheet re-probes the
world every time somebody opens 设置, and a debounce armed against a dead core is an
effect nobody will answer.

**Why not an app-wide singleton / `@Environment`**: the repo's standing rule is that
components are pure — strings and models in, elements out. Screens already take their
model by parameter, which is what lets the galleries and `VELA_PAGE=` routes render
fixtures through the same view code. An environment-injected store would make every
screen implicitly stateful and quietly break that property.

**Why eager construction is safe**: a Crux `Core<A>` is a `Model::default()` plus an
empty `HashMap`. The expensive part is the first event, and that is what stays lazy.

**Precedent**: this is the same fork desktop's `resident.rs` documents (entity, not
`Global`) and the same shape as web's `network-admin.svelte.ts` (87 lines).

---

## D2 — Views are decoded, records stay whole

**This decision is already made, and 050 extends it rather than inventing it.**
`Features/Onboarding/Core/CoreViews.swift:12-17` states the rule in the tree today:

> Views are `Decodable` with `.convertFromSnakeCase`; OPERATIONS and RESULTS stay
> dictionaries. That split is deliberate rather than lazy: an operation is a tagged
> union of eighteen shapes, and a Swift enum for it would be ~300 lines of
> hand-written `init(from:)` whose only job is to reproduce a discriminator the
> executor immediately switches on again. A view is a flat record read by screens,
> where types earn their keep.

`CoreJSON.decoder` is that one decoder, `CoreJSON.decode` throws rather than returning
`nil`, and `SessionController` already reads `SessionView` through it. The three new
machines get the same treatment and the same file-level home — `ContactsWire.swift`,
`SettingsWire.swift` — rather than a second convention.

**The rule, restated for storage**: *a value the shell only renders is typed; a value
the shell gives back to the core or to disk is kept whole.*

**Why views are typed**: `dispatch` returns `{ view, effects, cancelled_effect_ids }`
as text. Reading a thirty-field view model by subscript gives thirty chances to
produce `nil` and render an empty screen with no error anywhere. A `Decodable` mirror
turns that into one thrown error at one seam.

**Why records are not**: `AccountStore`'s module doc already carries this invariant and
explains what it cost to learn — an `Account` mapper that copies field by field drops
`keys`, and the next restore silently derives a *different* Safe address. 050
generalises that invariant rather than weakening it: contacts, groups, tombstones,
custom networks and provider keys are round-tripped, so they are round-tripped whole.

**Casing** (verified): wire types carry no `rename_all` —
`grep -c 'rename_all = "camelCase"' rust/crates/vela-core/src/app/{contacts,display_currency,network_admin}.rs`
returns `0, 0, 0` — so views and operations are **snake_case** (`display_name`,
`is_contract`, `last_import`). The decoder therefore uses
`.keyDecodingStrategy = .convertFromSnakeCase`. **Stored** JSON is camelCase; the two
are different alphabets and confusing them is a defect class — see D5.

**Why not generate the mirrors**: `ts-rs` has no Swift backend, `serde-reflection`
is a second codegen toolchain to own, and either is a spec of its own rather than a
line in this one. The drift risk is answered by a test instead (D3).

**Rejected**: raw `[String: Any]` everywhere (silent `nil`s); `AnyCodable` (moves the
subscript problem behind a type name).

---

## D3 — The guard that replaces a generator

**Decision**: a Swift test per machine that constructs the real core, drives it to a
representative state, takes `view()`, and decodes it into the mirror — asserting on a
field that only exists if decoding really happened.

A Rust wire change then fails in Swift CI with a `DecodingError` naming the field,
which is the same signal `gen-core-types.mjs --check` gives web. It costs one test per
machine and no new toolchain.

`VelaWalletTests/EngineSmokeTests.swift` already links `VelaCore` and calls into it,
so the pattern exists; this extends it rather than inventing it.

---

## D4 — Storage: generalise `AccountStore`, do not replace it

**Decision**: extract `VelaStore` — a `UserDefaults`-backed KV whose values are **JSON
text**, keyed by the `vela.*` names — and let `AccountStore` keep its account-shaped
API on top of it.

**Verified**: `AccountStore.readList/writeList` (lines 139–154) store
`JSONSerialization` output as a **string** under the key. That is not an
implementation detail; it is what makes "byte-compatible with the other clients"
literally true — the same JSON text web puts in `localStorage` and Expo puts in
`AsyncStorage`. `VelaStore` keeps that and only widens the value type from
`[[String: Any]]` to any JSON value.

**Corrupt reads answer empty, never throw** — `AccountStore`'s existing rule, kept, and
the same rule the core's own doc-comments assume (`ContactOperation::ReadStore`:
"Unreadable/corrupt answers as empty, exactly as the TS loaders' `catch { [] }`").

**Rejected**: a file-backed store (desktop's `VELA_STATE_DIR` shape). It would give
iOS a second storage location that no other client reads and that `AccountStore` does
not use. One store per device or none.

---

## D5 — `vela.serviceEndpoints` has two writers, and that is a known trap

`AccountStore.saveRegistryURL` merges `passkeyIndexURL` into `vela.serviceEndpoints`
today (line 115). `network_admin`'s `WriteServiceEndpoints` writes **the same key**
with all four endpoints.

Web hit this exact collision and recorded the fix in
`settings/core/network-admin-executor.ts:411` — *"Through the onboarding module (D3a):
same key, same camelCase shape."*

**Decision**: iOS does the same. The `network_admin` executor reads and writes
`vela.serviceEndpoints` **through `AccountStore`**, never around it, and the field
names are the stored camelCase ones — `passkeyIndexURL` (capital URL),
`ethereumDataURL`, `bundlerServiceURL`, `fiatRatesURL`, mapped from the wire's
`passkey_index_url` &c. Two independent writers on one key is how a person's custom
passkey-index endpoint disappears the first time they open 设置 → 端点.

---

## D6 — HTTP: a new small client, and no refactor of `RegistryClient`

**Decision**: add `CoreHTTP` — a `URLSession` wrapper offering exactly two shapes
(`GET` JSON, `POST` JSON-RPC), a per-call timeout, and an error classification that
maps onto the core's modelled failure variants. `RegistryClient` (408 lines, in
production, talking to `p256-index.getvela.app`) is **left alone**.

**Why not reuse `RegistryClient`**: it carries registry semantics — commit/reveal,
pending uploads, endpoint override. Bending it into a general client in the same cut
that first uses one is how a working onboarding path acquires a regression. The
duplication is one `URLSession` configuration, it is named as debt in results.md, and
the natural time to collapse it is 051, when the RPC pool needs a client anyway.

**Timeouts and proxying**: no SOCKS5/system-proxy discovery (desktop's `proxy.rs`).
iOS `URLSession` follows the device's own proxy configuration, which is the platform
answer.

---

## D7 — Live builders are siblings of fixtures, never replacements

**Decision**: `SettingsLive` and `ContactsLive` are new types producing the **same**
display models `SettingsFixtures` and `ContactsFixtures` produce. Fixtures are not
touched except additively.

This is the repo's standing rule (024 research D7, 030 FR-003) and it is what keeps
the gallery, the `VELA_PAGE=`/`VELA_STATE=` routes and the XCUITest screenshot sweep
meaningful: they render the drawing, and the drawing does not move because the wiring
landed.

**Two presentation judgements are the shell's**, and are documented in the builders
exactly as web documents them:

1. **A–Z sectioning.** The core's list order (favourites first, then most recent) is
   authoritative; the *page* is an A–Z directory, so rows are grouped by initial and
   keep the core's relative order inside each letter.
2. **Search narrowing.** The core has no list-search event — its `matches_query`
   serves the recipient picker — so filtering the rendered list by the box's text is
   display-side narrowing of core-ruled rows.

---

## D8 — 通讯录 is a tab section, not a pushed route

**Decision**: add `.contacts` to `WalletSection` (today `{ wallet, explore }`) and
replace `case .contacts: break` in `RootView.selectTab`.

`design/contacts/C1` draws the tab bar with 通讯录 **selected**, so it is a peer of
钱包 and 探索, not something pushed over them. Contact detail (C2) and group detail
(C4) are pushes *within* that section, which is how `ContactsScene` already models
them (`home` / `detail` / `group`).

设置 stays a pushed route, as it is today — its own drawing shows a back affordance,
not a selected tab.

---

## D9 — Identicons come from the core, not from a Swift redraw

`vela-core-uniffi` already exports identicon rendering (`IdenticonParams`, `lib.rs:400`)
and iOS already consumes it. Contact rows and detail headers use it with the
lowercased address as seed — the same seed rule every other client uses, which is what
makes one contact look identical on all four.

---

## D10 — Bridge exports are added per cut, not all at once

**Decision**: 050 adds exactly **three** `bridge_object!` lines — `ContactsCore`,
`DisplayCurrencyCore`, `NetworkAdminCore`. The other 18 wallet-state machines wait for
the cut that wires them.

**Verified**: `rust/crates/vela-core-wasm/src/wallet_state.rs` exports 22 machines to
wasm; `rust/crates/vela-core-uniffi/src/onboarding_bridge.rs` exports 3 to Swift
(`CreateWalletCore`, `LoginCore`, `SessionCore`). Session is in both lists.

**Why not export all 21 now and never touch Rust again**: every export lands a class
in the committed `vela_core_uniffi.swift` and in the `.xcframework`. Eighteen classes
nothing calls is dead surface in a signed binary, and desktop's 030 explicitly refused
the equivalent ("a commit whose only artifact is eighteen dead-code warnings"). The
cost of the alternative is one macro line and one script run per cut.

**The regeneration is a script, not an edit**: `rust/scripts/build-ios-xcframework.sh`
rebuilds the dylib, regenerates the bindings, cross-compiles both slices and refreshes
`app-ios/VelaCoreKit/Sources/VelaCore/vela_core_uniffi.swift`. That file is committed
and must never be hand-edited. `Artifacts/` is gitignored (`app-ios/.gitignore:30`),
so a fresh checkout **cannot build the app** until the script has run once — which is
a quickstart line, not a defect.

---

## D11 — The text field

**Decision**: one new control, built from the existing
`Components/Settings/SettingsPrimitives.swift` vocabulary and `DesignSystem/Tokens.swift`
— no literal colours, spacing, radii or type. It is used by the add-network wizard
(chain id, RPC URL, and the endpoint/provider panels that are the same row shape).

**Verified gap**: `grep -rn "TextField" Components/Settings Features/Settings` returns
nothing; every wizard string today is a fixture constant, so ST10b (兼容) and ST10c
(不兼容) are reachable only by switching fixtures.

**Founder decision, 2026-09-05**: the field is in scope; the add/edit contact form and
the favourite control are **not** — they are whole surfaces nothing has drawn, and
composing them would be designing rather than wiring.

---

## D12 — Which operations go live, and which fail closed

Twenty-seven operations across three machines. The line is FR-007's: an operation goes
live when it is a self-contained HTTP call whose answer gates something a person sees.

| Machine | Live in 050 | Fail-closed, marked `// live in 051` |
|---|---|---|
| `contacts` (7) | `ReadStore`, `WriteContacts`, `WriteDismissed`, `WriteGroups` | `LoadSendHistory` (needs 052's transaction store), `ResolveIdentity`, `ClassifyRecipient` (need 051's pool) |
| `display_currency` (4) | `ReadStoredCode`, `WriteStoredCode`, `ReadDeviceCurrency` | `ResolveRate` (needs 051's price path) |
| `network_admin` (16) | the four writes + `ReadStore`, `StartSearchDebounce`, `FetchSearchIndex`, `FetchChainInfo`, `ProbeRpc`, `ProbeReachable`, `RpcGetCode`, `RpcCallP256`, `FetchServiceHealth` | `FetchFiatRates` (051), `InvalidatePools` (no pool exists yet), `ClearBundlerCache` (052) |

`ReadDeviceCurrency` is live here and was a carried debt on desktop: iOS has
`Locale.current.currency?.identifier`, so the region→ISO-4217 answer is a platform
call rather than a table to build.

A fail-closed answer is the core's own modelled *unknown*/*empty* variant — never an
invented value, never silence. FR-002 makes the silence case explicit: an operation
tag the executor does not recognise must still answer, loudly.

---

## D13 — Verification is on the device

**Founder decision, 2026-09-05**: 全程真机验证. `shelchin's iPhone` (iPhone 15 Pro,
`895C8176-371F-54BE-9AD3-8BCC5C5F9DFB`) is paired and reports `available`.

Signing is already configured for it — `DEVELOPMENT_TEAM = F9W689P9NE`,
`CODE_SIGN_STYLE = Automatic`, bundle id `app.getvela.VelaWallet` — and the
entitlements file carries `webcredentials:getvela.app` with its production AASA half
verified live in spec 019. So a device build needs no new provisioning work.

The simulator keeps one job: the XCUITest screenshot sweep that proves FR-003. It is
preparation, never proof (SC-008).
