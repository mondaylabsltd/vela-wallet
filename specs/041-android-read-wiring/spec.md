# Feature Specification: Android Read Wiring — The Wallet Home Tells the Truth

**Feature Branch**: `041-android-read-wiring` (stacked on `040-android-live-shell`)

**Created**: 2026-09-05

**Status**: Draft

**Input**: Continuation of the Android wiring program (040 → 041 → 042). The
read path: `rpc_pool` as the base, then `balance_dashboard`, `activity_feed`,
`manage_tokens`, `token_trust` for the home; `receive_watch` and
`payment_request` for the receive flow; and the backfills spec 040 left
fail-closed and marked `// live in 041`.

## Why

After 040, a signed-in person's settings and address book are their own. The
wallet home is still a picture: **$1,383.28 and four transactions that never
happened**, verified on a device on 2026-09-05. The missing ingredient is one
thing — Android cannot read a chain.

This feature lands that ingredient and then lets every read-path machine that
has been waiting speak. The shape is the one 040 proved: a machine costs an
executor and a display-model builder, and the road built in `core/crux/` does
not change. What is genuinely new is a **transport**: one HTTP/JSON-RPC client,
driven by the `rpc_pool` machine, which every chain read in the app goes
through.

**The pool is policy, and the policy is already written.** Endpoint scoring,
bans, cooldowns, the rate-limit verdict and the fastest-endpoint race all live
in `rpc_pool.rs`. The shell contributes four things and no decisions: a POST, a
clock, a jitter draw, and the ban-map bytes. Any Kotlin that starts choosing an
endpoint is a bug in this feature.

**Backfills owed from 040.** Eighteen arms across three executors answer
fail-closed today and carry `// live in 041`:

| Where | Arms | Becomes |
| --- | --- | --- |
| `NetworkAdminExecutor` | 10 | chain-info fetch, RPC/reachability probes, `eth_getCode`, the P-256 precompile call, service health, fiat rates, pool invalidation, bundler-cache clear |
| `ContactsExecutor` | 3 | send history, identity resolution, recipient classification |
| `CurrencyExecutor` | 1 | a real USD→code rate |
| `ContactsLive` | 1 | a contact's recent activity |

The count going to zero is how this feature knows it is finished.

**Standing exclusions**: sending money, signing, the dApp browser and the
explore tab are spec 042. The camera (QR scanning, flow state `S1`) is a
separate capability, not wiring. Adding a custom network — deferred from 040
because it needs the chain index this feature lands — comes back in scope here.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The pool under everything (Priority: P1, enabling)

Every chain read in the app goes through one transport whose decisions belong
to the core. When an endpoint fails, reads keep working through the pool's next
choice; the failure is recorded by the core's ban and cooldown rules and
survives a restart.

**Why this priority**: nothing else in this feature can be built or trusted
without it, and it is the piece most easily got wrong by writing "just one
fetch" somewhere else.

**Independent Test**: point a chain's primary endpoint at a dead host, open the
home; balances still arrive, and the ban is on disk under the key the other
clients use. Restart: the ban is still there.

**Acceptance Scenarios**:

1. **Given** a failing endpoint, **When** anything reads that chain, **Then**
   the pool routes around it, and the ban/cooldown the core decided persists.
2. **Given** a rate-limited endpoint (HTTP 429), **When** it answers, **Then**
   the core's *transient* verdict applies — a cached balance and no
   "swap your RPC" alarm.
3. **Given** the settings network editor from 040, **When** a probe runs,
   **Then** it reports a real chain id and latency, and the two acknowledged
   no-ops (`invalidate_pools`, `clear_bundler_cache`) act on the real pool.

---

### User Story 2 - A home that tells the truth (Priority: P1)

A signed-in person opens the wallet tab and sees **their** balances: real
tokens across the supported networks, a real total in the currency they chose
in 040, per-chain filtering, and cached figures when the network is down —
never a fixture's $1,383.28.

**Why this priority**: it is the product's face, and today it is the largest
untrue surface in the app.

**Independent Test**: with a funded address signed in, the home's total and
token rows match what a block explorer says for the same address; with the
network off, cached figures render under the core's stale rules; the fixture
identities never appear.

