# Results — Android Read Wiring (041)

Written as the work lands. Each phase adds its own verdict; the success
criteria are verdicted at the end.

---

## Phase 0 — The size gate (T101–T103) ✅

Seven machines added to the bridge (`RpcPool`, `BalanceDashboard`,
`ActivityFeed`, `ManageTokens`, `TokenTrust`, `ReceiveWatch`,
`PaymentRequest`), taking Android from 6 of 24 to **13 of 24**.

| Build | machines | stripped arm64-v8a | Δ | per machine |
| --- | --- | --- | --- | --- |
| 019 baseline | 3 | 3,715,432 | — | — |
| spec 040 | 6 | 4,786,088 | +1,070,656 | 356,885 |
| **spec 041** | **13** | **5,919,272** | **+1,133,184** | **161,883** |
| whole program | 24 | 7,890,696 | — | — |

**The estimate held, and slightly better than predicted.** 040 measured a
marginal 172,478 bytes per machine over its last eighteen and this feature's
seven came in at 161,883 — so the extrapolation the handover asked 041 to
budget from was sound, and re-measuring cost one build to confirm it.

Remaining headroom: 1,971,424 bytes to the whole-program ceiling, for the
eleven machines spec 042 and beyond will add (≈179 KB each). Nothing here
approaches a decision point.

**What a person downloads**: the App Bundle ships one ABI, so this feature adds
≈ 1.1 MB to an install.


---

## Phase 1 — The transport and the pool (T104–T111) ✅

Android can read a chain, and it can only do so one way.

```text
  caller ──call(chain, method, params)──►│ registers payload + deferred
                                         │ CallRequested ──► rpc_pool (Rust)
                                         │   ◄── json_rpc_post ──► okhttp
                                         │   ◄── conclude(verdict) ──────────
  caller ◄──────── a body, or a failure ──┘
```

New files: `core/net/VelaHttp.kt` (the one client), `feature/wallet/core/`
`RpcWire.kt`, `RpcPoolExecutor.kt`, `RpcPool.kt`, `NetworkEndpointSource.kt`.
The 040 road is untouched — seven `asBridge()` lines and nothing else in
`core/crux/`, which is what that road was for.

**Not one routing decision is in Kotlin.** The executor posts, probes, draws a
random number, sleeps, and writes a ban list. Which endpoint next, when to ban
it, whether 429 means "broken" or "busy" — `rpc_pool.rs`, unchanged.

### What the tests establish

| Case | What it pins |
| --- | --- |
| healthy endpoint | a routed call returns its body |
| dead endpoint, then a good one | the pool routes around it and the caller never learns |
| HTTP 429 | arrives as **rate-limited**, not as a failure |
| a refused connection | arrives as a failure, **not** as rate-limited |
| HTTP 401 | banned, and the ban is on disk as `{url, bannedAt, permanent}` under `vela.rpc.banned` |
| a remembered ban | the endpoint is not tried again after a restart |

### Three things found by running it

1. **The core bans on 401/403/404, not on unreachability.** My first ban test
   used a refused connection and timed out waiting for a ban that was never
   coming: an endpoint that cannot be reached gets a *cooldown*; only one that
   is up and refusing us gets banned. That distinction is the core's, and the
   test now asserts the half the core actually has.
2. **`NoStrayHttpClientTest` caught a file on its first run** —
   `core/passkey/PasskeyDirectory.kt` fetches a provider icon by URL. Not a
   chain, so it is allow-listed with that reason; the test also checks its own
   allow-list for entries that no longer exist, so the exceptions cannot rot.
3. **A leaked coroutine scope is a flake in somebody else's test.** Six pools
   left running made a spec 040 test time out on a ten-second budget it had
   never approached. Cancelling the pools was not enough — the 040 machine
   tests leaked their scopes too, and the flake came back on the next full run.
   Every machine test now cancels its hosts in `@After`; **three consecutive
   full runs are clean**. The lesson is not "add a teardown": it is that a
   background scope nobody closes fails somewhere other than where it was
   created, which is why it took two attempts to place.

**Gate**: 218 unit tests (210 + 8), 0 failures, twice in a row.

### Open from this phase

- **T108 endpoint admission (FR-104)** is not done. Today the pool is seeded
  only from the core's own network list, so no untrusted URL reaches it — but
  that changes in phase 2 when the chain index and provider tiers arrive, and
  admission lands with them rather than after.


---

## Inbox — a core change coming from another session (2026-09-05)

