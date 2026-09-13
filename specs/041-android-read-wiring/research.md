# Research — Android Read Wiring (041)

Decisions taken against this worktree, with the sibling implementations named
where this is a port rather than a design.

---

## D1 — The transport library

Android already talks HTTP two ways: `HttpURLConnection` in `RegistryClient`
(whose header explains the choice: "six calls, not a client library"), and
**okhttp**, already a dependency for the caBLE WebSocket tunnel.

The pool is not six calls. It is every balance read, every log query and every
probe, across a dozen chains and several endpoints each, with per-call timeouts
and a fastest-endpoint race that issues concurrent requests on purpose.

**Decision: okhttp for the pool.** No new dependency, connection reuse across
the many calls to the same host, per-call timeouts without hand-rolled threads,
and cancellation that actually cancels. `RegistryClient` keeps
`HttpURLConnection`; there is no reason to churn working code.

**Not negotiable either way**: the pool is the *only* component that issues a
chain request. FR-101 and SC-107 exist because "just one fetch here" is how the
routing rules get quietly bypassed.

---

## D2 — Where the pool lives

The ban map, the per-endpoint statistics and the fastest-endpoint winners are
**facts about the network that every caller shares**. Two pools would mean two
opinions about whether an endpoint is banned, and the second one would keep
hammering a host the first had given up on.

**Decision**: one process-wide pool in the composition root, created lazily
like 040's controllers, running on `Dispatchers.IO`. The web sibling reached
the same conclusion for the same reason (`services/rpc-pool.ts`: "ONE
module-level session … the ban map, per-endpoint stats and fastest-RPC winners
are facts about the network every caller shares").

`Dispatchers.IO` rather than the `Main.immediate` 040's storage controllers
use: these calls block on sockets, and a pool sharing the main dispatcher would
make a slow endpoint a dropped frame.

---

## D3 — How a machine asks for a chain read

Every read-path machine needs to make RPC calls, and none of them may choose an
endpoint. The web solves this with a facade whose header states the split
exactly: *"This file owns exactly two things the core cannot have: the fetch,
and the promise the caller is waiting on."*

**Decision**: port that shape.

```text
BalanceExecutor ──rpcCall(chain, method, params)──► RpcPool (facade)
                                                      │  dispatch to RpcPoolCore
                                                      │  ◄── json_rpc_post ──► okhttp
                                                      │  ◄── conclude ──────────
                ◄────────── the answer ───────────────┘
```

The facade suspends the caller until the core concludes the call. The `call_id`
in every operation and result is what makes a late answer droppable by
construction — the core designed for exactly this.

**Consequence for the executors**: a read-path executor never sees a URL. It
sees `rpcCall(chainId, "eth_getBalance", params)`. That is what keeps
FR-101 true by construction rather than by discipline.

---

## D4 — Where the endpoint list comes from

`rpc_pool` asks the shell to `load_pool_config(chain_id)` and expects seeded
endpoints back. The web has `services/rpc-pool-endpoints.ts` (222 lines)
collecting them from the built-in chain table, the person's custom networks,
their per-network overrides and their provider keys — all of which Android
already reads, because **040 wired `network_admin`**.

**Decision**: the seed comes from the `network_admin` machine's own view rather
than from a second reader of the same storage keys. 040 built that reader; a
second one would be a second opinion about which endpoints exist.

---

## D5 — Numeric types, again, and worse

040 shipped a bug because ts-rs writes `u32` and `f64` both as `number`, and
serde rejects a float where a `u32` is expected. That was one field on one
machine. This feature carries chain ids, block numbers, timestamps, latencies,
token decimals and balances.

**Decision**: every numeric field in a 041 wire type is transcribed **from the
Rust struct**, and the drift test's existing blind-spot note is extended to say
so. Where the Rust is `u64` and the value can exceed 2^53, the wire is checked
for how the core actually serialises it before a Kotlin type is chosen.

---

## D6 — Two dormant defects inherited from the desktop sibling

Spec 031 found two bugs that are **invisible while prices are `None`** — and
this feature is the one that makes prices exist. Both are recorded here so they
are looked for rather than rediscovered:

1. **`BalanceToken.balance` is a human decimal, not a raw unit.** The core
   multiplies it by a price directly. A shell that hands over raw units
   produces a total 10^18 times too large, and nothing shows it until a price
   arrives.
2. **Tempo's RPC returns the same constant for every address** (it has no
   native coin — gas is a TIP-20 stablecoin), and its native symbol is `USD`,
   so a stablecoin peg would price that constant at $1 and put ~4×10^57 dollars
   into the total. The rule that excludes it is the core's own
   `fee_policy::TEMPO_CHAIN_IDS` — **never a "that looks too big" threshold
   invented in the shell**.

---

## D7 — The bridge, measured before the work

The gate this program runs at every slice. 040's data: 3 machines = 3,715,432
stripped bytes; 6 = 4,786,088; 24 = 7,890,696 — a marginal 172 KB per machine
once the shared serde and crux code is paid for.

041 adds seven, so the expectation is ≈ +1.2 MB, taking the app to ≈ 6 MB of
`.so` per ABI. **Measured, not assumed** — the number is recorded in
`results.md` before the Kotlin lands, and the whole-program figure (7.89 MB)
remains the ceiling this cannot exceed.

---

## D8 — What this feature must not become

Three things sit temptingly close and are out (spec Assumptions):

- **Sending money.** `send`, `sign_request`, `fee_policy` and the guards are
  042. The receive flow is in scope because watching for a deposit is a read.
- **The camera.** Flow state `S1` scans a QR code. That is a new capability
  with its own permission story, not a wire.
- **The explore tab and dApp surfaces.** 042, and on this base they have no
  navigation destination at all.


---

## D9 — Where the ABI codec goes (decided during phase 4c)

Pricing a coin needs Multicall3 batching, ERC-20 reads, Uniswap-V3 and Solidly
quotes and Chainlink decoding — 342 lines of hand-written hex framing in the
web's `services/abi.ts`. Android needs the same. Three options: port it to
Kotlin, promote it into `vela-core` and export it over uniffi, or put it in the
uniffi crate alone.

**Decision**: port the framing to Kotlin; leave the rules in Rust.

The codebase already drew this line, and it holds up. `best_group_price`,
`best_native_dex_price` and `choose_native_price` live in
`vela_core::app::balance_dashboard` *because they are judgement about money* —
which pool wins, which stable wins, whether a quote is trustworthy. They are
already exported to both wasm and uniffi, so Android calls the same rules the
web does. What stayed in each shell is transport framing: 32-byte words and
offsets, which cannot be wrong in an interesting way without a test noticing.

Promoting the framing was the tempting alternative and was rejected on blast
radius, not on principle: it changes the crate every shell links, mid-programme,
while a colleague works 02x/03x on the same tree — and it buys nothing the
vectors do not already buy. It stays available as a later consolidation.

**The cost, stated plainly**: a second hand-written copy with no generated
artifact to drift-test against, unlike the wire types. Mitigations: every
encoding and decoding is pinned by hand in `AbiTest` (18 cases, including
malformed remote input), and the whole path runs against real chains on a
device.

**The exception that proves the split.** `firstGroupedQuotePrice` — first usable
quote across groups, each scaled by its own stable's decimals — is judgement,
not framing. Its docstring is entirely about a 10^12 mispricing, and it has **no
Rust owner**: it exists only in the web's TypeScript. It is deliberately not
ported. Android does not price custom ERC-20s until that rule is in the core,
which is the first task of phase 4d.
