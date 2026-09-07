# Feature Specification: iOS Read Wiring — the Wallet Tells the Truth

**Feature Branch**: `051-ios-read-wiring` (stacked on `050-ios-live-shell`)

**Created**: 2026-09-05

**Status**: Draft

**Input**: Founder description: "把 iOS 接到 vela-core 的读路径七台机器：rpc_pool 路由权威 + balance_dashboard / activity_feed / manage_tokens / token_trust / receive_watch / payment_request。"

## Why

After 050 the iPhone's address book and its networks are the person's own. The
**home screen still shows somebody else's money**: `$1,383.28` and a fixture
token list, under their real name, their real address and their real identicon.
That is the most directly wrong screen in the client, and it is what this cut
fixes.

It is also where iOS stops being able to borrow from itself. 050's three
machines were storage machines — the outside world was a `UserDefaults` string
and one operation-local probe. Every machine here needs the network, and needs
it **routed**: which endpoint, after which failure, under which ban, is the
single decision `rpc_pool` exists to own.

## Scope — bigger than 050, and honestly so

| | 050 | 051 |
|---|---|---|
| Machines | 3 | **7** |
| Core lines driven | 4,698 | **7,846** |
| Operations | 27 | **35** |
| Shell service layer to port | ~600 | **~2,600 lines** |

The last row is the shape 050 did not have.
`BalanceOperation::FetchTokens { address, force, pull }` names **no chain and no
URL**: the core delegates the whole multi-chain fetch to the shell and rules
only on what comes back. So this cut ports a service layer as well as writing
executors — web's `wallet-api.ts` (788), `rpc-pool.ts` (334), `abi.ts` (342),
`chains.ts` (231), `activity.ts` (203), `token-metadata.ts` (159) and the
price/cache group.

**What is NOT ported, because it already exists in Rust and reaches Swift
today**: keccak, hex and quantity codecs, `function_selector`,
`decode_calldata`, `abi_encode_address/uint256/bytes32`, Safe derivation — all
exported by `vela-core-uniffi` since spec 019. And every routing *rule* — the
six-tier source scoring, EMA latency, cooldowns, temporary and permanent bans,
the four-way error classification, the three-pass sweep, the all-banned
self-rescue — which is `rpc_pool.rs`'s 1,975 lines and is exactly what the shell
must not reimplement.

**The one genuine gap**: Multicall3's `aggregate3((address,bool,bytes)[])` is a
dynamic array of tuples, and the bridge exports no encoder for it. Web
hand-rolled it in `abi.ts`. Where that encoder should live — a fourth hand-roll
in Swift, or a pure helper in Rust reached through the bridge — is research D1,
and it is a **measurement** (bridge bytes) rather than a preference.

## Out of Scope

- **Money.** No `send`, no `sign_request`, no bundler, no `fee_policy`. Spec 052.
- **Explore and the dApp surface.** Spec 053.
- **050's blocked surfaces.** The add/edit contact form and the favourite
  control stay blocked on drawings that do not exist. That boundary was recorded
  in 050's closeout and this cut does not move it.
- **Signing anything.** This is the read path. A seeded read-only account is a
  legitimate test subject here and would not be in 052 — see FR-010.
- **Any machine change under `rust/crates/vela-core/src/app/`.** New *pure
  helpers* are in scope if D1 concludes that way; machines are not.
- **No corpus regeneration.** Existing keys only.

## User Scenarios & Testing

### User Story 1 — The wallet shows the person's own money (Priority: P1)

Somebody with a wallet opens the home screen and sees their real balance across
their real networks, priced — or an honest absence where a chain could not be
reached.

**Why this priority**: it is the wrong screen this cut exists to fix, and the
first time iOS reads a chain *for the person* rather than for a probe.

**Independent Test**: seed the golden Safe's address, open the home, and compare
the total against a direct `eth_getBalance` on Gnosis taken independently.

**Acceptance Scenarios**:

1. **Given** a wallet with a funded address, **When** the home opens, **Then**
   the balance is that address's, from the chain — never a fixture figure.
2. **Given** one chain unreachable, **When** the home renders, **Then** that
   chain says so and the others still show; the total does not silently count
   it as zero.
3. **Given** no price source, **When** an asset renders, **Then** the amount
   shows and the fiat figure degrades — `rate: null` is not `1`, as in 050.
4. **Given** a cold start, **When** the home opens, **Then** the cached balance
   renders first and is replaced by the fetched one — never a spinner over a
   figure the app already knows.

