# Results — 052 iOS Money Wiring

Written as the work lands. **[tasks.md](./tasks.md) is the handoff** — it holds
the commands, what is next and the traps. This file holds what happened.

---

## Phase 0 — Baselines

Branch point: `051-ios-read-wiring` @ `6e693b26`.

| | |
|---|---|
| `@Test` functions | **356** (331 hermetic + 25 behind `-DVELA_LIVE_TESTS`) |
| XCUITest `func test` | **15** (11 acceptance, 2 sweep, 2 launch) |
| Swift files under `VelaWallet/` | 179 |
| Committed `vela_core_uniffi.swift` | **353,772 bytes** — and this cut expects it to end at exactly that |
| Literal-audit violations | **35** in 146 files (never green on `main`; the gate is "no new ones") |
| `// live in 052` markers | **3** |
| Device Debug dylib | measured at phase 1, when the first device build runs |

### The inventory that changed the cut's shape

Phase 0's first job was to stop guessing what the repository contains — 051's
most expensive mistake was an assertion from memory. What the count found:

| | |
|---|---|
| `bridge_object!` exports on this branch | **23 of 23** |
| …driven by Swift | **10** |
| `user_op_*` free functions exported | **8 of 8**, plus the five relay-classification helpers |
| Swift references to any of them | **zero** |

Android's 043–049 (merged to `main` as PR #194, which this branch already
carries) exported everything this cut needs. So **052 adds no bridge line and
regenerates no bindings** — the first iOS cut where the five-to-eight-minute
xcframework round trip is off the critical path entirely. SC-011 turns that
into a gate rather than an observation: if `vela_core_uniffi.swift` has a
diff at closeout, something was added that the plan says is unnecessary.

### The three markers this cut inherits

| File | Line | What it answers today |
|---|---|---|
| `Features/Settings/NetworkAdminExecutor.swift` | 263 | `clear_bundler_cache` → `bundler_cache_cleared`, a truthful no-op: there is no bundler client, so no cache |
| `Features/Contacts/ContactsExecutor.swift` | 108 | `load_send_history` → `history_failed`, **deliberately** not an empty list: an empty list would tell the core nobody has ever been paid and make every address wear the poisoning warning forever |
| `Features/Contacts/ContactsLive.swift` | 163 | the contact detail's 最近往来 renders the drawn empty state |

---

## Phase 1 — the money plumbing, and two sentences that were being dropped

`Core/RelayClient.swift` (582), `Core/UserOpSpine.swift` (328),
`Features/Send/Core/UserOpSigner.swift`, `CoreHTTP.getREST`,
`RpcPool.bundlerBase`, and `TxRecords`' write half — all ported from
`app-android/.../feature/send/core/`, which is the desktop's
`executor/user_op.rs` and `executor/relay.rs`. No screen changed. 25 tests say
the order is right before anything can spend.

### The order is the deliverable, and it is tested as an order

`UserOpSpineTests` does not merely assert that a bad precondition fails — it
asserts **the signer was never called**, and in one case that the calls made
were exactly `["eth_getCode", "eth_call"]` and nothing after. A passkey prompt
raised for an operation the relay must reject is a Face ID sheet somebody
dismisses for nothing, and "we refuse eventually" is not the same promise.

`theChallengeIsTheCoresThirtyTwoByteSafeOpHash` pins the other half: what the
ceremony signs is 32 bytes and comes from `user_op_safe_op_hash`. A shell that
pre-hashed would produce a signature over the wrong bytes and find out from a
reverted transaction.

### Defect 1 — the pool was dropping the relay's own sentence

`rpc_pool`'s shell half stored a response body **only when there was no JSON-RPC
error**. But the core routes an error that is neither permanent nor transient to
`Route::Success` (`classify_response_error`) — a revert, an "out of gas" and a
relay's refusal of a user operation are all valid answers to a question. So the
caller received `.ok(nil)`: "the chain said no" and "the method returned null"
became the same thing, and the relay's sentence — the one input
`classify_relay_rejection` and `parse_existing_user_op_hash` need — was gone.