The web session working on spec 028 flagged commit `6cec4ddf` on
`028-web-port-completion` (not on `origin/main` yet; it arrives when 028
merges). It changes `vela-core`'s `contacts.rs` by **events and view fields
only**, and adds `app/contacts_io.rs` — the desktop's own `contact_io.rs`
lifted into the core, with the rule that a malformed file is **refused before
any write** rather than parsed into an empty success.

**Checked, not assumed: Android is unaffected.**

| Changed | Android's exposure |
| --- | --- |
| `ContactsView` gains `import_failure`, `export` | none — `ignoreUnknownKeys`, and the drift gate's view rule is Kotlin ⊆ mirror |
| `ContactEvent` gains 7 variants | none — the event rule is subset too |
| `ContactOperation`, `ContactShellResult` | **unchanged** — verified in the diff; the three operation lines it touches are new *uses* of the existing `WriteGroups` |

The desktop breaks on the same commit because a Rust struct literal must be
exhaustive. Kotlin has no such failure mode, which is a real difference between
the shells and worth remembering when a native heads-up arrives: **the question
for Android is never "did a view gain a field", it is "did an operation or a
result change shape".**

### What it changes for the Android plan

1. **Debt #6 from spec 040 (contacts import/export) gets smaller and better
   shaped.** It read "needs a file picker; the core supports it". The core now
   owns the parsing *and* the refusal rules, so the Android work becomes: a
   file picker, a dispatch of `import_file { content, filename, into_group,
   now_ms }`, and rendering `import_failure`
   (`malformed_json | no_address_column | empty | unknown_group`). **No CSV
   parser in Kotlin** — which is the outcome FR-113 wants anyway.
2. **Android is not part of the divergence 028 recorded**, because Android has
   no import at all. The trap runs the other way: whoever builds it here must
   go through the core's `import_file` rather than writing a parser, or Android
   would *join* a divergence it is currently outside of.
3. **New group events** — `add_group_members`, `remove_group_member`,
   `set_contact_groups` — are what spec 040's debt #5 (group editing) needs,
   and they are the core's now rather than something to invent.
4. **`send.rs`**: `picked_address` closes the picker and `open()` seeds
   `recipient` from `prefilled_recipient`. View-level, so spec 042 inherits it
   for free.


---

## Phase 4a — The price rules stop being web-only (T120 partial)

`bestNativeDexPrice` and `chooseNativePrice` were reachable from the web
through wasm and **from nowhere else** — so Android and iOS were each one
convenient afternoon away from writing their own price ladder, and two Vela
wallets would have disagreed about what the same holding is worth. The desktop
handover asked for a promotion rather than a third copy; this is it.

Two `#[uniffi::export]` wrappers in `vela-core-uniffi`, and **nothing else in
`rust/` touched** — so `pkg-web`, the ts-rs mirrors and their `--check` gates
are all unaffected, verified by `git status` before rebuilding. The shell keeps
the multicall and the decoding; what crosses the bridge is the judgement.

`NativePriceTest` proves it end to end from Kotlin, on the real core:

| Case | Verdict |
| --- | --- |
| two quotes in one stable | the deeper pool wins |
| no quotes at all | **`null`, not `0`** — a coin nobody could price is unknown, and a screen rendering unknown as zero has told a person their holding is worthless |
| DEX agrees with Chainlink | `dex` |
| DEX disagrees far enough | `chainlink_sanity` — the band is why a thin or manipulated pool cannot show somebody a fortune |
| no DEX quote | `chainlink_local`, then `chainlink_eth` |
| nothing at all | `price = null`, `source = none` |

`BalanceWire.kt` is transcribed and in the drift gate, with two type choices
pinned by a test because both would compile if they were wrong:

- **`balance` is a `String`.** The core parses it as a human decimal and
  multiplies by the price (`balance_dashboard.rs:159`). A numeric field invites
  raw units, which is a total 10^18 times too large **and invisible until a
  price exists** — which is precisely what this feature makes happen. Inherited
  from the desktop's spec 031 findings and now verified in the core's source
  rather than taken on trust.
- **`display_total_usd` and `price_usd` are nullable.** "We do not know" is not
  zero.

**Gate**: 235 unit tests (218 + 6 price + 11 drift/balance), 0 failures.

### What Phase 4 still needs

The balance **pipeline**: per-chain token discovery, a Multicall3 batch per
chain, DEX price quotes in the same batch, the Chainlink fallback, and the
streaming partial results the core expects. That is the ~800-line
`services/wallet-api.ts` port, and it is the single largest piece of this
spec — followed by `manage_tokens`, `token_trust`, the controller, `WalletLive`
and the device check.


