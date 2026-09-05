# Results — Android Live Shell (040)

Written as the work lands, not after. Each phase adds its own verdict; the
success criteria are verdicted at the end.

---

## Phase 0 — The size probe (T001–T004) ✅

Four builds, all `cargo ndk -t arm64-v8a --platform 29 build --release`
(`lto = true`, `opt-level = "z"`, `codegen-units = 1`), sizes taken after
`llvm-strip -s`.

| Build | uniffi objects | machines linked | stripped bytes | Δ vs A |
| --- | --- | --- | --- | --- |
| **A** baseline | 3 | 3 | 3,715,432 | — |
| **D** scaffolding probe | 6 (3 duplicates) | 3 | 3,733,192 | **+17,760** |
| **B** this slice | 6 | 6 | 4,786,088 | **+1,070,656** |
| **C** whole program | 24 | 24 | 7,890,696 | **+4,175,264** |

**Unit costs, measured:**

- one uniffi object: **5,920 bytes** (D − A, ÷3)
- one machine, first three added: **356,885 bytes** (B − A, ÷3)
- one machine, remaining eighteen: **172,478 bytes** (C − B, ÷18)

### The gate, and what happened at it

The rule was fixed before the builds ran (research D6): *if B − A exceeds
400 KB stripped, stop and price the multiplexed-bridge alternative.*

**B − A = 1,070,656 bytes. The gate tripped.** So the alternative was priced
with build D rather than with an argument: three extra uniffi objects wrapping
machines *already linked* cost 17,760 bytes between them. Bridge scaffolding is
**1.7%** of the cost the gate was aimed at. Collapsing 21 objects into one
multiplexer would save ~124 KB of 4.18 MB and would replace a compiler-checked
set of classes with a hand-written string switch.

**Verdict: the gate tripped, the alternative was measured, and it does not
exist.** The bytes are the machines' own code — the same 39,630 lines of rules
the web ships as wasm and the desktop links directly. Proceeding.

### The number the program carries

Wiring all 24 machines costs **+4,175,264 stripped bytes per ABI**. The app
ships an App Bundle with three ABI splits, so one person downloads one split:
**≈ +4.2 MB**. This slice spends 1.02 MB of it; specs 041 and 042 inherit
≈ 3.1 MB between them, and the marginal cost per machine is *falling*
(357 KB → 172 KB) as shared serde and crux code amortises.

If that ever needs to come down, the first lead to measure is
`panic = "abort"` in `[profile.release]` — a repository-wide change affecting
all clients, and therefore not this feature's call.

### Reproducing it

`/tmp` scripts are not artefacts; the method is in
[quickstart.md](./quickstart.md) §5, and the registration lists used for builds
B and C are the same ones `rust/crates/vela-core-wasm/src/wallet_state.rs`
already carries, with `bridge_class!` read as `bridge_object!`.

**Working tree left clean**: every probe restored
`rust/crates/vela-core-uniffi/src/onboarding_bridge.rs` on exit, verified with
`git status` after the run.

---

## Phase 1–2 — The road (T005–T015) ✅

Six machines on the bridge, the plumbing out of the onboarding feature, one
key-value space, and a drift gate that has been shown to fail.

**SC-008 demonstrated.** `CurrencyView.rate` was renamed to `fx_rate` in the
generated mirror — exactly what a Rust rename produces after regeneration — and
the suite went red, naming the field and both candidate causes:

```text
CoreWireDriftTest > currencyRateStaysNullable FAILED
CoreWireDriftTest > currencyViewMatchesTheGeneratedMirror FAILED
  CurrencyView.rate is declared in Kotlin but absent from the generated mirror
  (fields there: [code, committed, fx_rate]) — a Rust rename, or a typo here
```

Reverted; the mirrors are untouched in the diff. Two tests caught it
independently, which is the design: one checks that every field Kotlin names
exists, the other that `rate` specifically stays nullable, because `null` is not
`1` and the core's comment says what confusing them costs.

The Gradle input declaration matters as much as the test. `app/build.gradle.kts`
now lists the mirror directory as a test input; without it the mirrors would
regenerate and the one test that would have caught the change would be skipped
as `UP-TO-DATE`. The file's own comment had already warned about this hazard for
the design tokens.

**Gate**: 118 existing tests unchanged and green, +5 drift tests.

---

## Phases 3–8 — The machines (T016–T045) ✅

Three machines live, 209 unit tests (from 118), `assembleDebug` green.

| Surface | Reads from | Writes through |
| --- | --- | --- |
| Currency row + sheet | `display_currency` | `vela.displayCurrency` |
| Networks list | `network_admin` | — |
| Service endpoints | `network_admin` | `vela.serviceEndpoints` |
| Provider keys | `network_admin` | `vela.rpcProviders` |
| Remove a custom network | `network_admin` | `vela.customNetworks` |
| Contacts list / search / groups / detail | `contacts` | — |
| Delete a contact | `contacts` | `vela.contacts` (+ `.dismissed`) |