`RpcOutcome` gains `.rpcError(code:message:)`. Two latent defects in the READ
path close with it, both of which were already shipping:

| Where | Was | Is |
|---|---|---|
| `TokenTrustExecutor` `eth_getLogs` | an errored query answered `logs: []` — a scan reporting "nothing arrived" on the strength of an error, which the core would then believe | `failed` |
| `TokenReads` `eth_getBalance` | an errored read left the balance `nil` and the chain marked **healthy**, so the total quietly omitted a coin somebody owns | the chain is `failed`, which is 051's own rule applied |

### Defect 2 — the translator can eat the marker that prevents a double spend

`relay_error_message` is a **translator**, not a pass-through: a message
matching a known rung is replaced wholesale by its human sentence. The relay
marks an operation already pending for a nonce with `[existingHash:0x…]`, and a
duplicate submit is exactly an `AA25 invalid account nonce` condition — so the
one case where the marker matters most is the one where the translation removes
it. Pinned by test:

```
relay_error_message("AA25 … [existingHash:0xabc123]")
  → "Transaction nonce mismatch. Please try again."
parse_existing_user_op_hash(translated) → nil
parse_existing_user_op_hash(raw)        → "0xabc123"
```

The spine reads the raw json first, then the translated message. **Android and
the desktop parse only the translated text.** Reported rather than quietly
diverged; it is theirs to fix or to rule on.

### Gates

Hermetic tests 331 → **356**; live (flagged) 25; device UI 11. Literal
violations 35. Zero lines under `rust/crates/vela-core/src/app/`; zero corpus
delta; `vela_core_uniffi.swift` unchanged.

---

## An inherited device failure, attributed rather than inherited silently

The device acceptance suite is **10 of 11** green on the founder's iPhone.
`testAddingATokenByContractFindsItAndKeepsIt` fails after 77 s.

**It is not this cut's.** Checked out `0e2fcb1e~1` — the tree before phase 1 —
and ran the same test on the same phone: it fails there too, in 77 s. Recorded
before changing anything else, because a failure inherited and a failure caused
are different conversations.

### What the device says

The accessibility tree at the moment of failure: the field carries the typed
contract, the network row reads Ethereum, the status line reads
**正在搜索所有网络…**, and 添加到钱包 is disabled. So the address was accepted
(the core would not be `detecting` otherwise) and the sweep ran — and no chain
answered inside sixty seconds.

Two hypotheses were tested and **both were wrong**, which is worth recording so
the next person does not re-run them:

1. *An accumulated ban map.* `vela.rpc.banned` holds facts about endpoints and
   persists, so a phone tested many times could start every sweep from a banned
   pool. The test now empties it for its own launch through the argument domain
   (`-vela.rpc.banned "[]"`), which is better hygiene either way — a test whose
   result depends on how often the device has been tested before is a bad test.
   **It changed nothing.**
2. *The screen hiding an answer it already had.* `FlowsLive.result` read
   `detecting` before `found`, so a card the core had already published was
   covered by the spinner until the last chain settled. Fixed — a found card now
   outranks the spinner, with a test — and it is a real improvement. **It did
   not fix this**: `found` is genuinely empty on the device.

### What it actually is

Running the probe's own live suite **on the phone** rather than through the UI:

```
simulator   aRealContractAnswersWithItsOwnMetadata   passed  1.7 s
            [live] USDT on Ethereum: [symbol: USDT, name: Tether USD, decimals: 6]
device      aRealContractAnswersWithItsOwnMetadata   FAILED  36.0 s
device      theProbeIsCaseInsensitiveOnChain         FAILED  35.9 s
```

The probe is the same code on both. So this is **the phone's reach to the
Ethereum-mainnet endpoints**, not the add-token screen — Gnosis answers on that
same phone (the hero shows its real xDAI), and the history test passes because
the local store is the feed's source of truth and needs no chain at all.