---

## Phase 4b — Native holdings, read from chains (T121–T122 partial)

`BalanceExecutor` reads one `eth_getBalance` per chain, concurrently, through
the pool; `WalletController` holds the pool and the dashboard machine together.
Every judgement about money stays in `balance_dashboard.rs` — this half reads
chains and reports what it read.

**Deliberately native coins only.** ERC-20 holdings and prices need a Multicall3
batch with DEX quotes in the same call, and that is 4c. Until then every holding
crosses with `price_usd = null`, so the core reports the total as *unknown*
rather than as a number. A wrong total would be worse than the fixture it
replaces; an honest "not yet" is not.

### A second core rule promoted rather than copied

`is_tempo_chain` was reachable only by a shell that links Rust directly — so
Android and iOS could not ask, which left them one plausible-looking constant
away from the bug the desktop found: Tempo has no native coin, its RPC answers
the **same constant for every address**, and its native symbol is `USD`, so a
stablecoin peg prices that constant at a dollar and puts ~4×10^57 dollars into
somebody's total. It now crosses as `is_chain_without_native_coin`, and the test
proves the chain is **never queried at all**.

Again only `vela-core-uniffi` changed, so no regeneration gate applies.

### What the tests establish

| Case | What it pins |
| --- | --- |
| 1.5 ETH | crosses as `"1.5"` — a **human decimal**, not raw units |
| a chain with no native coin | never asked, by the core's predicate |
| a chain holding nothing | **not** a failed chain |
| a chain that did not answer | named in `failed_chain_ids`, not rendered as zero |
| a zero balance | not a holding |
| an unpriced holding | never becomes a price, and the total stays unknown |

### Two bugs the tests caught, both mine

1. **An empty chain was reported as a broken one.** The first version treated
   "produced no token" as "did not answer", so a chain where a person simply has
   no funds would have raised a network-down banner. `answered` is now tracked
   from the RPC result rather than inferred from the output.
2. **A settle predicate that fired before the fetch started.**
   `!holdings_loading` is true on the initial view, so the assertion ran against
   an empty screen. Only `fetch_settled` carries the failed list, so waiting for
   that is waiting for the fetch to actually finish. Third variation on "the
   view is right before the rest of it arrives" in this program.

**Gate**: 241 unit tests, 0 failures.

---

## Phase 4b/4c — Balances and prices ✅

**$5.02 on the device**, from three real holdings: 0.002 ETH on Arbitrum,
0.152784 POL on Polygon, 0.01 USDT that phase 4b could not see. One
`aggregate3` per chain carries the native balance, every stablecoin's balance
and decimals, the wrapped native token, four DEX quote tiers per stable, and
the chain's own Chainlink feed. Twelve chains cost twelve requests.

The device log shows the core's ladder choosing, which is the point of putting
it there:

| chain | coin | price | source |
| --- | --- | --- | --- |
| 1 | ETH | $2501.19 | `dex` |
| 137 | POL | $0.0973 | `dex` |
| 100 | XDAI | $0.99975 | `chainlink_sanity` — a thin pool disagreed and lost |
| 130 | ETH | $2511.83 | `chainlink_eth` — no local feed, no quote |
| 143 | MON | — | `none` — prices nothing rather than guessing |

### Three defects, all found on the device

1. **A confident `$0.00` over two real holdings.** The core folds an unpriced
   holding in at zero, so a wallet whose every coin is unpriced totals exactly
   `0.0` — indistinguishable at the type level from an empty one. Phase 4b had
   no price source, so that was EVERY wallet. Fixed with a skeleton and a
   warning that says why.
2. **A log line became a failure.** `android.util.Log` is a stub on the JVM and
   throws "not mocked". One line of tracing in the balance executor meant it
   threw instead of answering its operation, and the machine waiting on that
   answer hung until the test timeout — a diagnostic breaking the one shell
   contract that cannot be recovered from.
3. **An asset row named the token twice.** The second line is the chain, and it
   was read off `BalanceToken.name` — correct only while the shell was writing
   chain names into a field that means the token's name. Real names arrived with
   the chain documents and every row read "USDT / USDT".

**Gate**: 274 unit tests, 0 failures.

---

## Phase 5 — Activity ⚠️ (wired; SC-103 not claimed)

