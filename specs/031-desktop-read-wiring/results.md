# Results — 031 desktop-read-wiring

## Baselines — recorded 2026-09-04, branch point `6324ba39` (tip of `030-desktop-live-shell`)

Stacked on 030 for the same reason 030 stacked on 029: both cuts edit
`wallet/page.rs` heavily, and 031's gates are the ones 029 added.

### Desktop at the branch point

| | |
|---|---|
| `cargo test` | **125 passed · 0 failed · 8 ignored** |
| `src/**/*.rs` | 61 files |
| `wallet/page.rs` | 4,255 lines ⚠ |
| warnings (forced rebuild) | 1 (`BLE_CHANNEL_SUPPORTED`, pre-existing) |
| gallery states | 36 |

### The seven machines

| Machine | Core lines | Operations |
|---|---|---|
| `rpc_pool` | 1,975 | 7 |
| `token_trust` | 1,937 | 6 |
| `activity_feed` | 1,117 | 6 |
| `balance_dashboard` | 1,096 | 7 |
| `payment_request` | 687 | 2 |
| `manage_tokens` | 657 | 4 |
| `receive_watch` | 377 | 3 |
| **total** | **7,846** | **35** |

### The service layer this cut must port — and the part it must not

`BalanceOperation::FetchTokens { address, force, pull }` names **no chain and no
URL**. The core delegates the whole multi-chain fetch to the shell and rules only on
what comes back, so 031 ports a service layer as well as writing executors. That is a
shape 030 did not have, and it is why this cut is materially larger.

| Web source (at `f9bcb278`) | Lines |
|---|---|
| `services/wallet-api.ts` | 788 |
| `services/abi.ts` | 342 |
| `services/rpc-pool.ts` | 334 |
| `services/chains.ts` | 231 |
| `services/recipient-identity.ts` | 223 |
| `services/rpc-pool-endpoints.ts` | 222 |
| `services/activity.ts` | 203 |
| `services/token-metadata.ts` | 159 |
| `services/price-service.ts` | 130 |
| `services/native-price.ts` | 100 |
| `services/balance-cache.ts`, `endpoint-admission.ts`, `incoming-transfers.ts`, `tokens.ts`, `currency-rate.ts`, `fiat-rate-quote.ts` | 343 |
| **total** | **~3,075** |

**Already in Rust, and not to be ported:** calldata decoding and selector maths
(`vela-core/abi.rs`, 539), hex/quantity codecs and keccak (`primitives.rs`, 202), Safe
derivation (`safe.rs`, 760) — and every routing *rule*, which is `rpc_pool.rs`'s 1,975
lines. The shell must not reimplement six-tier scoring, EMA latency, cooldowns, bans,
error classification, the three-pass sweep or the all-banned self-rescue.

**Genuinely missing and needed:** the ABI *encoders* web hand-rolled — `encAggregate3`
/ `decAggregate3` (Multicall3), `encBalanceOf`, `encDecimals`, `encGetEthBalance`.
`alloy-dyn-abi` is already a `vela-core` dependency, so these are assembly rather than
new machinery.

### Owed from 030 (FR-007's handoff contract)

| Arm | Answers today | Must become |
|---|---|---|
| `contacts::resolve_identity` | `identity: None` | the identity waterfall |
| `contacts::classify_recipient` | `code: None` | `eth_getCode` via the pool |
| `display_currency::resolve_rate` | `rate: None` | the rate source chain |
| `display_currency::read_device_currency` | `None` | the region's ISO-4217 |
| `network_admin::invalidate_pools` | acknowledged no-op | a real pool invalidation |

### A debt inherited from 030 that shapes this cut

`rpc.gnosischain.com` refuses this HTTP client with **403** while curl gets 200 —
measured through a bare `ureq::Agent` and through `proxy::agent`, against all three
Gnosis endpoints. It is `registry.rs:362`'s **first** endpoint and it is Gnosis's
default in the chains table. The pool's scoring and ban rules will meet it
immediately, which makes it a useful first real test of exactly the behaviour this
cut is wiring — and a reason not to assume any single endpoint answers.

## Phase 1 — the routing authority

`src/executor/pool.rs`: the `rpc_pool` machine on its own thread, with a blocking
call API, six-tier endpoint collection, and bans that persist.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **128 passed · 0 failed · 9 ignored** (031 opened at 125) |
| `cargo fmt --all --check` | ✅ clean |
| warnings (forced) | ✅ 1, pre-existing |
| live read through the pool | ✅ see below |

### Why the pool is a thread, not a resident

Every other machine lives in `resident.rs`, driven from the main thread and rendered
by a screen. The pool has neither property: its callers are **background workers doing
blocking HTTP** — the balance fetch, the activity read, a recipient probe — and they
need an answer on the thread they are already on. Routing them through the main thread
would put a multi-second round trip in front of the next frame, which is precisely
what the resident host exists to avoid.

So the pool owns a thread, and callers block on a reply channel. One `OnceLock`, and
no way to make a second session — the ban map, per-endpoint statistics and race
winners are facts about the network **every** caller shares. Two sessions means an
endpoint banned by the balance fetch and retried by the activity read a second later.

### The live read, and why it is the right test

```
golden Safe: 0.76997 xDAI via the pool
second read agreed — one session, shared state
```

That figure matches an independent `eth_getBalance` taken by hand. And chain 100's
**built-in default endpoint is `rpc.gnosischain.com`** — the one that answers this
client with 403. So the pool could only produce that number by scoring it, failing
over and reaching a different endpoint. A single-endpoint client cannot read this
balance at all, which makes the inherited 403 debt the most useful possible first test
of exactly what this phase wires.

### What this file owns, and what it must never

Two things the core cannot have: **the fetch**, and **the reply channel the caller is
waiting on**. Everything about *where a call goes next* — six-tier source scoring, EMA
latency, cooldowns, temp and permanent bans, four-way error classification, the
three-pass sweep, the all-banned self-rescue — is `rpc_pool.rs`'s 1,975 lines. If this
file grows an `if` that decides where a call goes, it is in the wrong file.

Two details the core's comments insisted on and this file obeys:
- **Bans are not filtered during collection.** "Do NOT filter banned URLs — bans are
  this core's state." The shell offers every endpoint; the core decides which is dead.
- **The body never enters the core.** A `PostOutcome` reports only whether there was
  an `error` member; the shell holds the JSON per `(call_id, url)` and hands over the
  one the verdict names. A 3 MB `eth_getLogs` answer stays out of the machine's state.

### A tier deliberately not implemented, recorded rather than forgotten

Tiers 5 and 6 are the chain index's endpoint list. `LoadPoolConfig` is answered
synchronously on the pool thread, and an index round trip there would stall every
first call on a chain behind an HTTP fetch. The core orders whatever it is given, so
adding those tiers later changes no rule — it is a debt, not a divergence.

## Phase 2 — the wallet reads its own money

`executor/balances.rs` (the multi-chain fetch the core delegates whole) and
`executor/balance_dashboard.rs` (seven operations, the 24h total cache, the privacy
flag).

| Gate | Result |
|---|---|
| `cargo test` | ✅ **133 passed · 0 failed · 10 ignored** (031 opened at 125) |
| `cargo fmt --all --check` | ✅ clean |
| warnings (forced) | ✅ 1, pre-existing |
| `scripts/sweep-gallery.sh` | ✅ every state rendered |

### The live read, across every chain

```
11 chains answered, 1 did not
  chain 100 : 769970000000000000 xDAI
  unreachable: [4217]
```

`769970000000000000` wei is **0.76997 xDAI** — the golden Safe's known balance, now
reached through the pool's routing rather than a hand-written endpoint list.

**The second line is the one that matters.** Tempo comes back in `failed_chain_ids`,
not as a zero-balance token, and a test asserts the two lists are disjoint. A wallet
that renders an unreachable chain as empty **under-reports somebody's money and looks
completely normal doing it** — which is SC-003, and the reason the core takes the two
lists separately rather than a single token array.

### Scope of this cut, stated rather than implied

**Native coins only.** ERC-20 needs Multicall3 aggregation and a token list; prices
need a source. Both are additive: the core already accepts a `Vec<BalanceToken>` and
already knows what an unpriced one means. `price_usd` is `None`, never `0` and never
`1` — the same discipline `display_currency` made explicit in 030.

