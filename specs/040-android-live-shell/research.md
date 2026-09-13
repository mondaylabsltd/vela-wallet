# Research — Android Live Shell (040)

Every decision below was taken against files in this worktree at
`origin/main` (28d25ae9), not from recollection. Where a sibling client already
solved the same problem, the decision is "port it", and the port source is
named with its path.

---

## D1 — What Android is actually missing

**Checked, not assumed.** `rust/crates/vela-core/src/app/` holds **24
`impl App for …` machines** (39,630 lines). Android drives three of them
(`CreateWallet`, `Login`, `Session`) through
`rust/crates/vela-core-uniffi/src/onboarding_bridge.rs:209-224`.

`grep -n 'Fixtures\.' navigation/VelaNavHost.kt` returns **eight** call sites
across six drawn surfaces — wallet, flows, explore, signing, contacts,
settings. All of them are pictures.

**Decision**: no rule is written in Kotlin, and no rule is written in Rust by
this feature. The three machines it wires exist and are complete; the work is
an executor and a display-model builder each.

**Consequence for the request that opened this feature**: the address book,
the explorer and clear signing do **not** need rewriting in `vela-core` —
`contacts.rs` (1,385 lines), `browser_history.rs` (471) and `clear_signing.rs`
(4,841) are already there. That premise is retired here so specs 041 and 042
do not re-litigate it.

---

## D2 — How a Kotlin shell reads a core view

The bridge speaks JSON in both directions (spec 019 contract §3). Two options
were on the table.

- **A. `org.json`, hand-written tolerant parsers**, which is what
  `feature/onboarding/core/CoreViews.kt` (254 lines) does today for 25
  onboarding types.
- **B. `kotlinx.serialization`**, `@Serializable` data classes with a
  discriminated sealed hierarchy.

`ls app-web/vela-wallet/src/lib/core/generated | wc -l` → **311** wire types
for the wallet-state family. Option A scales that to several thousand lines of
hand-written `optString`/`optJSONObject` walking, and every one of them is a
place a typo becomes a silently-empty screen.

The Rust wire format is `#[serde(tag = "type", rename_all = "snake_case")]`,
which is *exactly* `kotlinx.serialization`'s default class discriminator plus
`@SerialName`. The mapping is mechanical, so the Kotlin declaration ends up
being a readable transcription of the Rust enum rather than a parser.

**Decision: B.** Add the `org.jetbrains.kotlin.plugin.serialization` plugin
(version-matched to the Kotlin already in `libs.versions.toml`: 2.2.10) and
`kotlinx-serialization-json`.

**Costs accepted**: one Gradle plugin and one runtime library (~300 KB dex,
against a `libvela_core_uniffi.so` already an order of magnitude larger — see
D6). `org.json` stays for `AccountStore`'s record-passthrough, which
deliberately does **not** want typed classes (its own header explains why: a
field-by-field `Account` copy silently drops `keys` and rebuilds a *different*
Safe).

**Configuration, fixed once for every machine**:
`ignoreUnknownKeys = true` (a Rust field Android does not render must not
crash it), `explicitNulls = false`, and **no** `coerceInputValues` — an unknown
enum variant must throw, because rendering an unknown value as a default is
how a person gets shown the wrong screen (the rule `CoreViews.kt` already
states: *tolerant of absent, intolerant of wrong*).

---

## D3 — Keeping Kotlin wire types honest (FR-008)

`ignoreUnknownKeys` is the price of tolerance: rename a field in Rust and
Kotlin quietly reads `null` instead of failing. So the tolerance needs a gate.

The repository already has the shape of one: `DesignTokenDriftTest` reads
`docs/design-tokens.json` through the `vela.repo.root` system property that
`app/build.gradle.kts` sets for unit tests, and fails when the Kotlin mirror
drifts from the export.

**Decision**: a `CoreWireDriftTest` in the same style. It reads the committed
ts-rs mirrors under `app-web/vela-wallet/src/lib/core/generated/*.ts` — the
generated projection of the same Rust types — and asserts, for every Kotlin
wire class this feature declares, that:

1. every `@SerialName` Kotlin declares exists in the TypeScript type;
2. every non-nullable TypeScript field is present in Kotlin (a field Android
   silently drops is allowed only when it is optional in Rust);
3. every sealed-subclass discriminator matches a variant of the TS union.

Rejected alternative: generating Kotlin from Rust with a new emitter. ts-rs
emits TypeScript only, and `crux`'s `serde-generate` path produces
bincode-oriented Kotlin that does not match this JSON bridge. A generator is
~500 lines of new tooling to save ~40 declarations in this slice; the drift
gate gets the same safety for ~120 lines. Revisit at 041 if the declaration
count crosses ~150.

---

## D4 — Where the plumbing lives (FR-004)

Today `CoreDriver` (184 lines: serialized bridge calls, per-effect jobs,
cancellation, fault reporting) sits in
`feature/onboarding/core/`. It is already feature-neutral — its own header says
"blind to product semantics" — but its package says otherwise, and a contacts
executor importing from `feature.onboarding.core` is a dependency arrow
pointing at the wrong thing.