---

### User Story 2 — Routing is one decision, made once (Priority: P1)

Every chain read in the client goes through `rpc_pool`, so a banned endpoint is
banned for everybody and the fastest one is raced once.

**Why this priority**: it is the architectural point of the cut. Two callers
with their own endpoint lists is how the Expo client got a ban map that
disagreed with itself.

**Acceptance Scenarios**:

1. **Given** several read machines, **When** they call a chain, **Then** every
   call is routed by one shared `rpc_pool` session — not one per screen.
2. **Given** an endpoint that fails, **When** it is banned, **Then** a later
   call from a *different* machine does not retry it.
3. **Given** every endpoint for a chain banned, **When** a call arrives,
   **Then** the core's self-rescue runs rather than the chain being dead.

---

### User Story 3 — Activity, receive and tokens (Priority: P2)

The activity feed, the receive watcher and the token list read real state.

**Acceptance Scenarios**:

1. **Given** a funded address, **When** activity loads, **Then** it lists real
   transfers grouped by day.
2. **Given** the receive screen open, **When** a deposit lands, **Then** it is
   noticed without a manual refresh.
3. **Given** a custom token added, **When** the list renders, **Then** its
   metadata came from the chain and survives a relaunch.
4. **Given** a token nobody asked for arriving by transfer, **When** it renders,
   **Then** `token_trust`'s verdict is what gates it — the shell never decides
   a token is safe.

## Requirements

- **FR-001 (Core decides, shell performs)**: every rule stays in the core. The
  shell fetches, decodes and caches; it never decides which endpoint, when to
  ban, whether a result is complete, or whether a token is trustworthy.
- **FR-002 (One pool)**: a single app-level `rpc_pool` session serves every
  caller. The ban map, endpoint stats and race winners are facts about the
  network that all callers share.
- **FR-003 (Every operation is answered exactly once)**: 050's rule, unchanged,
  including the loud answer for an unrecognised tag.
- **FR-004 (Fixtures stay canon)**: no `*Fixtures.swift` loses or alters a
  constant; galleries and the screenshot sweep render unchanged.
- **FR-005 (Storage bytes)**: `vela.balanceCache`, `vela.customTokens`,
  `vela.transactionHistory` and the rest keep their cross-client key and field
  names, through `VelaStore`.
- **FR-006 (Ported, not reinvented)**: service code ported from web carries a
  provenance header naming the source file. Where the bridge already has the
  primitive — hex codecs, selectors, calldata decode, Safe derivation — it is
  used, never re-implemented in Swift.
- **FR-007 (050's markers come down)**: the four arms marked `// live in 051`
  are flipped live, and the table of them is the handoff contract.
- **FR-008 (Waiting is not fixtures)**: before the core has ruled, a neutral
  surface. A **cached** figure is not waiting and is not a fixture — it is the
  last thing the core knew, and the drawing has a state for it.
- **FR-009 (`rate: null` is not `1`)**: unchanged from 050, and now load-bearing
  on every figure on the home screen.
- **FR-010 (A seeded account is a read-path fixture, and says so)**: the dev pin
  that seeds an address exists to test **reads**. It must be impossible to
  mistake for a signed-in wallet — it holds no key, and 052 must not inherit it
  as a way to skip a signature.
- **FR-011 (No machine changes)**: zero lines under
  `rust/crates/vela-core/src/app/`; zero corpus delta; no other client touched.

## Success Criteria

- **SC-001**: the home shows the golden Safe's real Gnosis balance, matching a
  direct `eth_getBalance` taken independently. **On the device.**
- **SC-002**: one `rpc_pool` session serves every machine; a ban set by one
  caller is observed by another. Proven by test, not asserted.
- **SC-003**: an unreachable chain renders as unreachable, not as zero.
- **SC-004**: the four `// live in 051` arms from 050 are live, and 050's
  `contacts::load_send_history` is the only one left waiting on 052.
- **SC-005**: galleries unchanged; every `*Fixtures.swift` diff additive.
- **SC-006**: the Swift test count strictly increases; build and the device
  acceptance suite green at every phase boundary.
- **SC-007**: zero machine changes under `rust/`; zero corpus delta; zero lines
  under the other four clients.
- **SC-008**: every P1 scenario confirmed on `shelchin's iPhone`, with evidence
  in results.md. A simulator run is preparation, never proof.