Two more deliberate gaps, marked in the code rather than left to be discovered:
- **`force` is accepted and ignored**, because this cut keeps no 5-minute shell TTL —
  every fetch is live. Adding the TTL later changes no core rule, since the core
  already says when it wants one bypassed.
- **No streaming.** The core supports `ChainAssetsArrived` so a home fills in as
  chains answer; that needs a way to push events into a resident from a worker, which
  this cut does not build. Twelve chains run in parallel and settle once — correct,
  just less alive.

### The split the cache respects

The core's words: *"The shell applies the 24h TTL"* and *"The CORE decides when this
may happen — the complete-results-only write gate."* Both halves are obeyed exactly.
Expiry reads as **absent**, not as a stale figure — the hero would rather show a
skeleton than yesterday's number presented as today's — and this file never writes
uninvited, because caching a total assembled from a partial fetch is how a wallet
remembers a number that was never true.

### A red that was my test, not the code

`only_unexpired_rows_reach_the_switcher` failed with 0 rows where 1 was expected. The
cause was the test seeding a fixed past timestamp while the operation reads the
**real** clock — so both rows were legitimately expired and the executor was right.
Fixed by seeding from the same clock. Worth recording because the failure looked
exactly like a broken TTL.

## Phase 3 — the hero shows the person's own money

`wallet/live.rs` and the hero bound to `BalanceDashboard`.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **138 passed · 0 failed · 10 ignored** (031 opened at 125) |
| `cargo fmt --all --check` | ✅ clean |
| warnings (forced) | ✅ 1, pre-existing |
| `scripts/sweep-gallery.sh` | ✅ every state rendered |

### No fallback to the fixture, and that is the point

`balance_model` returns the mock only when there is **no session**. For a real one it
returns whatever the core says — including a skeleton while the count is in flight.
Falling back to `$1,383.28` there would be the app showing somebody a stranger's money
and calling it theirs.

The three states the tests pin are the ones the core went to trouble over:
- **unknown → skeleton, never `$0`** (invariant ②). A wallet that shows zero while it
  is still counting has told the person their money is gone. A test asserts the
  rendered integer contains no digit at all.
- **a real zero is not unknown.** Different state, drawn differently, on purpose.
- **hidden withholds by construction** (invariant ⑧). The core already nulls the
  total; the test asserts the shell does not reintroduce a figure.

### What a 50-second run of the real app produced

```
[vela-wallet] core: balance_dashboard booting
keys written: vela.accounts, vela.activeAccountIndex, vela.rpc.banned
balanceCache: ABSENT
BANNED: https://rpc.gnosischain.com  (temporary)
```

Both of those lines are the system working, and neither is obvious:

1. **The pool banned `rpc.gnosischain.com` by itself.** That is the endpoint 030
   phase 2 found refusing this HTTP client with 403 while curl gets 200 — recorded
   then as a debt with no fix. It now needs none: the pool met it, classified it,
   banned it, routed around it, and wrote the ban down so the next launch does not
   spend a request rediscovering it. The debt closed itself the moment the machinery
   that owns the decision was wired.
2. **No balance cache was written, and that is correct.** Tempo was unreachable, so
   the result was partial, and the core's complete-results-only write gate (invariant
   ⑥) refused to ask for the write. A cached total assembled from eleven of twelve
   chains is a number that was never true. Distinguishing "the core refused" from
   "the fetch had not finished" needed a 50-second run rather than a 25-second one —
   the 25s run looked identical and would have supported the wrong conclusion.

## Phase 4 — the handoff contract, mostly closed

Four of the five arms 030 marked `// live in 031` are live.

| Arm | 030 | now |
|---|---|---|
| `contacts::classify_recipient` | `code: None` | `eth_getCode` through the pool |
| `display_currency::read_device_currency` | `None` | the region's ISO-4217 |
| `display_currency::resolve_rate` | `None` | the configured fiat endpoint |
| `network_admin::invalidate_pools` | acknowledged no-op | `pool::refresh` |
| `contacts::resolve_identity` | `None` | **still owed** — the waterfall needs the passkey index and name services |

| Gate | Result |
|---|---|
| `cargo test` | ✅ **137 passed · 0 failed · 13 ignored** |
| `cargo fmt --all --check` | ✅ clean · gallery ✅ · warnings 1, pre-existing |
| live, per module | ✅ pool 1 · balances 1 · display_currency 2 · contacts 1 · network_admin 3 |

Live evidence: `USD → CNY = 6.71907`, `USD → JPY = 156.014`; the golden Safe classifies
as **171 bytes of code** and `0x000…001` as **`0x`** — a verdict, and a different
answer from `None`.

**Two currency arms were one debt, and that is why they landed together.** A desktop
always had a region (`Loc::from_env`); what it lacked was a *rate*, and the core
persists a seeded currency only after a real rate resolves. A region candidate is
useless until something can price it.

### Three tests changed with the arms, which is the contract working

`a_chosen_currency_comes_back_unpriced_rather_than_invented` asserted `rate: None`.
That was correct in 030 and is wrong now, so it became
`a_chosen_currency_comes_back_priced` — the visible half of a fail-closed arm going
live. Likewise `the_unavailable_lookups_answer_unknown_rather_than_a_verdict` split:
history stays honestly empty (local), classification became a live test asserting a
contract and a non-contract answer **differently**, because conflating `0x` with
`None` is how a wallet calls somebody's own address a contract.

### Three process failures of mine, all the same shape

1. **A `str.replace` that matched nothing** because `cargo fmt` had reflowed the
   target — the same trap as 030 phase 6.
2. **A script that asserted *before* writing and aborted between the two**, leaving
   the file untouched while its log said "flipped". I then debugged a stale file.
3. Fixed by **verifying after the write**, not only asserting before it. Every edit
   since re-reads the file and asserts the new text is on disk.

### A test that failed because the world changed

`the_golden_safe_reads_across_chains` pinned `769970000000000000` wei. It went red at
`758970000000000000` — the Safe's balance **moved on-chain**, 0.011 xDAI spent by
something outside this session. A wallet balance is not a constant, and a test that
fails when the world changes is reporting the wrong thing. It now asserts what must
hold: Gnosis answered, the quantity is non-zero, the symbol and decimals are right.
The exact figures in this document stay as they are — point-in-time evidence, not
pins.

### Live tests run per module, and the reason is a real property

Run as one `executor::` set they interfere; per module they are green. The cause is
structural rather than accidental: **the pool is a process-wide singleton thread** and
`with_temp_state` swaps a process-wide `VELA_STATE_DIR` underneath it. Both facts are
correct on their own — one pool per process is the architecture, and per-test isolation
is how storage tests work — and they simply cannot share a process. These are
`#[ignore]`d manual gates, so per-module is the documented way to run them:

```
cargo test executor::pool -- --ignored --test-threads=1
```

## Phase 5 — the activity feed, and the day boundary

`executor/activity_feed.rs` (six operations) plus `executor::day_start_ms`.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **143 passed · 0 failed · 13 ignored** (031 opened at 125) |
| `cargo fmt --all --check` | ✅ clean · gallery ✅ · warnings 1, pre-existing |
| `check-windows.sh` | ✅ the `cfg(not(unix))` path type-checks |

### The day boundary is the shell's, and getting it wrong is visible

The core's words: *"LOCAL-midnight epoch ms — computed by the shell, which owns the
device timezone."* `vela-core` deliberately ships **no timezone database**, so this is
the one fact it cannot derive.

Grouping by UTC day files a 20:00 transaction in Tokyo under **tomorrow**, and a 19:00
one in New York under **today** when it belongs to yesterday. That is wrong for part of
every day for everybody outside Greenwich — not a rounding error, a heading with the
wrong transactions under it.

So the offset comes from `localtime_r`, and `libc` moved from a Linux-only dependency
to a `cfg(unix)` one. `localtime_r` rather than a value read once at startup, because
the offset must include **daylight saving as of this instant** — a cached offset is
wrong twice a year.

**Windows has no `localtime_r`** and `GetTimeZoneInformation` is not wired, so it
returns 0 and groups by UTC day. Recorded as a debt with its consequence spelled out
rather than left as a silent `#[cfg]`. `check-windows.sh` type-checks that path.

The test asserts the property rather than a figure, which is what makes it runnable on
a machine whose timezone it does not know: two instants an hour apart share a
boundary, two a day apart do not.

### Three answers that are reports, not decisions

- **A legacy row reports `kind: None`.** The core reads absent as `send` by its own
  rule (`t.type ?? 'send'`). Substituting `Send` in the shell would hide a legacy row
  from the core's own rule about legacy rows.
