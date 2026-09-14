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
