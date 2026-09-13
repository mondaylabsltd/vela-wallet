# Feature Specification: Desktop Read Wiring — the Wallet Tells the Truth

**Feature Branch**: `031-desktop-read-wiring` (stacked on `030-desktop-live-shell`)

**Created**: 2026-09-04

**Status**: Draft

**Input**: Founder description: "把 desktop 接到 vela-core 的读路径七台机器：rpc_pool 路由权威 + balance_dashboard / activity_feed / manage_tokens / token_trust / receive_watch / payment_request。"

## Why

After spec 030 the desktop's settings and address book are the person's own, and the
**wallet home still shows somebody else's money**: `$1,383.28` and a fixture token
list, under their real name and address. That is the most directly wrong screen in
the client, and this cut is what fixes it.

It is also where the desktop stops being able to borrow. 030's three machines were
storage machines — the whole outside world was a JSON file. Every machine here needs
the network, and needs it *routed*: which endpoint, after which failure, under which
ban, is the single decision `rpc_pool` exists to own.

## Scope — bigger than 030, and honestly so

| | 030 | 031 |
|---|---|---|
| Machines | 3 | **7** |
| Core lines driven | 4,698 | **7,846** |
| Operations | 26 | **35** |
| Shell service layer to port | ~0 | **~3,000 lines** |

The last row is the one that matters, and it is a shape 030 did not have.
`BalanceOperation::FetchTokens { address, force, pull }` names **no chain and no
URL**: the core delegates the entire multi-chain fetch to the shell and rules only on
what comes back. So this cut ports a service layer as well as writing executors —
web's `wallet-api.ts` (788), `activity.ts` (203), `token-metadata.ts` (159),
`rpc-pool*.ts` (556), `chains.ts` (231), `abi.ts` (342) and the price/rate group.

**What is NOT ported, because it already exists in Rust**: calldata decoding and
selector maths (`vela-core/abi.rs`, 539), hex/quantity codecs and keccak
(`primitives.rs`), Safe address derivation (`safe.rs`), and every routing *rule* —
six-tier source scoring, EMA latency, cooldowns, temp/permanent bans, the four-way
error classification, the three-pass sweep, the all-banned self-rescue — which is
`rpc_pool.rs`'s 1,975 lines and is exactly what the shell must not reimplement. The
ABI *encoders* web hand-rolled (`encAggregate3`, `encBalanceOf`, Multicall3) do need a
Rust home; `alloy-dyn-abi` is already a `vela-core` dependency.

## Out of Scope

- **Money.** No `send`, no signing, no bundler. Spec 032.
- **030's blocked surfaces.** The contacts add/edit sheet, the context-menu actions
  and the favourite control stay blocked on drawings that do not exist; that boundary
  was recorded in 030's closeout and this cut does not move it.
- **Any change under `rust/` except new pure helpers** the read path needs (ABI
  encoders). No machine changes. No corpus regeneration while 026 is open.

## User Scenarios & Testing

### User Story 1 — The wallet shows the person's own money (Priority: P1)

Somebody signed in on the desktop opens the wallet and sees their real balance across
their real networks, priced, or an honest absence where a chain could not be reached.

**Why this priority**: It is the wrong screen this cut exists to fix, and it is the
first time the desktop reads a chain for the person rather than for a probe.

**Independent Test**: Seed the golden Safe's key set, open the wallet, compare the
total against a direct `eth_getBalance` on Gnosis.

**Acceptance Scenarios**:

1. **Given** a signed-in wallet with a funded address, **When** the home opens,
   **Then** the balance is that address's, from the chain — never a fixture figure.
2. **Given** one chain unreachable, **When** the home renders, **Then** that chain
   says so and the others still show; the total does not silently omit it as zero.
3. **Given** no price source, **When** an asset renders, **Then** the amount shows
   and the fiat figure degrades — `rate: null` is not `1`, as in 030.

---

### User Story 2 — Routing is one decision, made once (Priority: P1)

Every chain read in the client goes through `rpc_pool`, so a banned endpoint is
banned for everybody and the fastest one is raced once.

**Why this priority**: It is the architectural point of the cut. Two callers with
their own endpoint lists is how the Expo client got a ban map that disagreed with
itself.

**Independent Test**: Point the pool at a dead endpoint plus a live one; assert the
dead one is banned once and every later caller skips it.

**Acceptance Scenarios**:

1. **Given** several read machines, **When** they call a chain, **Then** every call
   is routed by one shared `rpc_pool` session — not one per screen.
2. **Given** an endpoint that fails, **When** it is banned, **Then** a later call
   from a *different* machine does not retry it.
3. **Given** every endpoint for a chain banned, **When** a call arrives, **Then** the
   core's self-rescue runs rather than the chain being permanently dead.

---

### User Story 3 — Activity, receive and tokens (Priority: P2)

The activity feed, the receive watcher and the token list read real state.

**Acceptance Scenarios**:

1. **Given** a funded address, **When** activity loads, **Then** it lists real
   transfers grouped by day.
2. **Given** the receive screen open, **When** a deposit lands, **Then** it is
   noticed without a manual refresh.
3. **Given** a custom token added, **When** the list renders, **Then** its metadata
   came from the chain and survives a relaunch.

## Requirements

- **FR-001**: Every rule stays in the core. The shell fetches, decodes and caches; it
  never decides which endpoint, when to ban, or whether a result is complete.
- **FR-002 (One pool)**: A single module-level `rpc_pool` session serves every
  caller. The ban map, endpoint stats and race winners are facts about the network
  that all callers share.
- **FR-003**: Every operation is answered exactly once, with the core's own failure
  variants. The 030 rule, unchanged.
- **FR-004 (Fixtures stay canon)**: No `fixtures.rs` loses or alters a constant;
  galleries render unchanged.
- **FR-005 (Storage bytes)**: `vela.balanceCache`, `vela.customTokens`,
  `vela.transactionHistory` and the rest keep their cross-client key and field names.
- **FR-006 (Ported, not reinvented)**: Service code ported from web carries a
  provenance header naming the source file and commit. Where `vela-core` already has
  the primitive — ABI decode, hex codecs, Safe derivation — it is used, not
  re-implemented.
- **FR-007**: The 030 fail-closed arms marked `// live in 031` are flipped live, and
  the table of them is the handoff contract.
- **FR-008 (Waiting is not fixtures)**: Before the core has ruled, a neutral surface.
- **FR-009**: No machine changes under `rust/`; no corpus regeneration while 026 is
  open.

## Success Criteria

- **SC-001**: The signed-in home shows the golden Safe's real Gnosis balance, matching
  a direct `eth_getBalance` taken independently.
- **SC-002**: One `rpc_pool` session serves every machine; a ban set by one caller is
  observed by another. Proven by test, not asserted.
- **SC-003**: An unreachable chain renders as unreachable, not as zero.
- **SC-004**: The four `// live in 031` arms from 030 are live, and 030's
  `read_device_currency` returns a real region currency.
- **SC-005**: Galleries unchanged; every `fixtures.rs` diff additive.
- **SC-006**: `cargo test` count strictly increases; fmt and the desktop CI job green.
- **SC-007**: Zero corpus delta; no machine file under `rust/` changed.