A third finding, free: `anAddressThatIsNotATokenAnswersNothing` **passed** on the
device — in 36 seconds. It expects nothing, so a total blackout looks exactly
like a correct negative. A test that cannot tell "the chain says no" from "the
chain never answered" is a test that will one day certify a dead network.

**Open, and the founder's to settle**: whether that phone is on a network that
blocks or throttles those endpoints. Until it is settled, SC rows that depend on
Ethereum mainnet from the device are reported as what they are rather than
waved through.

---

## Phase 2 — the parallel space

A second **static** xcframework (`vela-dev-fixtures-uniffi` gains a `staticlib`
crate type), built by `rust/scripts/build-ios-dev-fixtures.sh`, linked only by
the Debug configuration; `Dev/vela_dev_fixtures.swift` and
`Dev/ParallelSpaceBinding.swift` are in `EXCLUDED_SOURCE_FILE_NAMES` for
Release. `Core/ParallelSpaceHook.swift` is the always-compiled seam.

### SC-010, measured with a control

A measurement of an absence is worthless without proof the instrument can see a
presence, so both configurations were built fresh and grepped the same way.
The app's code lives in `VelaWallet.debug.dylib` inside the bundle, **not** in
the 72 KB stub named `VelaWallet` — grepping the stub reports zero for
everything and proves nothing.

| | Release | Debug (device) |
|---|---|---|
| `vela_dev_fixtures` symbols | **0** | 567 |
| `PARALLEL SPACE` strings | **0** | 1 |
| `ParallelSpaceBinding` / `FixtureUserOpSigner` | **0** | 93 |

The badge went behind `#if DEBUG` for the middle row. A dormant "PARALLEL
SPACE" literal in a shipped wallet is neither key material nor a door — but it
is the first thing a reviewer would ask about, and the answer should be that it
is not there.

### On the founder's iPhone

`testTheParallelSpaceOpensOnTheGoldenSafeAndClosesCleanly` passed on the device
in 12.4 s: the badge on screen, the header reading **Parallel One ·
0x88cC…6894** — the golden multi-key Safe every other client derives — and the
badge gone again after a launch with the door closed. Address and badge only,
deliberately: a phone with no reach to a chain still has the right wallet open,
and asserting a figure would fail for a reason that has nothing to do with the
door.

### Two defects of my own making, both found by running it

1. **`createdAtISO` is not a spelling the core knows.** The record was
   hand-built with camelCase field names. The core's hand-written reader accepts
   `publicKeyHex` and `createdAt` — but only to read a list the retired Expo
   client wrote; what every client WRITES is snake_case
   (`app/mod.rs:160`). `createdAtISO` matches neither, so the whole account list
   failed to deserialize and the app opened on **Welcome** with the record
   sitting on disk. The failure is silent by design: a list the core cannot read
   is refused rather than half-adopted. This is the field-by-field hazard
   `AccountStore`'s own doc warns about, committed by the person who had just
   read the warning.
2. **A persisted door poisons every later run.** The space survives a relaunch
   on purpose — a device test spanning a relaunch must not fall out of it
   halfway — and the cost is that a session left inside it is inherited. Three
   UI tests failed against a fixture wallet nobody had asked for. Every UI test
   launch now states its own environment through the argument domain
   (`-vela.parallelSpace "0"`), which outranks the persisted value without
   writing anything. Proven by leaving the simulator **deliberately inside** the
   space and running the whole suite: all green.

### Recorded, not fixed

The badge overlaps the wallet header's top edge, clipping the account name by a
few points, because `WalletScreen` draws under the safe area. A `safeAreaInset`
did not move it. Cosmetic, and only in a configuration that does not ship.

### Gates

Hermetic tests **357**; device UI 11 → **12** (the new one green on the phone).
Literal violations 35. Zero lines under `rust/crates/vela-core/src/app/`; zero
corpus delta; `vela_core_uniffi.swift` unchanged. The one Rust edit is a
`crate-type` line, which adds no code and no export.