- **Deleting a missing record answers `DeleteFailed`.** A delete that removed nothing
  is a failure, not a quiet success: the row is still on the person's screen and the
  core has to know it is still there.
- **A haptic is answered on a machine with no haptics.** Skipped would leave the core
  waiting; the celebration simply runs with one fewer sense.

### Own accounts resolve locally, and case does not matter

`ResolveRecipientIdentity` checks the person's own accounts first — on disk, no
network — and compares lowercased. A wallet that misses its own account on casing
labels it a stranger. The ENS/name-service half is the same waterfall
`contacts::resolve_identity` still owes; they should land together rather than be
written twice.

### `ScanIncomingTransfers` answers zero, marked `// live in 032`

Receipt discovery is `getLogs` over the transfer allowlist plus `token_trust`
admission, and the records it persists are the same store 032's send path writes.
Zero new records is a true statement about a scan that found none — the feed simply
has nothing to celebrate yet.

## Phase 6 — the feed reaches the screen

`wallet/live.rs::activity_rows` and the home's activity list bound to `ActivityFeed`.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **148 passed · 0 failed · 13 ignored** (031 opened at 125) |
| `cargo fmt --all --check` | ✅ clean · gallery ✅ · warnings 1, pre-existing |
| `*fixtures.rs` deleted lines in 031 | ✅ **0** |

### The bug this phase nearly shipped

`FeedItem` carries both `value` and `decimals`, which reads like raw-integer-plus-scale
— the same shape `BalanceToken` uses, where `balance` **is** raw. It is not. The web
renders it with `trimBalance(item.value)` and the core sums it with `parseFloat`,
neither of which scales: **`value` is the human amount already**.

Scaling by `decimals` would have printed every activity figure 10¹⁸ times too large,
and it would have looked deliberate — two fields that plainly belong together, used
together. A test now pins it: `1.5` renders as `+1.5`.

### Three render decisions the core deliberately does not make

- **Headers are dropped here, not filtered out of the core.** `FeedView::rows`
  interleaves day headers with items because the full Activity screen draws them; the
  home preview is a flat short list. Asking the core for a different shape would move
  a render decision into the machine.
- **The kind comes from the record, not the item.** `FeedItem` has only a direction;
  `FeedView::transactions` carries `kind`. Looking it up keeps the dApp distinction the
  mocks draw — a swap is not "sent", and labelling it so loses the one word that
  explains where the money went.
- **Privacy is read from the BALANCE view**, not the feed's own flag. Every money
  surface masks together; reading two flags is how one ends up out of step. The figure
  goes, the unit stays — H5's rule, with a test asserting the number cannot survive.

### The badge tint is the settings table

Read through `settings::model::chain_tint` — the same table the network rows use, which
is the same table the mocks use. A second colour map for the same chains is how one
screen's Polygon stops matching another's.

## Phase 7 — three more machines

`receive_watch`, `payment_request` and `manage_tokens`.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **156 passed · 0 failed · 14 ignored** (031 opened at 125) |
| `cargo fmt --all --check` | ✅ clean · gallery ✅ · warnings 1, pre-existing |
| live ERC-20 read | ✅ `USDC / USD//C on xDai / 6 decimals` |

### Six of seven machines are wired

`rpc_pool`, `balance_dashboard`, `activity_feed`, `receive_watch`,
`payment_request`, `manage_tokens`. Only `token_trust` remains.

### `MulticallErc20Meta` is three calls, and the name is the core's word for a want

The operation is named for what the web does — one `aggregate3` against
Multicall3. This asks `symbol()`, `name()` and `decimals()` separately: three
round trips, no Multicall3 encoding, the same answer. The name describes *what
the core wants*, not how; folding them into one call later changes nothing it sees.

**The failure rule is not deferred.** Metadata is all-or-nothing: a token with a
symbol and no decimals renders an amount at the wrong magnitude, and once saved it
stays wrong for as long as it is in the list. `None` unless all three answered.

The ABI decoders are hand-written and tested against real return data, including the
shapes that must produce `None` rather than a panic or a garbage symbol: a truncated
word, a non-hex body, an offset with no length behind it. A mojibake symbol saved into
somebody's token list is forever.

### Two "failed" answers that could easily have been quiet successes

- **Removing a token that is not there** answers `RemoveFailed`. The row is still on
  the screen.
- **Saving the same token twice replaces it** rather than appending. Adding the same
  contract again is a person correcting themselves, not two tokens.

### A float that is safe because of what it is *for*

`receive_watch`'s `TokenSnapshot.balance` is an `f64`, which would be wrong for money.
It is safe here because the number is only ever compared against an earlier snapshot
of itself to answer "did it go up" — never rendered, never summed. The comment says so
at the conversion, because the next person to read it will reasonably wonder.

### The pay-link base is the web wallet, deliberately

`payment_request`'s `base_url` points at `getvela.app`, not a desktop URL scheme. A
pay link is for somebody else to open, and a scheme most people cannot follow is a
link that does not work.

## Phase 8 — the tokens, the price, and a chain that lies

`executor/abi.rs`, `executor/chain_tokens.rs`, `executor/chainlink.rs` and a
rewritten `executor/balances.rs`.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **173 passed · 0 failed · 18 ignored** (031 opened at 125) |
| `cargo fmt --all --check` | ✅ clean · gallery ✅ · warnings 1, pre-existing |
| `check-windows.sh` | ✅ |
| live, per module | ✅ pool 1 · balances 2 · chain_tokens 1 · chainlink 1 · manage_tokens 1 · display_currency 2 · contacts 1 · network_admin 3 · wallet::live 1 |
| `*fixtures.rs` deleted lines in 031 | ✅ **0** · no file under `rust/` changed |

### SC-001, end to end

```
hero: $0.76  state=Normal notice=None
```

That is the golden Safe's real xDAI, priced, rendered by the real hero model
after the real machine settled over the real network. It is a test, not a
screenshot — `wallet::live::the_hero_shows_the_golden_safes_own_money` drives
`CoreHost<BalanceDashboard>`, performs every operation it asks for, and asserts
the figure is neither the `$1,383.28` fixture nor a skeleton.

### The bug that was invisible until money was

`BalanceToken.balance` is the **human decimal amount**, not base units. Phase 2
wrote raw wei into it under a comment claiming the opposite ("the core takes the
raw integer as a string and owns every decimal decision"). The core does not:
`token_usd_value` is `token_balance_double(&balance) * price_usd`, and
`token_balance_double` is `parseFloat` — it parses a fraction and scales nothing.

Nothing showed, because every `price_usd` was `None` and anything times zero is
zero. The moment this phase produced a price, the same code would have reported
the golden Safe's 0.76 xDAI as **769,970,000,000,000,000 dollars**. The two
changes that made money visible are the two that made this dangerous, which is
the general shape worth remembering: a dormant unit bug is not a small bug, it
is a bug with a fuse.

`abi::format_raw_balance` is now the only path from base units to that field,
and its test asserts the round trip through the core's own `token_balance_double`.

### Tempo answers the same "balance" for every address on earth

```
chain 4217 : 4242424242424242424242424242424242424242424242424242424242.42… USD @ $1.0000
```

Measured, three addresses, one answer:

```
eth_getBalance 0x88cCA0…6894 → 0x9612084f0316e0ebd5182f398e5195a51b5ca47667d4c9b26c9b26c9b26c9b2
eth_getBalance 0x0000…0001   → 0x9612084f…  (identical)
eth_getBalance 0x1111…1111   → 0x9612084f…  (identical)
```

`rpc.mainnet.tempo.xyz` returns a **constant**, not a balance — 4.24 × 10^75 base
units — because Tempo has no native coin at all: its gas is a TIP-20 stablecoin
(`fee_policy::TEMPO_DEFAULT_FEE_TOKEN`). And because its coin is *called* `USD`,
`chainlink::resolve`'s stable-gas peg prices it at exactly $1.00, so the junk
arrives **fully valued** rather than unpriced. One chain would have put 4 × 10^57
dollars into somebody's total.

The fix is a fact, not a threshold: `has_native_coin` reads
`fee_policy::TEMPO_CHAIN_IDS` — the core's own record of which chains settle in
a stablecoin — and a chain with no native coin gets no native row. It is still
**read**, so its reachability verdict is unchanged; it simply has nothing native
to show. Inventing a "too large to be real" cutoff was the other option and is
worse: it would be a number this file made up, and it would let the next such
chain through at 10^30.

### Two calls per chain, because two failures mean different things

The web reads the native coin through Multicall3's own `getEthBalance`, inside
the batch. This reads it with `eth_getBalance` and uses the batch only for
ERC-20s and quotes:

- `eth_getBalance` failing means **the chain could not be reached** — the verdict
  `failed_chain_ids` carries to the home (SC-003).
- the batch failing means Multicall3 is not deployed here, or a quoter reverted.
  The chain is fine and the coin is still theirs.

Folded together, a chain without Multicall3 reports as unreachable, which is a
true-sounding lie about somebody's money. `enc_get_eth_balance` was written, then
deleted, because a ported function with no caller is a claim that we use it.

### The one rule this cut refused to write

The web prices a **custom** ERC-20 by DEX quote through
`firstGroupedQuotePrice`. That rule has no `vela-core` home, and FR-009 forbids
this cut from adding one to a machine file. Writing it in the shell instead
would put a money rule in the shell, which is FR-001.

Both requirements point the same way: a custom token comes back with its real
balance and `price_usd: None`, and the core's `Unpriced` notice says so out
loud. The native coin's rules **are** in the core (`best_native_dex_price`,
`choose_native_price`), which is exactly why its price is here and this one is
not. `pick_quote_token` was ported, then deleted, because its only job is
ordering the attempts of the rule that did not land.