The feed is on `activity_feed` and `token_trust`. The fixture's four
transactions are gone; the screen shows this device's own history and an honest
empty state when there is none.

**SC-103 is NOT claimed.** `token_trust` is a LIVE monitor with a 100-block
window — it cannot discover money that arrived last week, and this wallet's did.
The pipeline is proven end to end in `IncomingScanTest`: a real Transfer log
becomes a stored receipt, the same one twice does not, an off-allowlist contract
does not, and a token whose metadata never resolved stays out rather than being
written with a guessed 18 decimals. Confirming it on the device needs a deposit
made while the app is watching, which is SC-104's test too.

### Three defects, two of them device-only

1. **The scan waited for the wrong thing.** `dispatch` is asynchronous, so
   waiting for `!scanning` matched the state from BEFORE the poll was requested.
   It answered instantly with the previous poll's feed; a receipt would have
   surfaced one poll late while the log said it found nothing.
2. **The scan watched the wrong chains.** It asks which chains this person holds
   on, at the same instant the balance read starts — so the answer was always
   "none" and it fell back to the six default chains. Money on Optimism,
   Avalanche, Unichain, Monad or World Chain had no receipt monitoring on the
   first pass. The log said `chains=6` without saying anything was wrong; it now
   says `chains=2`, which is this wallet's truth.
3. **An empty scan was silent** — logged only when something landed, so "found
   nothing" and "never ran" were indistinguishable. The same blind spot the
   balance executor had one phase earlier.

**Gate**: 308 unit tests, 0 failures.

---

## Phase 3 — The display currency ✅

**£3.71 on the device**, converted from $5.02 at Chainlink's GBP/USD feed. The
hero, its label and every asset row move together. GBP had been *selected* since
before this phase and the hero correctly showed dollars, because the core
refuses to convert without a rate.

Two lines here are silently, hugely wrong when they are wrong, and both are
pinned:

- **The inversion.** A `<CCY>/USD` feed answers what one unit costs in dollars —
  0.0068 for JPY. The display rate is the other direction. Returning the feed's
  own number shows a Japanese person a balance 22,000× too small.
- **The decimals.** Most fiat feeds report 8; PHP reports 18. Each feed's own
  `decimals()` is read in the same batch.

The RPC pool moved to the composition root. Its own doc already said it belongs
there — "two pools would mean two opinions about a dead endpoint" — and the
currency rate is the second consumer, which is when that stopped being
theoretical.

**Gate**: 322 unit tests, 0 failures.

---

## Phase 2 — The network arms ✅

Nine operations stop being fail-closed. On the device: **Gnosis · 在线 · 542ms**,
on the one row that was probed, with every other row still blank — the rule 040
wrote and this phase keeps: *a latency nobody measured is not drawn.*

**The probes had no caller.** The network row was drawn tappable from the start
and nothing was listening, so `expandOverride` existed on the controller with no
path to it. The nine arms would have gone live and the screen would still have
shown nothing.

`noRowClaimsALatencyNobodyMeasured` was 040's "no pill, ever". It now pins the
invariant that outlived it: measured shows a number, unmeasured and checking
show nothing.

**Gate**: 323 unit tests, 0 failures.

---

## Phase 7 — Contacts backfills ✅

Three arms live. The address book reads the SAME activity store the feed reads,
so the two can never disagree about whether a payment happened. A name comes
from the passkey index through the client onboarding already publishes to.

**Seconds in, milliseconds out.** The store keeps epoch seconds and the core
wants milliseconds; a missed conversion has nothing to catch it except the test
that now does — every "last paid" would read as 1970 and a person's most recent
recipient would sort to the bottom of their own address book.

The contact detail's 最近往来 block shows this person's transactions, matched on
ADDRESS rather than name. The device bug stays fixed: a contact with no history
shows nothing, not the C2 fixture's "+50 USDC received yesterday".

Deliberately not ported: the ENS-style reverse waterfall (.bnb, .arb,
Basenames) needs a namehash, which needs keccak256. The core already has one for
selectors and should own this rather than carry a third hand-written copy.

**Gate**: 334 unit tests, 0 failures.

---

## Phase 6 — Receive ✅

**The QR code was a decoration.** Xorshift noise with three finder squares,
drawn at full size, encoding nothing. Spec 015 chose that deliberately and said
why: *"a code that looked scannable but was not would be worse than one that
plainly is not."* By this spec the screen around it had become entirely real —
this person's name, address and identicon — so the pattern stopped plainly not
being a code and started looking exactly like one. The failure that reasoning
guarded against arrived from the other side.