**Decision**: promote, do not copy.

```
core/crux/
  CoreBridge.kt      the three-method interface uniffi objects satisfy
  CoreDriver.kt      moved verbatim from feature/onboarding/core/
  CoreHost.kt        NEW: a driver + its view StateFlow, one per machine
  Wire.kt            NEW: the shared Json instance and its rules (D2)
```

Onboarding imports the promoted classes; it does not keep a copy. The move is
mechanical (package line + imports) and is proven by the existing
`OnboardingExecutorTest` staying green.

**`CoreHost` is the piece that is genuinely new**, and it is small: a machine,
its driver, and a `StateFlow<ViewModel>` the UI collects. It exists because
`SessionController` (79 lines) is the same three fields hand-rolled for one
machine, and writing that twenty-one more times is the thing this feature is
supposed to prevent.

---

## D5 — Where a machine lives at runtime (FR-007)

Three candidates: a `ViewModel` per screen, a nav-graph-scoped `ViewModel`, or
the `AppContainer` composition root.

`SessionController` is already in `AppContainer` and its header explains why:
the guard "outlives every screen", and one rebuilt per screen would report
`loading` after every rotation and bounce a signed-in person back to
onboarding.

The contacts and network machines have the same property for a weaker reason —
they hold a loaded store, and re-booting on every screen entry would flash an
empty address book — but the *conclusion* differs: they should not be
constructed at app start either, because a person who never opens Settings
should not pay for the network machine's boot.

**Decision**: `AppContainer` holds them **lazily** (`by lazy`), created on
first use and never torn down for the process's life. The composition root
stays manual (no DI framework — research D8 of spec 008 stands at this scale).

Rejected: `viewModel()` per screen — loses state on process death in exactly
the way that makes a settings page flicker; and the machines are not
screen-shaped (contacts is read by Settings' row count as well as by the
Contacts tab).

---

## D6 — The bridge size budget (FR-002 / SC-006)

Spec 019 recorded **+785,864 stripped bytes (arm64-v8a)** for linking crux and
the first three machines. The handover note for this program required the
next slice to *measure* before adding more, because an estimate of "+3–5 MB"
was never checked.

**Method** (both builds `--release`, `cargo ndk -t arm64-v8a --platform 29`,
size taken after `llvm-strip`):

| Build | Machines exported | Stripped bytes |
| --- | --- | --- |
| A (baseline) | 3 — `CreateWallet`, `Login`, `Session` | 3,715,432 |
| B (this slice) | 6 — plus `Contacts`, `NetworkAdmin`, `DisplayCurrency` | 4,786,088 |
| C (whole program) | 24 — every machine | 7,890,696 |

The decision rule was fixed **before** the builds ran: if B − A exceeded
**400 KB stripped** (the 019 per-machine cost × 1.5), the plan would stop and
price the alternative — one multiplexed bridge object dispatching by machine
name, instead of 21 monomorphised `Bridge<A>` instantiations.

**B − A = 1,070,656 bytes (357 KB per machine). The gate tripped.** So the
alternative was priced, with a fourth build rather than an argument:

| Build | Objects exported | Machines linked | Stripped bytes |
| --- | --- | --- | --- |
| A | 3 | 3 | 3,715,432 |
| **D** | **6** (three of them *duplicate* wrappers over machines already linked) | 3 | **3,733,192** |
| B | 6 | 6 | 4,786,088 |

A uniffi object costs **5,920 bytes**. A machine costs **356,885**. The
scaffolding is **1.7%** of what the gate was worried about; multiplexing 21
objects into one would save ~124 KB out of 4.18 MB and cost a hand-written
string switch that the type system could no longer check.

**Verdict: the gate tripped, the alternative was measured, and it does not
exist.** What the bytes buy is the machines themselves — the same 39,630 lines
of rules the web client already ships as wasm and the desktop client links
directly. The marginal cost also *falls* as machines are added (357 KB for the
first three, 172 KB each for the last eighteen), which is what shared serde and
crux infrastructure amortising looks like.

**The number to carry**: the whole program is **+4,175,264 stripped bytes per
ABI** over today's Android baseline. The app ships an App Bundle with three ABI
splits, so a person downloads one of them: **≈ +4.2 MB**. This slice's share is
1.02 MB; specs 041 and 042 inherit ≈ 3.1 MB between them.

This is a product cost, and it is recorded rather than absorbed. If it ever
needs to come down, the lead worth measuring first is `panic = "abort"` in
`[profile.release]` (unwinding tables across a 7.9 MB binary), **not** the
bridge shape — and that is a repository-wide profile change, not this feature's
call to make.

---

## D7 — The boxes nobody can type in