**Acceptance Scenarios**:

1. **Given** a wallet holding tokens on two or more networks, **When** the home
   opens, **Then** each row shows its real balance and the header total is the
   core's aggregation, converted at the display-currency rate — or shown as the
   USD figure when no rate could be fetched (040's rule, now with a source).
2. **Given** every endpoint for one chain failing, **When** the home loads,
   **Then** that chain's cached balances render and the screen does not spin
   forever.
3. **Given** a reload, **Then** cached figures paint first and live ones
   replace them by the core's rules.
4. **Given** the person hides balances, **Then** every money figure on the
   screen honours it.
5. **Given** a token the core cannot price, **Then** the row says so rather
   than showing a zero or a guess.

---

### User Story 3 - Activity that happened, and a deposit that arrives (Priority: P2)

The activity feed lists the wallet's real transfers, deduped and day-grouped by
the core's rules. A deposit that lands while the receive screen is open is
noticed without a manual refresh.

**Independent Test**: with an address that has on-chain history, the feed lists
real transfers; sending a small amount to the address while the receive screen
is open surfaces the arrival.

**Acceptance Scenarios**:

1. **Given** an address with prior transfers, **When** the home opens, **Then**
   the feed shows them with the core's folding, dedup and day grouping.
2. **Given** the receive screen open, **When** a transfer lands, **Then** the
   watcher notices it within its polling rules and the arrival is acknowledged.
3. **Given** an EIP-681 payment link, **When** it is parsed, **Then**
   `payment_request` rules on it: valid → a prefilled receive context; garbage
   → refused in the corpus's words.
4. **Given** a contact's detail page, **When** it opens, **Then** the recent
   activity block shows that contact's real transfers — the block 040 left
   deliberately empty.

---

### User Story 4 - The honest blanks fill in (Priority: P3)

Every surface 040 left showing its unknown state starts showing a measurement:
the settings RPC-health tiles, the endpoint health rows, a contact's resolved
name, a recipient's risk classification, and the exchange rate behind the
display currency.

**Why this priority**: these are the visible half of the "fail-closed" promise
040 made. Until they fill in, the app is honest but incomplete; the moment they
do, 040's discipline pays off with no further shell work.

**Independent Test**: `grep -rn 'live in 041'` returns nothing, and each
surface that grep used to point at now shows a real value.

**Acceptance Scenarios**:

1. **Given** the settings home, **Then** the RPC-health tiles show live pool
   verdicts, not fixture latencies.
2. **Given** a contact with an ENS or Basename, **Then** the name resolves and
   is shown; when the lookup is unreachable, the address is shown and nothing
   is claimed.
3. **Given** a display currency other than USD, **Then** a real rate is fetched
   and the total converts — and when no source answers, the core's `null` rule
   still degrades to the USD figure rather than multiplying by 1.

---

### Edge Cases

- **Every fail-closed answer keeps its failure variant.** Upgrading an arm from
  "always unknown" to "ask the network" must not turn a genuine failure into a
  crash or a guess: no rate source → `rate: null`; identity lookup down →
  `null`, not a verdict; a range-capped `eth_getLogs` → the core's range rule.
- **A URL a person typed** goes through endpoint admission before any request
  is made, as the other clients do.
- **The screen is not visible.** A watcher polling while the app is backgrounded
  is battery a person did not agree to spend; the core has an activity gate and
  the shell must feed it honestly.
- **A chain that returns nonsense**: Tempo's RPC answers the same constant for
  every address and its native coin is called `USD`. The core's own
  `fee_policy::TEMPO_CHAIN_IDS` is what decides that, never a threshold
  invented here.
- **Balances are human decimals, not raw units**, on the wire. Multiplying a
  raw-unit figure by a price is a 10^18 error that is invisible until a price
  exists — which is exactly when this feature makes prices exist.
- **Numeric types**: `u32`, `u64` and `f64` are all `number` in the generated
  mirrors. The Rust struct is the reference, and this feature carries far more
  numbers than 040 did.

## Requirements *(mandatory)*

### Functional Requirements

**The transport**

- **FR-101**: All chain reads MUST dispatch through the `rpc_pool` machine. No
  other component may choose an endpoint, and no component may issue a chain
  request of its own.
- **FR-102**: The ban map MUST persist under the key and byte shape the other
  clients use, so a device that banned an endpoint agrees with its siblings.
- **FR-103**: The shell MUST contribute only transport, clock, jitter and
  storage to the pool — never a retry decision, a ranking or a verdict.
- **FR-104**: A user-configured endpoint MUST pass endpoint admission before
  any request.

**The home**

- **FR-105**: Balances, the header total, per-chain filtering, the assets list
  and the activity feed MUST come from `balance_dashboard`, `activity_feed`,
  `manage_tokens` and `token_trust` through live builders, with the fixtures
  remaining gallery canon.
- **FR-106**: A balance figure the core reports as cached or stale MUST be
  presented as such, and a rate-limited chain MUST be presented as transient.
- **FR-107**: A token with no price MUST render its no-price state rather than
  a zero or an inferred value.

**The receive flow**

- **FR-108**: `receive_watch` MUST drive deposit detection on the receive
  screen, with the core's polling phases and its activity gate honoured.
- **FR-109**: `payment_request` MUST rule on EIP-681 input; the shell parses
  nothing and validates nothing.

**The backfills**

- **FR-110**: All eighteen `// live in 041` arms MUST become real, each keeping
  the failure variant it answers with today for genuine failures.
- **FR-111**: Adding a custom network MUST work, using the chain index this
  feature lands — closing the criterion 040 recorded as impossible.

**Honesty**

- **FR-112**: After this feature, no signed-in route MUST reach a fixture for
  the wallet home, the assets or activity screens, the receive flow, or the
  settings health surfaces.
- **FR-113**: No business rule MAY be written in Kotlin. Where a rule appears
  to be needed, it is already in the core or the gap is recorded for the core.

### Key Entities

- **Pool**: the `rpc_pool` machine and its per-chain endpoint state. One per
  process; the only thing in the app that knows where a chain lives.
- **Call**: one JSON-RPC request the pool authorised, identified by `call_id`
  so a late answer is dropped by construction.
- **Ban**: an endpoint the core has ruled against, with the reason and the
  cooldown, persisted.
- **Balance snapshot**: what the dashboard machine holds per account and chain,
  including whether a figure is live or cached.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-101**: The wallet home shows the signed-in address's real balances and
  total, matching a block explorer for the same address, on a device.
- **SC-102**: With one chain's endpoints unreachable, the home still renders
  that chain from cache and the rest live; the ban persists across a restart.
- **SC-103**: The activity feed lists real on-chain transfers for an address
  with history, day-grouped by the core.
- **SC-104**: A deposit sent to the address while the receive screen is open is
  noticed without a manual refresh.
- **SC-105**: `grep -rn 'live in 041'` returns **zero** matches, and each
  surface it used to name shows a real value.
- **SC-106**: A custom network can be added on the device (040's deferred
  criterion), and survives a restart.
- **SC-107**: Zero chain requests are issued outside the pool — verifiable by
  inspection and by a test that fails if any executor gains its own client.
- **SC-108**: The bridge's stripped `arm64-v8a` delta for the machines this
  feature adds is measured and recorded before the work lands.
- **SC-109**: The existing suite (210 unit + 8 instrumented) stays green and
  grows; `assembleDebug` passes.
- **SC-110**: Every gallery state that existed before this feature still
  renders.

## Assumptions

- **The core is complete for these seven machines.** No `vela-core` change is
  expected; if one proves necessary it is a rule, it goes in Rust, and it
  brings the repository's regeneration gates with it.
- **The device has a signed-in wallet** — true as of 040's third device pass.
  A funded address is needed for SC-101/103; an unfunded one still exercises
  every path with zero balances.
- **Network access is available** on the test device.
- **040's road does not change.** If this feature finds itself editing
  `core/crux/`, that is a signal something in the road was wrong, and it is
  recorded rather than absorbed.
- **Out of scope, by construction**: send, signing, clear signing, the dApp
  browser, the explore tab (042); the camera; any change to the drawn visual
  language.