It draws the real matrix now, from the encoder already on the bridge (the one
caBLE and the desktop use). Decoded from a device screenshot with `zbarimg`:

```
QR-Code:0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141
```

— exactly the address printed on the card.

**`focused()` and `backgrounded()` had no caller.** The balance machine has had
a focus-driven auto-refresh since phase 4 and nothing was telling it; the
receive watcher needs the same signal or it polls a dozen chains from inside a
bag. Counted rather than a boolean, because a rotation stops one activity and
starts another.

The receive screen is the one place in this app where a stale value is
unrecoverable — every other leaked fixture shows wrong information, a fixture
ADDRESS sends money to a stranger. The address is replaced unconditionally, and
an empty session renders a blank card rather than the drawn one.

**Gate**: 347 unit tests, 0 failures.

---

## Phase 8 — Add a network ⚠️ (search live; SC-106 not claimed)

Typing "celo" returns Celo Mainnet (42220), Alfajores (44787), Baklava (62320)
and Sepolia (11142220) from the public index. Picking one runs the compatibility
checks against that chain's own RPC — eight contracts, all found.

**The search box was drawn read-only.** `value = ""`, no `onValueChange` — the
component has taken one all along. The fixture's three results sat under a
search nobody could perform, and two of them were invented chains.

**A chain id past u32 was killing the whole index.** The public index carries
7,078,815,900; the core holds `chain_id` as `u32`, so serde rejected the ENTIRE
`search_index` result. The symptom was not "that chain is missing" but "network
search does nothing", with a core fault in the log and an empty screen.

> **For every platform, not just this one.** The web guards this no better
> (`Number.isFinite` only) and iOS met the same class in spec 050. Whether
> `chain_id` should widen in the core is recorded here, not answered.

### The "reset" was the wrong event, and probably a success

Chased down after the fact: the row tap was wired to
`AddByChainIdRequested`, which the core calls the **auto** path — the scan
route for a QR code or deep link. On a compatible verdict it does

```rust
model.wizard = Wizard::default();   // ← the "reset" seen on the device
return save_custom_network(model, record);
```

— it saves the network and clears the wizard in one move, with **no confirm
step**. So the screen emptying itself was almost certainly the add succeeding,
and the network list would have shown Celo Alfajores. The device came off USB
before that could be confirmed, so it stays unclaimed rather than asserted.

It was still the wrong event for a person browsing search results: it skips the
verdict, the contract checks and the button the design draws, all of which had
just been wired. The row tap is `ChainSelected` now, which ends at the core's
`checked` phase with a verdict and lets the person press the button themselves.
`addNetworkByChainId` stays for the scan path it was built for, and its doc now
says so.

**SC-106 is NOT claimed** — not because the wizard is broken, but because the
last device run could not be completed. Six wizard cases pin the behaviour in
the meantime, including the two that matter most: no verdict pill before a
verdict exists, and no add button while the checks are still running.

---

## Phase 4d — Custom tokens, and the rule that had no owner ✅

The last `live in 041` marker is gone, and it went the way it was deferred for:
`firstGroupedQuotePrice` existed **only in the web's TypeScript**, so it was
ported into `vela_core::app::balance_dashboard::first_grouped_quote_price` and
exported over uniffi — not copied into Kotlin. A second hand-written copy of a
rule whose entire history is a mispricing was not worth having, and iOS gets it
for free.

**Why FIRST here and MAX next door.** `best_native_dex_price` takes the deepest
pool across every stable, because a near-empty pool would otherwise price a
chain's own coin. `first_grouped_quote_price` walks the stables in the shell's
preference order — native USDC, then any USDC, then USDT — and takes the first
that answers, because for an arbitrary token the preferred venue is the
trustworthy one and a deeper pool elsewhere may be a different asset wearing a
similar ticker. They look interchangeable and are not; the docs on both now say
so.

The shell's part is the ORDER, and that is the rule's input: which stable comes
first decides which venue prices somebody's token. It comes from the chain
registry's `pickQuoteToken`, not from the executor.