### Four bugs the real machine found, none of them visible in a fixture

The JVM suite drives the actual Rust through JNA (`jna.library.path` was
already wired for spec 005), so these surfaced in a test rather than on a
device — which is the argument for having written those tests at all.

1. **`encodeDefaults`.** kotlinx omits a property equal to its default, so
   `StoreLoaded()` went out as `{"type":"store_loaded"}` and serde refused it:
   *"invalid result from shell: missing field `custom_networks`"*. serde
   defaults a missing `Option` to `None` but requires every `Vec`. Symptom: a
   machine stuck at `loaded = false` — a settings page frozen on its
   placeholder, one line in the log.
2. **`u32` is not `f64`.** `Contact.tx_count` is `u32`; ts-rs writes every Rust
   number as TypeScript `number`, so the drift gate is blind to the difference
   and serde is not — *"invalid type: floating point `0.0`, expected u32"*, and
   the address book never loaded. A test now pins the blind spot by name.
3. **Scientific notation in storage.** `org.json` renders a Double as
   `1.725E12`. Legal JSON that every parser accepts, and something no other Vela
   client has ever written into `vela.contacts`. Epoch ms go to disk whole.
4. **A view is right before the bytes are.** The core updates its model and
   emits the write as an effect, so asserting on the view and then reading the
   store is a race that passes on a fast machine. The machine tests await the
   store.

### What the device says

- **App boots on the store refactor.** `AccountStore` now rides on `VelaStore`
  over the same DataStore file; the wallet reaches Welcome with the session
  machine settled, on a real device (Xiaomi alioth, MIUI V816).
- **SC-003 ✅ verified on device.** The live contacts route on a device with an
  empty store shows the empty state and its two calls to action — not the six
  fixture people, not the three fixture groups.

**And a near-miss worth writing down**: the first run of that check showed the
fixture book. The cause was not the code — `adb install` had printed
`Performing Streamed Install` and *nothing else*, and I read only the last
line. The install had not completed. A screenshot of a stale APK is the most
convincing wrong answer available on a device; check for `Success`.

### What still needs a wallet, and is therefore blocked

SC-001, SC-002 and SC-004 all require a signed-in wallet on the device, which
requires a passkey ceremony — the device owner's finger. Two independent
blockers, both recorded rather than worked around:

1. **No wallet on the test device**, and creating one is a human action.
2. **MIUI refuses the instrumented-test APK**: `INSTALL_FAILED_USER_RESTRICTED`
   for `app-debug-androidTest.apk` through `adb install`, `pm install` and with
   `verifier_verify_adb_installs=0`. Enabling "install via USB" on MIUI needs a
   Mi account. (The setting was restored to `1` afterwards.)

`CurrencyPersistenceTest` is written and committed for the moment those clear:
two `@Test`s meant to be run as **separate `am instrument` invocations** with a
`force-stop` between them, so the second reads what survived the first process
dying. Run instructions are in its header and in
[quickstart.md](./quickstart.md) §3.

The in-process half of the same claim is green today:
`aSecondMachineOverTheSameStoreAgrees` and `aSecondBookOverTheSameStoreAgrees`
build a fresh machine over the same store and read back what the first one
wrote.

---

## A success criterion this slice cannot meet, stated plainly

**SC-002 says a custom network can be "added, edited and removed". Adding is
not possible in spec 040, and no amount of shell work would make it so.**

Both routes into the add-network wizard go through the network:
`add_by_chain_id_requested` needs `fetch_chain_info`, and the search box needs
`fetch_search_index`. Both are fail-closed here, so the core never resolves a
candidate and `can_add` stays `false` — which is the core being right, not a
gap in the wiring. It refuses to save a chain whose id, symbol and contract
support it has not confirmed.

What US1 *does* deliver on the device: the display currency, the network list,
per-network endpoints, the four service endpoints, the provider keys, and
removing a custom network — each read from and written to the store the other
clients share.

**Adding a network moves to spec 041**, with the chain index it depends on.
Recorded here rather than quietly reworded in the spec, because a criterion
that gets edited to match what shipped is not a criterion.

---

## Corrections to the record (found while planning, before any code)

1. **The premise of the request was wrong, and it matters.** Contacts,
   clear signing and the explorer do *not* need their logic written in
   `vela-core`: `contacts.rs` (1,385 lines), `clear_signing.rs` (4,841) and
   `browser_history.rs` (471) are complete Crux machines on `origin/main`.
   Verified by `impl App for` sites and line counts, not recalled. Retired in
   research D1 so specs 041 and 042 do not re-litigate it.

2. **The Contacts tab is inert on purpose, and this feature is why it stops
   being.** `VelaNavHost.kt:271-280` refuses to navigate there because the
   screen shows fixtures a signed-in person would read as real data. Meanwhile
   Settings' `通讯录` row *does* push to it (`:334`) — so the leak the comment
   was avoiding exists anyway, one tap further in. Recorded in research D11;
   closing it is part of US2, not a follow-up.