`grep -rn 'VelaTextField|BasicTextField' feature/settings/` returns **nothing**.
`VelaUrlField` (`components/SettingsPrimitives.kt:232`) takes `label` and
`value` and draws them; there are **ten** call sites in `SettingsScreen.kt`,
including the add-network form, the network detail's RPC and explorer rows,
the provider key fields and the endpoint fields.

The desktop sibling hit this exactly once and mis-filed it as "blocked on
missing artwork" for four features running. It is not blocked: the design
system already ships `VelaTextField` (used throughout onboarding), and the
mocks in `design/settings/` show a field with a value in it — a field that
happens to have been built read-only.

**Decision**: `VelaUrlField` gains an optional `onValueChange`. Passing it
makes the field editable; omitting it leaves every existing call site — and
every gallery state — rendering byte-identically. This is FR-011, and it is
sequenced *before* the settings write path, because a write path with no way
to enter a value cannot be tested.

---

## D8 — `read_device_currency` is real on Android

`display_currency.rs:46` documents the operation as "the device region's
currency, from the primary locale only. `None` on web and for regionless
locales", and `:390` states the guard: three ASCII uppercase letters.

Web answers `null` because a browser has no region. **Android does have one**,
so this is the first place the Android shell is *more* capable than the web
shell rather than a port of it: `java.util.Currency.getInstance(locale)` on
the primary locale from `Resources.configuration.locales`, guarded to
`^[A-Z]{3}$`, `null` on `IllegalArgumentException` (regionless locales like
`en` throw).

The seed rule itself — when a device currency becomes the display currency —
is the core's and is not restated here.

---

## D9 — Storage (FR-005)

`AccountStore` already owns a `preferencesDataStore(name = "vela_onboarding")`
with `vela.accounts`, `vela.activeAccountIndex`, `vela.pendingUploads` and —
already — **`vela.serviceEndpoints`**, which is one of the keys
`network_admin` reads.

Two files holding the same namespace is how two readers of
`vela.serviceEndpoints` end up disagreeing.

**Decision**: extract the raw key-value half of `AccountStore` into
`core/data/VelaStore` (`read(key)`, `write(key, value)`, `remove(key)`) over
the **same** DataStore file, and let `AccountStore` keep its
records-go-in-and-come-out-whole invariant as a thin layer on top. The
existing keys keep their names and formats; the new keys —
`vela.contacts`, `vela.contacts.dismissed`, `vela.contactGroups`,
`vela.displayCurrency`, `vela.customNetworks`, `vela.networkConfig`,
`vela.rpcProviders` — are the ones the other clients already use, with the
same camelCase JSON payloads (`contacts-executor.ts:38-40`,
`network-admin-executor.ts`).

The theme store stays separate, for the reason its own comment gives: sign-out
clears wallet identity and must not be able to reach a display preference.

---

## D10 — What stays fixture-fed, and how it says so

`network_admin` has sixteen operations, ten of which are network calls (probe,
chain info, service health, fiat rates). This feature has no network layer, so
those answer with the same "unknown" shapes the web's 024 contract defined
(`specs/024-web-live-shell/contracts/shell-operations.md`).

**Decision**: identical fail-closed arms, each carrying the marker
`// live in 041`, and the settings surfaces they feed (RPC health tiles,
latency, storage figures) render their *unknown* state rather than a staged
value. The desktop sibling proved the value of the marker: `grep` for it is
how the next feature finds its own work, and the count going to zero is how
that feature knows it is done.

`resolve_rate` answers `null`, **not `1`** — the core's comment at
`display_currency.rs:50` is explicit that the two differ and that the seed
decision depends on the difference.

---

## D11 — Navigation and reachability

`origin/main`'s `VelaNavHost` has no explore or signing destination: the
navigation repair that made them reachable lives on the unmerged
`029-native-repair` branch. This feature touches neither surface, so it inherits
that state unchanged and records it (spec Assumptions) rather than quietly
importing a fix from a branch nobody has reviewed.

**Settings and Contacts are not equally reachable, and the difference is the
point of this feature.** Read, not remembered — `VelaNavHost.kt:271-280`:

- **Settings** is pushed by the tab bar. A signed-in person reaches it today.
- **Contacts** is `push`ed only from Settings' `通讯录` row
  (`VelaNavHost.kt:334`). The *tab* deliberately does nothing, and the comment
  says exactly why: "通讯录 and 探索 stay on this screen rather than navigating
  to fixtures a signed-in person would read as their real data."

So the drawn address book was disconnected **on purpose**, because it lies.
Two consequences:

1. There is a live fixture leak today: Settings → 通讯录 shows a signed-in
   person six strangers. FR-017 closes it.
2. **Connecting the Contacts tab is part of US2's deliverable, not a separate
   feature.** The reason the tab was inert stops being true the moment the
   screen reads the person's own book — and a wiring feature that leaves the
   entry point disconnected has not finished the job it claims.

`CONTACTS` is also listed in `DEVELOPER_ROUTES`, which exempts it from the
session guard. That entry stays: the contacts *gallery* still needs to be
reachable on a device with no wallet. But it is no longer the only way in.