---

## Phase 3 — `fee_policy` + `send`, up to the confirm screen

Three new machines' shells (`FeeWire`/`FeeExecutor`/`FeeStore`,
`SendWire`/`SendExecutor`/`SendStore`), one live builder (`SendLive`), and the
two fields the form was drawn without.

### The picker is the person's own money now

On the founder's iPhone, in the parallel space: 转账 opens on
**xDAI · Gnosis · 0.53097**, and tapping it opens 发送 xDAI with that token's
card, an amount field, a recipient field and a 继续 the core keeps disabled
until it is satisfied. The fixture list — POL, ETH, somebody's USDT — is gone.

### Four defects, and the order they were found in matters

They are recorded in the order they surfaced because each one hid the next.

1. **The picker was empty — not fixtures, nothing.** A screen with no rows
   cannot say whether the shell answered badly or the core refused the answer,
   so the first move was a hermetic test that drives the REAL machine with a
   scripted holding. It passed. That split the question in half and made the
   rest quick.
2. **`network_admin` had never been opened.** The send machine resolves every
   holding against the chain list, and the settings machine only booted when
   somebody visited 设置. It is now opened from the home beside the currency
   machine, for the sharper version of the same reason 051 gave: which networks
   exist is app-wide. **This is Android's contact-picker defect with different
   nouns** — a machine that only boots on its own page, read from another page.
3. **The picker was a one-shot read of the balance at the instant the flow
   opened**, which on a cold start is before any chain has answered. The core
   has `RefreshTokens` for exactly this; a balance that lands later now
   re-fetches. Without it the list was empty *forever*.
4. **`Open` was re-fired on every stage change and reset the machine.** The
   task was keyed on the DERIVED state, so reaching the form re-entered the
   flow and bounced straight back to the picker. Keyed on `flows.top` instead,
   which stays `.sd1` for the whole journey — the lifetime that event actually
   has.

### A test that passed while the screen was wrong

Worth its own heading, because it is the failure mode this program keeps
finding. The first version of the acceptance test waited for "a text field and
xDAI" after tapping a token. **It passed on the device, and the screenshot
showed the picker** — which has a search box and the row just tapped. A test
that cannot tell "the form opened" from "nothing happened" is worse than no
test, because it certifies the defect.

It now waits for 收款人, which only the form has, and asserts 选择代币 is gone.
It failed immediately, which is how defect 4 was found.

### Three fixtures the device screenshot caught

Visible only once the form was real:

| | Was | Is |
|---|---|---|
| the title | **发送 USDT** on a wallet holding xDAI — a sentence about somebody else's money | `send.sendTitle` with the selected token |
| the fee row's badge | **ETH**, the drawing's, on every network | the chain the fee is actually paid on |
| the recipient's identicon | a face drawn from the **zero address**, sitting over an empty field as though somebody had been chosen | an empty seed draws a themed circle — only a real address earns a face, which is the founder's anti-poisoning rule the code had in a comment and not in the pixels |

### What phase 3 deliberately did not do

Sign anything. `submit_user_op` is wired in the executor and the spine behind it
is tested, but nothing on screen reaches it until phase 4. `simulate_calls`
answers `null` (055), `add_network` answers `error` (its only entry is the
scanner, 055), and split/sweep/batch are 054.

### Gates

Hermetic tests 357 → **359**; device UI 12 → **13**, all green on the founder's
iPhone. Literal violations 35.

**Measured against 052's branch point rather than the merge base** — the merge
base is `origin/main`, and 050/051 sit between the two, so a diff against it
reports their work as this cut's:

| | |
|---|---|
| `rust/crates/vela-core/src/app/` | empty |
| corpus (`i18n_catalogs/`, `assets/i18n/`) | empty |
| `vela_core_uniffi.swift` | **empty** — SC-011 holds |
| `app-web/`, `app-desktop/`, `app-android/`, `app-browser-extension/` | empty |
| all of `rust/` | a `crate-type` line and one new build script |