3. **Ten settings fields cannot be typed into.** `VelaUrlField` renders a value
   and takes no input; there is no `TextField` of any kind under
   `feature/settings/`. The desktop sibling mis-filed the same finding as
   "blocked on missing artwork" for four features running. It is not blocked —
   `VelaTextField` is in the design system already. Research D7, sequenced as
   Phase 4, *before* the write path that needs it.

---

## Success criteria — verdicts

| # | Claim | Verdict |
| --- | --- | --- |
| SC-001 | A currency choice survives a force-stop | **Machine-verified, device-blocked.** A fresh machine over the same store reads the choice back (`CurrencyMachineTest`); the process-death half needs a wallet on the device — see above |
| SC-002 | A custom network can be added, edited, removed | **Partly met, and the gap is stated.** Edit and remove are wired; **adding is impossible in 040** because both routes to it need the network layer 041 brings |
| SC-003 | Contacts render the device's own book; empty means empty | ✅ **verified on device** |
| SC-004 | Deleting the middle of three deletes that one | **Machine-verified** (`ContactsMachineTest`); the device pass is blocked with SC-001 |
| SC-005 | Zero network requests from these surfaces | ✅ by construction — 18 `// live in 041` arms, and no HTTP client is reachable from any of the three executors |
| SC-006 | Bridge size measured and recorded | ✅ four builds, per-machine costs, a budget for 041/042 |
| SC-007 | The third machine costs no shared plumbing | ✅ `display_currency` landed as four files plus one bridge line; `contacts` the same |
| SC-008 | A Rust rename turns a test red | ✅ demonstrated and reverted, output quoted above |
| SC-009 | Existing suite stays green; the suite grows | ✅ 118 → **209**, 0 failures; `assembleDebug` green |
| SC-010 | Every gallery state still renders | ✅ untouched by construction — the fixture builders and gallery routes were not modified, and `VelaUrlField` renders byte-identically without an `onValueChange` |

## FR-017, honestly

*"No signed-in route reaches a fixture for the Settings or Contacts surfaces."*

For the **data these three machines own**, met. What remains fixture-fed on
those two screens, and why:

- **Resolved copy.** Both live builders start from a fixture-built model to get
  titles, labels and empty-state text the i18n engine has already resolved.
  That is content, not data — but it does mean `ContactsFixtures` and
  `SettingsFixtures` are now imported by production code, which is a debt: the
  copy should move to a builder of its own when a second consumer appears.
- **Surfaces whose machines are not wired**: the storage figures, the RPC
  health tiles and banner, the relayer panel, the about page, the language and
  format sheets, rows 2–3 of the accounts sheet, and the recent-activity block
  on a contact. Each needs either the network layer (041) or a machine this
  spec did not claim.

## Debts handed on

| # | Thing | Owner |
| --- | --- | --- |
| 1 | Add-network wizard (needs the chain index) | **041** |
| 2 | RPC/endpoint health, latency, fiat rates — 18 `// live in 041` arms | **041** |
| 3 | A contact's recent activity (needs the local tx store) | **041** |
| 4 | Identity resolution and recipient classification | **041** |
| 5 | Contact **add/edit form** and **favourite** control | **no artwork** — the core supports both; nothing is drawn on Android. Not blocked on wiring |
| 6 | Import / export of contacts | needs a file picker; the core supports it |
| 7 | Per-network RPC override page | drawn, but has no selection state — the detail page shows a fixed fixture network |
| 8 | Currency captions are unlocalised (English here, Chinese on web) | a content decision: needs i18n keys for currency names |
| 9 | Live builders import fixture files for copy | see FR-017 above |
| 10 | Wallet home, flows, explore, signing still fixture-fed | **041 / 042**, as scoped |

## Handover — what spec 041 should do first

1. **Read `contracts/shell-operations.md`**, then `grep -rn 'live in 041'` under
   `app-android/…/wallet/`. Eighteen arms; each says what shape it must answer
   when it becomes real. The count going to zero is how 041 knows it is done.
2. **The numeric-type trap is not theoretical.** ts-rs writes `u32`, `u64` and
   `f64` all as `number`, so the drift gate cannot see the difference and serde
   rejects it at runtime. Read the Rust struct, not the mirror. 041 wires
   balances, where the numbers are worse.
3. **Budget from data, not estimate.** 041 and 042 together add ≈ 3.1 MB per
   ABI; the per-machine marginal cost is 172 KB and falling. Re-measure with
   the method in quickstart §5 rather than assuming.
4. **The paved road should take no more work.** A machine is: one
   `bridge_object!` line, one `…Wire.kt`, one executor, one entry in the drift
   registry, one controller method per intent. If 041 finds itself editing
   `core/crux/`, something in the road was wrong and it is worth saying so.
5. **Get a wallet onto the test device before starting.** Three of this spec's
   success criteria are waiting on it, and every criterion 041 writes will be
   too.