**Owed:** `first_grouped_quote_price` in `balance_dashboard.rs`, then eight lines
of shell to call it.

### Where each price comes from, and who decided

| Slot | Price | Decided by |
|---|---|---|
| native, wrapped | DEX → local Chainlink → mainnet Chainlink | **core** (`choose_native_price`) |
| stablecoin | $1.00 | **shell**, deliberately — see below |
| custom ERC-20 | `None` | nobody, and the notice says so |

The $1.00 is not a missing factor defaulting to one. `Stable` is a **membership
verdict** — the token came from this chain's curated stablecoin list — and ≈$1 is
the definition of that membership. The web owns it the same way and at length
(`wallet-api.ts:498-527`), including why a de-peg gate was considered and
rejected: the only independent measurement available is the same DEX quote whose
near-empty pools the price ladder already has to defend against, and a wrong
de-peg verdict silently drops a holding out of somebody's total.

### A dead feed, measured and kept

`0x14e613AC…5d25` is BNB/USD in the ported Chainlink table. `eth_getCode` at that
address on Ethereum mainnet answers `0x` — **there is no contract there**, so BNB
never appears in the mainnet price map on either client.

It is kept verbatim (FR-006) with the measurement written beside it, because BNB
is not left unpriced by it: chain 56's own feed answers (`$725.49`, measured) and
that is the rung *above* this one. What is lost is the fallback, on the day BSC's
local feed also fails. Deleting the line would hide that; a comment does not.

### Two decodes that refuse rather than default

- **A `decimals()` that did not answer drops the row.** The web defaults to 18.
  On a 6-decimal stablecoin that prints a balance a trillion times too small,
  and it reads as an answer rather than as a gap.
- **A `balanceOf` that did not answer drops the row.** Reporting it as zero is a
  holding quietly deleted.

Both are the same rule the native path already had: absent and zero are different
answers, and only one of them may be drawn as a number.

### u256 is a decimal string

Every quantity word decodes to decimal digits rather than an integer type. A
token with 18 decimals and a large supply passes `u128::MAX`, and the two ways an
integer type copes — saturating or refusing — are both a wrong balance. The core
takes these as strings anyway (`NativeQuoteGroup::amounts_out`), so the string is
the honest shape. A test pins 2^255 decoding exactly.

### Why the encoders are not in `vela-core`

031's spec points at `vela-core`, where `alloy-dyn-abi` already lives. They are
in the desktop shell instead, for two measured reasons:

1. `rust/pkg-web` is a **committed build product** and CI's `build-web.mjs
   --check` rebuilds and compares it. A module in `vela-core` means regenerating
   a cross-client artifact mid-031 for code only the desktop calls.
2. Android and iOS cannot use a `vela-core` module unless it is exported through
   uniffi — a bindings and binary-size change that 033 is already required to
   probe **before** it signs its plan.

So it lives where its only caller lives, and promoting it is 033's move, made
with 033's size measurement in hand. The header says so, so the next person does
not read it as an oversight.

### One test of mine was wrong, and the code was right

`a_hex_quantity_decodes_without_being_a_full_word` failed on
`0xaaf7d19cc1a0000 → 769971611055554560` where I had written
`770000000000000000`. I had typed the hex by hand from the decimal. The decoder
was correct; the expectation was a guess. It is the same shape as phase 2's red
— a test that asserts what I assumed rather than what is true — and the cheap
defence is to derive the expectation with a tool rather than by eye.


## Phase 9 — the last fail-closed arm, and one lookup instead of two

`executor/identity.rs`, `registry::query_by_wallet_ref`, and both machines'
identity arms.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **177 passed · 0 failed · 19 ignored** |
| `cargo fmt --all --check` | ✅ clean · gallery ✅ · `check-windows.sh` ✅ · warnings 1, pre-existing |
| `// live in 031` markers remaining | ✅ **0** |

### FR-007's handoff contract is closed

| Arm | 030 | now |
|---|---|---|
| `contacts::classify_recipient` | `code: None` | `eth_getCode` (phase 4) |
| `display_currency::read_device_currency` | `None` | the region's ISO-4217 (phase 4) |
| `display_currency::resolve_rate` | `None` | the fiat endpoint (phase 4) |
| `network_admin::invalidate_pools` | no-op | `pool::refresh` (phase 4) |
| `contacts::resolve_identity` | `None` | **the waterfall** |

Live: `0xd8dA6BF2…6045` → `vitalik.eth` via ENS; the golden Safe → `None`.

### Two machines asked the same question, so now there is one function

`contacts::ResolveIdentity` and `activity_feed::ResolveRecipientIdentity` are
the same question about the same address. Phase 5 had written the local half
twice and said so; this phase merged them into `executor::identity::resolve`,
and `own_account_name` exists once. The feed's test moved with the code rather
than being deleted or duplicated.

### Only positives are cached, and that is not an optimisation detail

The core says so (invariant ⑦) and the reason is visible from the outside: a
name registered a minute after somebody looked would be invisible for a whole
day if the miss were remembered. A miss costs one lookup. A cached miss costs
the truth. The live test asserts both directions — the name comes back, and the
nameless address is *not* in the cache afterwards.

### Asked in order, not raced

The web fires all five name services in parallel and takes the first match **by
priority, not by arrival**. This asks them in order, which is the same answer by
a slower route. Five sequential lookups against an address nobody has named is
the worst case, and it is also the rare one: a stranger's address is asked once
and then not asked again for a day.

## Phase 10 — the seventh machine, and one ledger instead of two

`executor/token_trust.rs`, `executor/custom_tokens.rs`, and `abi::dec_string`.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **185 passed · 0 failed · 21 ignored** |
| `cargo fmt --all --check` | ✅ clean · gallery ✅ · `check-windows.sh` ✅ · warnings 1, pre-existing |
| live | ✅ `USDC / 6` through one `aggregate3`; block 48,072,486 and its header time; `eth_getLogs` → `Ok` |

### All seven machines are wired

`rpc_pool`, `balance_dashboard`, `activity_feed`, `receive_watch`,
`payment_request`, `manage_tokens`, `token_trust`.

### The range cap is the pool's word, not a string match

`eth_getLogs` fails two ways that must not be confused: the endpoint is broken,
or the endpoint is fine and the span was too wide. Only the second is worth
retrying narrower, and only the first is worth failing over — the next endpoint
usually has the same cap, and banning a healthy endpoint over it is how a pool
loses its best RPC.

`rpc_pool` already parses the wording into `PoolError::RangeCap { max_span }`.
This file maps that **one** error onto `RangeCapped` and everything else onto
`Failed`, and never reads an error message.

### Two writers, one file, so one place that knows how it is spelled