---

## Phase 4 — sign and submit. Money moved.

**SC-001 is met.** From the founder's iPhone, in the parallel space, dust left
the golden Safe on Gnosis:

| | |
|---|---|
| user operation | `0xcf9fcae68769c6d6a243427717ee871ac57c35710aa01d5764635032bd269195` |
| transaction | `0x151d63c8f6c4c59e886e6305cec679f6f999781de8d3665f386c57d77ff557b9` |
| block | `0x2e014dd` (Gnosis) |
| sender | `0x88cCA0EeDbF2C4426110bbFc998F048689266894` |
| receipt `status` | `0x1`, `success: true` |

Signed by the fixed keyset, so **no biometric prompt** — which is what the
parallel space exists to make possible. The founder's own passkey send
(SC-002) still wants a finger and is owed.

### Five defects between "wired" and "moved"

Every one of them was invisible on the simulator or in a hermetic test, and
each was found by looking at what the phone actually showed.

1. **The bundler endpoint had no chain.** `RpcEndpoints.collectBundlers`
   returned the bare relay base for every chain, but a bundler pool stores
   JSON-RPC URLs as `${base}/${chainId}` — the core strips that suffix itself
   when it wants the REST base (`rpc_pool::strip_chain_suffix`). So every
   bundler call went to the relay's root and answered nothing.
   **Spec 051 made no bundler calls at all**, so a list that was wrong for a
   whole cut looked right. It surfaced here as a fee quote that never settled,
   with no error anywhere: the shell answered "no quotes", the core waited, and
   the screen said 估算中… forever. With the suffix, the same relay answers
   three fee assets and a gas price.
2. **The fee asset kind was encoded as a tagged object.** `FeeAssetKind` is a
   fieldless enum — `"native"`, not `{"type":"native"}` — so serde rejected the
   whole in-band result and the machine waited for an answer that had already
   come. Found hermetically once the scripted relay was made to answer
   everything: the test hung exactly as the device did.
3. **`Continue` was never dispatched.** The form's CTA pushed a screen instead
   of telling the machine, and the screen renders by the machine's stage — so
   the button did nothing at all. A push that the core has not agreed to
   renders the state it is still in, which looks precisely like a dead control.