Custom ERC-20s now ride in the same `aggregate3` — balance, then path A (the
token against each stable) and path B (the token against the wrapped native
coin, times the coin's price). **Path B cannot run when the coin has no price**:
multiplying by an unknown is a fabrication, not a fallback.

The tokens come from `vela.customTokens`, the SAME key `token_trust` writes
through — so a token admitted by a confirmed receipt appears in the balances
rather than waiting to be added again by hand.

Four cases in the core (34 → 38 there) and three on Android pin it, including
the 10^12 trap on this new path: two stables with different decimals, only the
18-decimal one quoting, and a price that must come out 0.5 rather than
500,000,000,000.

**Gate**: 356 unit tests, 0 failures; `assembleDebug` passes with the Rust
cross-compile.

---

## Phase 5b — The screens behind "全部" ✅

Four read screens were reachable the whole time and none of them was wired.
Tapping 全部 on Activity, or any activity row, or any asset row, opened a
**fixture**: a person could tap past their own payments into somebody else's,
and tapping their own POL showed a stranger's transaction.

**A row that can be tapped has to say what it is.** Neither `ActivityRowModel`
nor `AssetRowModel` carried an id, so every row opened the same screen — the
navigation had no way to name a target. Both carry one now, and `FlowNavState`
carries the selection beside the stack, cleared on `back()` and `close()` so the
screen underneath can never read an id it was not opened for.

This is the shape the desktop shipped once — a page that displayed contact A
while its delete acted on contact B — and the fix is the same: look the target
up ONCE, from an id the navigation carries.

**A detail with no target renders nothing.** Not the fixture. A screen about the
wrong payment and a screen about the right payment look equally authoritative,
and only one of them is wrong.

That makes **six** drawn-and-unwired surfaces in this feature, and the count is
the finding:

| Surface | What was missing |
| --- | --- |
| network rows | tappable, nothing listening — the probes had no caller |
| add-network search | a read-only box with no `onValueChange` |
| the receive QR | a decorative pattern encoding nothing |
| `focused()` / `backgrounded()` | on the controller, called from nowhere |
| the contact activity block | inherited the fixture's transactions |
| **A1 / A2 / T1 / T2** | reachable from three taps, entirely fixture |

**Gate**: 366 unit tests, 0 failures.

---

## Success criteria

| # | Criterion | Verdict |
| --- | --- | --- |
| SC-101 | real balances and total on a device | ✅ $5.02 over three holdings, arithmetic checked |
| SC-102 | a chain down → cache renders, ban persists | ⚠️ ban persistence tested; the down-chain device run was not staged |
| SC-103 | the feed lists real on-chain transfers | ⚠️ pipeline proven in tests; needs a live deposit (100-block window) |
| SC-104 | a deposit noticed without a refresh | ⚠️ watcher wired and stopping correctly; same live deposit needed |
| SC-105 | `grep -rn 'live in 041'` → zero | ✅ **zero** — phase 4d gave the last rule a Rust owner |
| SC-106 | a custom network added, surviving a restart | ⚠️ search + checks + verdict live and tested; the final device confirmation was cut short by a USB drop |
| SC-107 | zero chain requests outside the pool | ✅ `NoStrayHttpClientTest`; the settings probes are single-URL by design and use the one client |
| SC-108 | bridge delta measured before the work | ✅ +1,133,184 stripped bytes for seven machines (phase 0) |
| SC-109 | the suite stays green and grows | ✅ 210 → **347**, 0 failures |

### The honest shape of this

Six of nine criteria are met or all-but-met, and the three that are not are
staging rather than wiring:

- **SC-103 / SC-104** need money to move while the app is watching, and this
  session had no funded sender. The 100-block monitor window cannot reach
  backwards.
- **SC-106** needs one more device run; the wizard's own behaviour is tested.

Every path they exercise is proven by test and every executor logs what it did,
so each is an afternoon rather than an investigation.

**Final gate**: 366 unit tests, 0 failures, and `grep -rn 'live in 041'` is zero.

### What this feature kept finding

Five separate surfaces were **drawn and not wired**, each looking finished:

| Surface | What was missing |
| --- | --- |
| network rows | tappable, nothing listening — the probes had no caller |
| add-network search | a read-only box with no `onValueChange` |
| the receive QR | a decorative pattern encoding nothing |
| `focused()` / `backgrounded()` | on the controller, called from nowhere |
| the contact activity block | inherited the fixture's transactions |

None of them fails loudly. Every one of them looks like a working screen, which
is why the device pass keeps earning its place: four of the six were found by
looking at a phone, and the last two by asking a question a phone had already
taught me to ask — *what happens when I tap this?*

The generalisation worth carrying into 042: **a drawn screen is not a wired
screen, and the difference is invisible.** Grepping for a marker finds the work
somebody remembered to mark. Tapping every tappable thing finds the rest.