`executor/custom_tokens.rs` now owns `vela.customTokens`: the on-disk record,
the replace-by-id rule, and the `networkName` the core deliberately does not
carry (`TrustCustomToken`'s own comment: "chain naming is display vocabulary the
shell derives from `chain_id`"). `manage_tokens` writes it, `token_trust` writes
it, `balances` reads it. Before this phase the shape was private to
`manage_tokens` and the second writer would have had to guess at it.

### A decoder that was refusing real tokens

`abi::dec_string` replaced `manage_tokens`'s private copy and gained the
**bytes32** fallback the web has and we lacked. A legacy ERC-20 — MKR and its
generation — answers `symbol()` with one fixed 32-byte word rather than the
`[offset][length][data]` triple. The old decoder required 64 bytes and returned
`None`, so adding one of those tokens by hand failed with no reason given.

Also ported with it: a declared length that does not fit the payload is read as
bytes32 rather than trusted, and non-UTF-8 bytes produce `None` rather than a
replacement character. A mojibake symbol saved into somebody's token list is
there forever.

### What is not wired, said plainly

Three of `token_trust`'s inputs are **events**, not operations, and nothing
dispatches them yet: `HeldChainsSnapshot` with real chains, `HeldTokensSnapshot`
and `RegistryTokensSnapshot`. Unfed, the core degrades exactly as it documents:
an empty held-chains list polls `DEFAULT_MONITOR_CHAINS`, and a cold registry
means the trusted set is the customs plus the native sentinels — "everything
unverified, the safe direction". Fewer contracts trusted, never more.

The dispatch site is screen work and it **must not live in a render pass**.
Measured, in gpui at rev `c97b7c0`: `App::record_entities_accessed` registers
every entity a window *read while drawing*, and `App::notify` invalidates the
window through that registration. So reads are tracked automatically — which is
why nothing in this client calls `cx.observe` and the live surfaces still
repaint — and equally why dispatching into another resident from inside a draw
re-enters. (I had this backwards first and was about to file "no screen observes
any resident" as a defect. Reading gpui's source cost ten minutes and the claim
was false.)

## Phase 11 — the closeout

`unreachable_chips` and SC-002's two proofs.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **187 passed · 0 failed · 22 ignored** (031 opened at **125**) |
| `cargo fmt --all --check` | ✅ clean |
| `scripts/sweep-gallery.sh` | ✅ 36 states |
| `scripts/check-windows.sh` | ✅ |
| warnings (forced rebuild) | ✅ 1 (`BLE_CHANNEL_SUPPORTED`, pre-existing) |
| `git diff 6324ba39..HEAD -- '*fixtures.rs'` | ✅ **empty** |
| `git diff 6324ba39..HEAD -- rust/` | ✅ **empty** |

### The verdict table

| | Criterion | Verdict | Evidence |
|---|---|---|---|
| SC-001 | the home shows the golden Safe's real Gnosis balance, matching an independent `eth_getBalance` | ✅ **met** | hero renders `$0.76`; app read `0.75897 xDAI`; hand-taken `eth_getBalance` = `0xa8867319d2da000` = `758970000000000000` wei = **0.75897**. `wallet::live::the_hero_shows_the_golden_safes_own_money` |
| SC-002 | one pool session serves every machine; a ban set by one caller is observed by another — **by test** | ✅ **met** | `one_session_serves_every_caller` (pointer identity on the `OnceLock`) and `a_ban_one_machine_earns_is_the_ban_the_next_machine_meets` — the balance read banned `1rpc.io/gnosis` and `rpc.gnosischain.com`; both were still known when the price service read |
| SC-003 | an unreachable chain renders as unreachable, not as zero | ✅ **met** | data: `failed_chain_ids` disjoint from `tokens`, asserted live. render: `unreachable_chips` drives the banner from `banner_chain_ids` (failed **minus** rate-limited), with a test for the empty, the two-chain and the custom-network cases |
| SC-004 | the `// live in 031` arms are live and `read_device_currency` returns a real currency | ✅ **met** | all five arms; zero `live in 031` markers remain; `USD → CNY = 6.71907` |
| SC-005 | galleries unchanged; every `fixtures.rs` diff additive | ✅ **met** | the `fixtures.rs` diff is **empty**, not merely additive; 36 gallery states render |
| SC-006 | `cargo test` strictly increases; fmt and the desktop CI job green | ✅ **met** | 125 → **187**; fmt clean; gallery and `check-windows.sh` green |
| SC-007 | zero corpus delta; no machine file under `rust/` changed | ✅ **met** | the `rust/` diff is empty |

### What 031 did NOT do, and why each one is a decision rather than a gap

| Owed | Why it is not here |
|---|---|
| custom ERC-20 **prices** | the rule (`firstGroupedQuotePrice`) has no `vela-core` home and FR-009 forbids this cut from adding one; writing it in the shell is FR-001. Both requirements point the same way |
| `token_trust`'s **poll dispatch** | a screen dispatch site that must not sit in a render pass, and it needs a visible run to verify |
| the **receive** and **assets** screens | still the mocks. The receive flow has a 2026-08-15 Penpot redesign the desktop has never drawn; implementing it is a screen build, not read wiring, and 030's precedent is that surfaces without a drawn desktop design stay blocked |
| **streaming** partial balances | `Event::ChainAssetsArrived` needs a worker→resident event push. Twelve chains settle once instead: correct, just less alive |
| the **5-minute token TTL** | `force` is accepted and ignored because every fetch is live. The core already says when it wants a TTL bypassed, so adding one later changes no rule |
| chain-index endpoint **tiers 5 and 6** | `LoadPoolConfig` is answered synchronously on the pool thread; an index round trip there stalls every first call on a chain |
| Windows **day boundaries** | `GetTimeZoneInformation` is not wired, so Windows groups the feed by UTC day. Recorded with its consequence rather than hidden in a `#[cfg]` |

### The two defects this cut found, and what they have in common

Both were **dormant**: correct-looking code that produced no visible error until
something else was switched on.

1. `BalanceToken.balance` held raw base units under a comment claiming the core
   wanted them. Invisible while every price was `None` — anything times zero is
   zero — and a **10^18×** total the moment a price arrived.
2. Tempo's RPC answers the same 4.24 × 10^75 constant to `eth_getBalance` for
   every address on earth, and its coin is called `USD`, so the stable-gas peg
   priced the junk at exactly $1.00.

Neither was findable by looking at the code that contained it. Both were found
by *turning the next thing on and reading the output* — a live test that prints
what it read, rather than one that asserts what I expected. That is the practice
worth carrying into 032, where the same class of bug moves money instead of
displaying it.

---

# The second half — 031 finishes the screens

Phase 11 called 031 closed on its seven success criteria. The founder read the
closeout and said: *if something is missing, add it — we want the functionality
complete.* This is that.

## A correction to phase 11's closeout, and what it cost

Phase 11's "what 031 did NOT do" table said the receive flow "has a 2026-08-15
Penpot redesign the desktop has never drawn". **That was wrong.** `src/flows/`
holds **19 drawn panels** — DR1/DR2/DR3 receive, DT1/DT4 assets, DA1/DA2/DA3
activity, DT3 add-token, DSD1–4 send. They were never undrawn. They were
unbound.

I asserted it from a memory note rather than from `ls src/flows/`, and the cost
was real: it turned nine phases of available work into a deferral. The check
that would have caught it took eleven seconds.

## Phase 12 — assets, activity and receive read the cores

`flows/live.rs`, the same fixtures/live split `wallet`, `settings` and
`contacts` already have.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **194 passed** (from 187) |
| live | ✅ the panel lists `0.75897 xDAI Gnosis $0.76`, and its sum equals the hero's total to the cent |

Two rules carried from the hero, because they are the same rules: privacy masks
the figure and keeps the unit, and the guided-empty body waits for the core to
rule — telling somebody their wallet is empty while it is still being counted is
the assets-panel version of the fake `$0`.

An unpriced holding says **"no price"**, never `$0.00`. `$0.00` reads as
worthless; the core keeps `unpriced_tokens` for exactly this distinction.

DR1L's rows bind one listener each, so stepping into "Gnosis" shows Gnosis. The
fixture keeps its single first-row listener — every mock row opens the same
picture, and twelve identical closures to say so would be noise.

## Phase 13 — the QR was not a QR

`flows::components::qr_card` drew a deterministic decorative pattern. Its own
comment said **"Never encodes data"** — and the live receive screen drew it too.
A person pointed a phone at their own wallet and got nothing, or worse, believed
they had.

`qrcode` 0.14 was **already a dependency** (onboarding's caBLE FIDO:/ code uses
it). The fix was to use it. No payload keeps the designed pattern, which is why
the gallery is unchanged: a mock has no address to encode and the drawing is
what it is meant to show.

**What the code says is `payment_request`'s decision**, not the shell's:
`qr_value` is the bare recipient in address mode and the EIP-681 URI in request
mode. Encoding the address here would work today and silently drop an amount the
moment the request builder lands.

Opening the QR panel boots `receive_watch` — reading the resident IS the start
event — so US3's "a deposit lands and is noticed without a manual refresh" is
live. Arrivals render as `ReceiveScreen.tsx`'s `depositBox` draws them, and
`detected` gates the section rather than a non-empty list: they are the core's
two separate answers and only the first means announce.

## Phase 14 — incoming payments reach the feed

`ScanIncomingTransfers` was answering zero. It now runs the whole
`syncReceivedTransfers` pipeline.

`token_trust` became a **session on a thread** — `pool.rs`'s shape, not
`resident.rs`'s — because its callers are background workers. One session
matters for a specific reason: the trusted-contract allowlist is assembled from
three inputs that arrive separately (held chains, held tokens, the registry's
stablecoins). A second session starts with none of them and degrades to "customs
plus the native sentinels" — safe, and blind to a plain USDC payment.

The balance fetch feeds those three, because **that fetch IS the observation**.

A wave of pending operations runs in parallel: six chains' block numbers have
nothing to say to each other, and serially a poll is a minute of waiting for a
screen whose job is to notice money arriving. Measured: one chain in 1.3s.

```
4 incoming in the scan window
  49750000 USDC from 0xb45373129b4220160b92bd2320869f44d48ecd01
   4975000 USDC from 0xb45373129b4220160b92bd2320869f44d48ecd01
   5812428 USDC from 0x082738d007001080a00099a000004f3006152085
   5812428 USDC from 0xe3fff29d4dc930ebb787fecd49ee5963dadf60b6
```

That test points at a busy Curve pool on Gnosis, not at us. It **cannot** assert
a non-empty result — whether anything landed in the last hundred blocks is a
stranger's business — so it asserts what must hold about whatever it finds, and
prints the count as the evidence a person reads.

Two ingest rules: a non-native token whose metadata would not resolve is
**skipped**, not stored at a guessed 18 decimals (which on a 6-decimal token
stores a misleading "+0 tokens"); and **no `usd` is written at all**, because the
core re-derives it on read including the stablecoin fallback. Writing `"$0.00"`
would store a claim over a rule.

## Phase 15 — the rule goes where rules go

Phase 8 left custom ERC-20s unpriced because `firstGroupedQuotePrice` had no
`vela-core` home and both ways out broke a requirement. With 026 merged and the
functionality asked for, the rule went into `balance_dashboard.rs`.

`first_grouped_quote_price` is deliberately **not** `best_native_dex_price`. The
native path takes the maximum because every group prices the same coin and the
deepest pool is least distorted. A custom token's groups are tried in a stated
order — preferred stablecoin first — so the first pool that answers is the one
the caller asked for, and a maximum would silently promote whichever stable
quoted highest.

Each group is scaled by **its own** quote token's decimals. That is the whole
point: on a chain holding both a 6-decimal (USDC) and an 18-decimal (DAI) entry,
a token with no USDC pool but a live DAI pool was priced 10^12 times too high.

Live: **GNO added by hand on Gnosis prices at $118.67** through a real SushiSwap
V3 quote.

`rust/pkg-web` was regenerated, because a `vela-core` source change changes it
and CI rebuilds and compares. Same wasm size (3,630,664 bytes), new content hash.
Re-verified: **46,408 conformance cases** green through the shipped artifact, 25
onboarding wire types current, `vela-core`'s own tests green.

## Phase 16 — a transaction opens its own detail

Row listeners bind per row, each carrying its record's id. `history_ids` walks
the feed exactly as `panels::history` draws it — two walks that could disagree
would open the **wrong transaction**, which on a money screen is worse than
opening nothing.

A pending or failed transfer does not wear the confirmed chip
(`componentsTx.detail.statusPending/statusFailed` — the same keys the RN
`TxStatusBadge` reads, so three clients say one word about one state). A
`usd_value` of 0 means unknown, not free, so the fiat line is empty.

## Phase 17 — the home stops showing a stranger's assets

The wallet home drew its asset strip from `fixtures::assets_default` and its
network list from `fixtures::chains` — **for a signed-in person too**. The hero
said `$0.76` and the six rows beneath it said BNB 0.8533, ETH 0.2253, USDT
53.4836. Under somebody's real name and address.

That is the exact screen spec 031's *Why* names, and it survived twelve phases
because the hero was the part everyone looked at, including me. The lesson is
narrow and worth keeping: **a screen is not wired until every list on it is**.

`chain_rows` lists only chains with something on them — twelve networks where
eleven say "0" is a list nobody reads — and the "all" row's count is the number
of chains listed, so the two halves cannot disagree.

## Phase 18 — the asset panel is about the asset you opened

Clicking a holding opened D3 hard-coded to BNB whichever row was clicked.
`asset_detail` is now an index into the core's sorted holdings, set by the row
that was clicked.

Neither this panel nor the transaction detail falls back to the mock when its
subject disappears. A refresh re-ordering the holdings under an open panel would
otherwise silently swap which asset somebody is looking at — **and the next thing
they do on that panel is send it**.

## Phase 19 — a token can actually be added

DT3L had a read-only well showing a hard-coded USDT contract above a card for a
token nobody had looked up.

`ui::text_field` already existed, so the work was threading `&Window` into the
panel render and putting the value where it belongs: `MtokView::input_address`
is the **only** copy, because the core validates the address and clears the found
cards on every keystroke.

The desktop drawing has no search button — the found card simply appears — so
the search fires as soon as the core says the address is one. WHEN to ask is the
shell's; whether the ask may **run** is still the core's.

The card says which of three things is true, never nothing: not searched,
searching, or searched and there is nothing there.

## Where the desktop stands

| Surface | State |
|---|---|
| balance hero, asset strip, network list | ✅ live |
| activity preview + full panel + tx detail | ✅ live |
| asset detail (D3) | ✅ live |
| receive: network list, QR, deposit watch | ✅ live |
| add token (DT3) | ✅ live |
| settings: networks, localization, currency | ✅ live (030/031) |
| contacts: roster, delete, identity | ✅ live (030/031) |
| **send (DSD1–4)** | ⛔ spec 032 |
| **scanner (DS1)** | ⛔ no camera pipeline on desktop at all |
| contacts add/edit/groups/favourite | ⛔ blocked on drawings (030's boundary) |

Everything a signed-in person can reach on the read path now reads their own
wallet. The remaining mocks are reachable only from the gallery, or lead into
032's money path.

## Final gates

| Gate | Result |
|---|---|
| `cargo test` | ✅ **205 passed · 0 failed · 26 ignored** (031 opened at **125**) |
| `cargo fmt --all --check` | ✅ clean |
| `scripts/sweep-gallery.sh` | ✅ 36 states, unchanged |
| `scripts/check-windows.sh` | ✅ |
| warnings (forced rebuild) | ✅ 1, pre-existing |
| `build-web.mjs --check` | ✅ current |
| `verify-web.mjs` | ✅ 46,408 conformance cases |
| `gen-onboarding-types.mjs --check` | ✅ 25 types current |
| `git diff 6324ba39..HEAD -- '*fixtures.rs'` | additive only — **no line removed** |

## Phases 20–23 — 030's third list, finished

030's closeout kept three lists: what was **wired**, what was **blocked on
drawings**, and a third headed *"Also not wired, and merely unfinished rather
than blocked"* — the wizard's search field, the RPC override field, and the
endpoints / providers panels. That list sat untouched through all of 031's first
nineteen phases, including its closeout. Four surfaces, all the same shape: the
core was complete, the refusal was proven, and there was no box to type in.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **209 passed · 0 failed · 26 ignored** (031 opened at **125**) |
| `cargo fmt --all --check` | ✅ clean · gallery ✅ 36 states · `check-windows.sh` ✅ |
| warnings (forced rebuild) | ✅ 1, pre-existing |
| live | ✅ `network_admin` 3 |

### One field, four surfaces

`settings::components::editable_url_field` wears `url_field`'s clothes so a live
panel and a mock one look identical. In every case the value lives in the
**core** — a shell-side copy would be a second opinion about what was typed —
and a keystroke dispatches Edited then Blurred, because the core's blur is what
PERSISTS and a keystroke that never blurred is a setting the next launch has
never heard of.

### "We have not asked yet" is not a verdict

The rule the balance hero established for a figure it does not have, applied
four more times:

- a **service endpoint** still being probed gets **no pill**, not a grey one;
- a **provider key** still being tested gets **no support line** — "0 of 0"
  printed mid-test reads as "this key works nowhere";
- an **override probe** still running draws no latency badge;
- a **chain the wizard could not reach** gets a **RETRY**, never four red
  crosses. That is the core's invariant ③ and the whole reason `rpc_failure` is
  a separate field from `compatible`: *"this chain does not work"* and *"we could
  not ask"* are different sentences, and only one is fair to a chain nobody
  managed to reach.

A P-256 probe that never ran leaves its row **out** rather than drawing it as a
failure — same rule, one level down.

### Three states that had to be told apart in words

| State | What the badge says | Why not the neighbouring one |
|---|---|---|
| not HTTPS | error, "HTTPS required" | a refusal to trust, not a slow answer |
| HTTP 502 | error, `HTTP 502` | a different problem from "offline", with a different fix |
| reachable, wrong service | **warning** | the core does not gate saves on it, so it must not look like a refusal |

### The one refusal in the app, finally worded

The RPC override is the only save this app can decline: the core probes the
endpoint and, if it answers `eth_chainId` with **another chain's id**, writes
nothing — a "Gnosis" endpoint that actually serves Polygon would route somebody's
money to the wrong chain. 030 proved that refusal against a real endpoint and
left the field read-only, so the refusal had never been *seen*.

It now replaces the save hint, carrying both chain ids. A person who watched
nothing happen has to be told why, and "saved" would be a lie.

### The banner's chip finally leads somewhere

Since phase 11 the unreachable-chain banner has been telling the truth and
pointing at a dialog that could not act on it. Each chip now opens **its own**
chain's editor — one "fix" button that always opened the first would send
somebody to repair a network that is working — and the dialog reuses the same
field the network card opens, because two editors for one override is two places
a refusal has to be worded, and they would drift.

No save button on the rescue dialog: the field persists on its own, and a button
that only sometimes saves is worse than no button.

### What the wizard would not let the shell re-decide

The "Add network" CTA renders only when `can_add` says so. That flag is the
core's whole judgement — resolved, checked, compatible, not already added — and
re-deriving any part of it in the shell would be a second opinion about whether
a chain is safe to add. Picking a suggestion **drops** a custom RPC typed for a
different chain (checking chain A against chain B's endpoint is meaningless); a
recheck **keeps** it, because a typed endpoint is usually the reason to recheck.

### The remaining boundary, re-verified rather than inherited

030 named three contacts surfaces as blocked on drawings that do not exist: the
context-menu actions, an add/edit sheet, and a favourite control. Two of those
are still true. The first — *"the context menus are pictures"* — is the same
shape as the per-row listeners this cut added twice, and is **wire-able**; it is
recorded as available work rather than as a blocker.


## Phases 24–29 — the fixture sweep, and what it kept finding

After phases 20–23 I asked what was left and got a clean-looking answer: four
groups of machines, all gated on the send path, a browser engine, or a drawing.
Then I ran one grep — **every `fixtures::` use, checked for an identity guard
above it** — and it found six more live surfaces drawing mocks. Two of them were
not merely wrong; they were dangerous.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **214 passed · 0 failed · 26 ignored** (031 opened at **125**) |
| `cargo fmt --all --check` | ✅ clean · gallery ✅ 36 states · `check-windows.sh` ✅ |
| warnings (forced rebuild) | ✅ 1, pre-existing |
| `// live in 032` markers | 2, both `ClearBundlerCache`, both genuinely waiting |

### A picture with a live weapon attached

The contact detail panel drew a **fixture** while its delete and copy acted on
the **real** contact. Somebody clicking their cousin saw Alice's name, Alice's
avatar and Alice's address — and the delete button removed the cousin.

That is worse than a mock. A mock is honestly a picture. This had been a picture
with a live weapon attached since 030 wired the delete, and it survived 030's
closeout, 031's closeout, and my own "everything reachable is live" claim in
phase 19.

### A stale marker, and the test that should have caught it

`contacts::LoadSendHistory` answered an empty list behind `// live in 032 —
there is no local transaction store yet`. That stopped being true in **phase 14
of this very cut**, when receipt discovery started writing to
`vela.transactionHistory`.

Its guard test was worse than useless: it asserted `txs.is_empty()` **without
`with_temp_state`**, so it read whatever state directory the process pointed at
and kept passing after a store appeared. A test that cannot tell an empty store
from an unread one cannot notice the arm it was watching go live.

### Copy did not copy

`cx.write_to_clipboard` has been available all along and **nothing in the app
used it**. The receive panel's "copy address" button, the receive QR's address
card and the contact detail's address block were all decorative. A receive
screen's entire job is to hand an address over, and until 031 both of its ways
of doing that — the code and the button — were pictures.

### Four more from the same grep

| Surface | Was | Now |
|---|---|---|
| contacts group rail | 家人 / 工作 / 朋友 fixtures | the person's groups, each with its id |
| group view (DC4) | `GROUPS[i]`'s members | that group's members, or nothing |
| 全部联系人 count | the mock's 12 | the real book's size |
| settings accounts (DST1) | real row 0 + **two strangers** | every account, switchable |
| storage (DST7) | 2.4 MB / 216 records, to everybody | this device's measured file |
| about (DST8) | **v1.0.0 (6ab8f)** while the crate was 0.1.1 | `CARGO_PKG_VERSION` |
| add-network dialog subtitle | "Zora · 链 ID 7777777" over an unresolved wizard | the chain it actually resolved, or nothing |

### `SwitchAccount` finally has a control

`session.rs` wrote, about this very event, that *"an event with no control is
dead code"*. The control — the account row — was drawn all along. It carries the
**core's** index rather than the loop's, which is invariant ⑦'s whole point: a
display reorder must not switch to the wrong wallet.

### The lesson, stated as a procedure

Three separate times this cut, a surface was wrong because somebody (twice, me)
believed a claim instead of checking it: *"the desktop never drew receive"*,
*"there is no local transaction store yet"*, *"the core exposes no account list
yet"*. All three were true when written and false when read.

The grep that finds this class costs ten seconds:

```bash
# every fixture use, and whether an identity guard stands above it
grep -n 'fixtures::' src/wallet/page.rs
```

**A comment explaining why something is not wired is a claim with a timestamp on
it.** When the reason is "X does not exist yet", the marker outlives the reason,
and nothing in the type system will tell you.


## Phases 30–33 — the contacts section closes

030 listed three contacts surfaces as **blocked on drawn UI that does not
exist**: the context-menu actions, an add/edit form sheet, and a favourite
control. Two of those were still true when 031 re-checked them in phase 23. By
phase 33 only one is.

What changed was not the drawings. It was that by then the app had **four
dialogs, an editable text field and a file picker**, and every word these
surfaces need was already in the corpus. What 030 correctly called a design
decision in 2026-08 had become composition.

| Gate | Result |
|---|---|
| `cargo test` | ✅ **222 passed · 0 failed · 26 ignored** (031 opened at **125**) |
| `cargo fmt --all --check` | ✅ clean · gallery ✅ 36 states · `check-windows.sh` ✅ |
| warnings (forced rebuild) | ✅ 1, pre-existing |
| unguarded `fixtures::` in `page.rs` | ✅ **0** outside the gallery, the mock paths and `explore` |

### An address book can leave this machine and come back

`prompt_for_paths` / `prompt_for_new_path` have been in gpui all along, so the
"needs a file picker" half of the blocked list was one call away.

`executor/contact_io.rs` ports `contact-io.ts`'s serialize and parse halves. The
**format is the point**: a backup written on the phone has to open here, so
`version` / `exportedAt` / `contacts` / `groups` keep their spelling and the CSV
keeps its column order. Inventing a desktop format would make export a feature
that only talks to itself.

The CSV heuristics came with their reason attached, and it is worth repeating
because it is the shape of every good comment in this codebase: a foreign file
rarely spells the column `address`, and when an unrecognised header fell back to
column 0 holding the NAME, every row failed the address test, every row was
dropped silently, and the import reported *"0 added, 0 already existed"*.
Nothing imported, nothing explained, nothing to try differently. **So when the
header does not say where the address is, the data does.**

Two rules that look like sloppiness and are the opposite:

- a malformed row is **carried** to the core, not dropped in the parser. "Is
  this an address" is the core's question and it counts the answer; swallowing
  bad rows here made `invalid` structurally zero on the CSV path.
- a CSV that plainly held contact rows and yielded no address at all is an
  **error**, not an empty parse. A file we cannot read must say so instead of
  succeeding with zero of everything — which from the outside is exactly what an
  empty address book looks like.

### An import that reports nothing is a feature that looks broken

The counts are the core's: it applied existing-wins and is the only thing that
knows how many rows were new. The shell states them in the same four keys the RN
screen alerts with, so three clients say one sentence about one outcome — and an
unreadable file gets the failure dialog rather than "0 added", which is the
distinction `Unreadable` exists to carry, finally visible at the end of the path
that raises it.

### The form, and the one field it will not let you change

The **address is the identity** the core keys on, so an edit keeps it fixed and
shows it read-only. Letting it change would be a delete and an add wearing one
button, with the old contact quietly surviving.

The save button is available exactly when the address is one — the core refuses
a malformed address anyway, and saying so before the press is what keeps the
button from looking available for something it will not do. The field turns red
only once something has been typed: an empty field is a person who has not
started, not one who is wrong.

An empty **name** clears the name, because the core reads `Some("")` that way
and a person who deleted the text meant to.

### One dialog for two questions

新建分组 and 重命名分组 are the same question, and the core takes the same event:
`ContactGroupInput` with `id: None` creates, an existing id renames.
`members: None` on the rename, because the core reads that as "leave membership
alone" and a rename that emptied the group would be a rename in name only.

导入到本组 / 导出本组 stay inert: the whole-book pair is wired, but a per-group
import has no core event behind it, and an item that highlights and does nothing
is worse than one that plainly does not.

### What is still blocked, after re-checking all three

Of 030's three, **one** survives: there is no favourite control anywhere in the
desktop drawings. `ToggleFavorite` is the same dead code `SwitchAccount` was
before phase 26 — an event with no control — and unlike the account row, this
one has no control drawn to wire.

---

# 交接:下一个会话从这里开始

**范围:只做 desktop。** Android / iOS / web 已交给其他同事(创始人 2026-09-05 确认)。

工作区 `/Volumes/data/production/vela-wallet-native`,分支 `031-desktop-read-wiring`
(叠在 `030-desktop-live-shell` 上,后者叠在 `029-native-repair` 上,均未合并)。
工作区干净,本轮共 33 个 phase。

## 一句话状态

**已登录的人能点到的每一个界面都读自己的钱包**,只有 explore(浏览器)例外。
读路径七台机器全接、七条 SC 全达标、设置四个只读框全能填、联系人区全通。
`cargo test` **125 → 222**,`// live in 032` 只剩 2 个(都是 `ClearBundlerCache`)。

## 立刻可跑的闸门

```bash
cd /Volumes/data/production/vela-wallet-native/app-desktop/vela-wallet
cargo fmt --all --check && cargo test && scripts/sweep-gallery.sh && scripts/check-windows.sh
# 真网测试必须【按模块】跑,原因见 Phase 4:
env -u all_proxy -u http_proxy -u https_proxy \
  cargo test executor::pool -- --ignored --test-threads=1
```

基线:**222 passed · 0 failed · 26 ignored**,fmt clean,36 个画廊状态,1 个既有
warning(`BLE_CHANNEL_SUPPORTED`)。

有真网测试的模块:`pool` `balances` `chain_tokens` `chainlink` `identity`
`manage_tokens` `token_trust` `display_currency` `contacts` `network_admin`,
以及端到端的 `wallet::live`(英雄区)和 `flows::live`(资产屏)。

**动过 `rust/` 就还要跑(仓库根)**:
```bash
node rust/scripts/build-web.mjs --check   # pkg-web 是入库产物,CI 会重建比对
node rust/scripts/verify-web.mjs          # 46,408 条一致性用例
node rust/scripts/gen-onboarding-types.mjs --check
```

## 先读这四样

1. **本文件**——尤其是 Phase 24–29 的「fixture 扫描」和它记的那条程序。
2. `src/flows/live.rs` 与 `src/wallet/live.rs` 的模块注释——fixtures/live 分工,
   以及"核心还在数的时候画什么"。
3. `src/executor/{pool,token_trust}.rs` 的模块注释——这两台为什么是**独立线程**
   而不是 gpui resident,以及"一个会话"为什么是要紧事。
4. `src/executor/balances.rs` 的模块注释——每条链为什么发两个请求。

## desktop 还欠的:3 组 + 5 件

### 三组机器(24 台里 desktop 该接的还剩 6 台)

| 组 | 机器 | 核心行数 | 前置 |
|---|---|---|---|
| **A. 花钱** | `send` `tx_tracker` `batch_import`,加 `fee_policy` 真接线 | 9,152 | ⚠️ 见下方"开工前必做" |
| **B. 签名面板** | `clear_signing` `approval_guard` `sign_request` | 9,362 | 要有签名请求来源(A 或 C) |
| **C. dApp 浏览器** | `dapp_session` `dapp_permissions` `browser_history` | 3,771 | ⛔ **桌面没有 web 引擎**(Cargo.toml 里没有 wry/webview/cef)。这是平台决策,不是接线 |

**`ext_cache`(692 行)不算 desktop 的**——它是 Safari 扩展的 App Group 快照 +
Universal Link,桌面没有这个概念。

B 组的图(DCS1–8)画好了,但现在是 `src/signing/fixtures.rs` 里 **33 个手写场景的
画廊**,背后没有请求管线;要驱动它得先有 A 或 C。

### 五件零碎的

| # | 事 | 状态 |
|---|---|---|
| 1 | 收藏控件(`ToggleFavorite`) | 核心有事件,**桌面图里没有星标**。和 phase 26 之前的 `SwitchAccount` 同病,但那个的控件画着,这个没有 |
| 2 | 设置页 新建/登录账户 | 要一条从已登录窗口回 onboarding 的路由(导航决策)。现在**故意不画**,而不是画成死按钮 |
| 3 | 扫码 (DS1) | 桌面没有相机管线。新功能,不是接线 |
| 4 | 余额流式到达(`ChainAssetsArrived`) | 要 worker→resident 的事件推送 |
| 5 | Windows 日界线 | `GetTimeZoneInformation` 没接,现在按 UTC 分组。`check-windows.sh` 只保证那条路能编译 |

外加两个 `// live in 032`,都是 `network_admin::ClearBundlerCache`,真在等 bundler。

## A 组开工前必须先做的一件事

**把固定密钥集签名者用 Rust 写进 `vela-core`(`dev-fixtures` feature),而且要在
A 组的第一个 phase。** 那三把是裸 P-256 私钥,而 desktop 的每条签名路径都通向
**导不进密钥的真实认证器**(USB HID / 平台 passkey),所以金标密钥集在桌面**一条路都
走不通**。放到中段才发现,验收标准就没法满足。`vela-core` 已有全部零件
(`webauthn.rs` / `registry_proof.rs` / p256),约 150 行。

## 每次接手都要跑的一条 grep(十秒)

```bash
grep -n 'fixtures::' src/wallet/page.rs
```

逐条看它上面有没有 `identity.is_none()` 之类的门。本刀里它一次找出**六个**已登录还在
画 mock 的界面,其中最狠的是**联系人详情画着 A 的名字和地址,而删除/复制作用在 B 身上**
——从 030 接上删除那天起就是这样,熬过了 030 收口、031 收口,和我自己"能点到的都接了"
的断言。

> **"为什么还没接"的注释,是一条带时间戳的断言。** 当理由是"X 还不存在",理由会先于
> 注释过期,而类型系统一个字都不会说。

本刀应验四次:「桌面从没画过收款流」(`src/flows/` 有 19 块画好的板)、「还没有本地
交易库」(phase 14 我自己建的)、「核心还不暴露账户列表」(019 就暴露了)、
「没有增改表单是设计缺口」(到 phase 32 已经只是组装)。

## 四条容易踩的坑(我踩过)

1. **`str.replace` 静默不匹配** —— `cargo fmt` 会把目标重排。改完必须**读回并断言**。
2. **真网测试不能同进程一起跑** —— pool 是进程级单例线程,`with_temp_state` 会换掉
   进程级的 `VELA_STATE_DIR`。两者各自都对,不能共处一个进程。
3. **别把链上数字钉进断言**,也**别手算十六进制** —— 两样都栽过。
4. **别凭记忆断言仓库里有什么。** 见上面那条 grep。