4. **The fee row showed the drawing's number while estimating.** `0.0021 ETH ·
   ≈$0.55` on a Gnosis send: the fixture's fee, presented as this one's. A
   number in the fee slot is a promise about what this costs. It now says
   估算中… and then the real figure, and `—` when there is none.
5. **`onDisappear` re-armed the flow's door mid-journey.** SwiftUI rebuilds a
   view for reasons that have nothing to do with the journey; the rebuild sent
   `Open` again and the form bounced back to the picker. Leaving is now the
   CORE's leaving — the `close` port — not a view's disappearance.

### A hang is not an answer

`FeeStore.quote` now has a deadline. A quote that does not settle in 45 seconds
is reported as a failure rather than held open, because the `await` behind it
holds `estimate_fee` open, which holds the confirm gate shut, and nothing on
screen says why. `RpcPool` learned the same lesson from the other side in 051
(it refuses before boot rather than hanging); this is that rule applied to the
second facade in the client.

### What is tested rather than watched

- **One prompt per attempt** (FR-009), counted on the signer, with a cancel
  reported as a cancel — the core routes that back to confirm rather than
  raising an error surface over a ceremony the person stopped themselves.
- **Persist before track** (FR-006): the store is read from *inside* the
  tracker handoff, which is where an asynchronous write would still be empty.
  The row is `pending`, carries a user-operation hash, has no transaction hash
  yet, and its timestamp is in **seconds**.

### Gates

Hermetic tests 359 → **362**; live (flagged) grew by `RelayLiveTests`, which
prints what the real relay answers — the suite that found defect 1. Device UI
13. Literal violations 35. Against 052's branch point:
`rust/crates/vela-core/src/app/`, the corpus and `vela_core_uniffi.swift` are
all unchanged.

### Owed

SC-002 (the founder's own passkey, one prompt), SC-003 (force-quit and resume —
phase 5's tracker), SC-005 (each refusal driven and read), SC-006 (change the
fee token; the relay offers three on Gnosis, so this one is now drivable).

---

## Two findings raised to the founder — one resolved, one open

### 1. ~~That iPhone cannot reach the endpoints this app reads~~ — resolved

**Withdrawn the same day.** The founder restored the phone's network and every
measurement inverted:

| On the device | Without network | With network |
|---|---|---|
| `aRealContractAnswersWithItsOwnMetadata` | failed, 36.0 s | **passed, 2.3 s** |
| Gnosis balance read (chain 100) | `failed: true` | **passed** |
| `oneSessionAnswersTwoMachines` | failed, 146 s | **passed, 1.7 s** |
| `testAddingATokenByContractFindsItAndKeepsIt` | failed, 77 s | **passed, 23.0 s** |
| The whole acceptance suite | 10 of 11 | **12 of 12** |

So the diagnosis held: the add-token screen was never the problem, and neither
was anything in this cut. Both hypotheses that were tested and recorded as wrong
stay wrong, and the two changes they produced stay because each is right on its
own terms — a found card outranking the spinner, and a test that empties the ban
map rather than inheriting one.

**The finding that outlives the outage** is the one worth carrying:

> The home renders the **cached** total, so a suite that reads money was green
> on a phone with no network at all. The device screenshot in this file shows it
> plainly — ¥3.56 in the hero with the 资产 list empty underneath.

That is 051 working as designed, and it means an SC row that says "the home
showed real money" is not by itself evidence of a chain read. Where a criterion
is about reaching a chain, the evidence has to be a figure that could only have
come from one — a fresh receipt, a balance that changed, a hash.

A second finding keeps its sting: `anAddressThatIsNotATokenAnswersNothing`
**passed** during the outage, in 36 seconds. It expects nothing, so a total
blackout looked exactly like a correct negative. A test that cannot tell "the
chain says no" from "the chain never answered" will one day certify a dead
network.

### 2. A Release build cannot be compiled on this toolchain

```
While running pass #242738 SILFunctionTransform "EarlyPerfInliner"
  on SILFunction "@$s10VelaWallet9CoreStoreCfD"
  for 'deinit' (Core/CoreStore.swift:45)
Apple Swift version 6.2.4 (swiftlang-6.2.4.1.4)
```

The optimiser crashes. **Not this cut's**: reproduced at `2bcdce54`, the 051
code with only spec documents added. `xcodebuild archive -configuration Release`
fails; `SWIFT_OPTIMIZATION_LEVEL=-Onone` builds, which is how SC-010 was
measured.

This is a **launch blocker** independent of 052 — an unoptimised wallet is not
what should ship — and the fix is upstream's or a local workaround
(`@inline(never)`, or restructuring that `deinit`). Recorded here because this
cut is where it surfaced, and named separately so it is not read as a
consequence of the parallel space.

---

## Success criteria — verdicts

Filled at closeout. Every row says **device-verified** or **test-only**; a
simulator run is preparation, never proof.

| | Criterion | Verdict |
|---|---|---|
| SC-001 | dust leaves the golden Safe from the iPhone | |
| SC-002 | the same send from the founder's own passkey, one prompt | |
| SC-003 | submit, force-quit, relaunch — the row reaches confirmed | |
| SC-004 | cancel at the assertion; the prompt count is one | |
| SC-005 | every refusal driven and read; no raw units | |
| SC-006 | the fee token changes and the quote follows | |
| SC-007 | the live route reads no send fixture | |
| SC-008 | the drift gate is exhaustive for all three families | |
| SC-009 | the three markers are gone | |
| SC-010 | a Release archive carries no fixture symbol | |
| SC-011 | no core, corpus, bindings or sibling-client change | |
