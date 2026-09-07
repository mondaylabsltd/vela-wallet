# Results — 051 iOS Read Wiring

Written as the work lands. **[tasks.md](./tasks.md) is the handoff** — it holds
the commands, what is next, and the traps. This file holds what happened.

---

## Phase 0 — Baselines

Branch point: `050-ios-live-shell` @ `07b4ccad`.

| | |
|---|---|
| `@Test` functions | **203** |
| Device Debug dylib | 57,346,720 bytes |
| Committed `vela_core_uniffi.swift` | 215,916 bytes |
| Literal-audit violations | 35 (never green on `main`; the gate is "no new ones") |
| `// live in 051` markers | 5 |
| `// live in 052` markers | 3 |

The dylib figure is a **Debug** device build — not what ships, but a consistent
before/after for the bridge growth this cut is responsible for.

---

## Phase 1 — `rpc_pool`, and the one shape this cut had to invent

Committed `870a86bd`.

### Seven exports at once, against 050's own rule

050's D10 said one `bridge_object!` per cut, so an export always arrives with
the code that calls it. This cut adds **seven**, and the rule is honoured rather
than broken: this cut wires all seven. The alternative was seven `.xcframework`
rebuilds at five to eight minutes each for an identical end state.

### What the pool's shell half actually is

`rpc_pool.rs` is 1,975 lines of routing decisions and none of them are in Swift.
Its module doc reserves exactly one half for the shell, and that is what shipped:

> The shell keeps the fetch execution: it holds the request payloads keyed by
> `call_id`, performs `JsonRpcPost`, and reports transport outcomes back — this
> core only ever decides *which URL next and why*.

So `RpcPool` mints a `call_id`, holds the payload and the response bodies under
it, and resolves the caller when `Conclude` names a winning URL. **A response
body never travels through the core**: routing does not need it, and a model
carrying `eth_getLogs` output through would be paying for the privilege of
ignoring it.

It is the only machine in either cut with a facade, because it is the only one
whose caller wants an *answer* rather than a view.

### Bans are collected through, not filtered

The subtle one, and it has a test. The core excludes banned URLs at **selection**
(invariant ⑧) so its all-banned self-rescue has something to reconsider. A shell
that helpfully removed them at *collection* would leave the core rescuing an
empty list. `isBanned` is threaded through and always answers `false`, exactly as
web's `NEVER_BANNED` does.

### A reply is classified, not interpreted

`CoreHTTP.rpcEnvelope` distinguishes a JSON-RPC error from a 429 from a 500 from
a timeout from a refused connection. Those are not this file's convenience —
they are the vocabulary `rpc_pool` bans on, and flattening them (as
`CoreHTTP.rpc` does for its own callers) would hand the core one failure where
it needs five.

### Proven against the real network

```
[live] golden Safe on Gnosis: 0xa8867319d2da000
```

**0.75897 xDAI** — the same figure desktop's 031 measured independently, from a
different client, through a different HTTP stack. The caller named no URL.

### Two live tests were wrong, and both were mine

1. **Gnosis is a built-in.** The test typed `100` and demanded a compatibility
   verdict; `already_added` is the correct and only answer, so it was asking the
   core to contradict itself. Rewritten as two tests: a built-in is refused, and
   Zora (not a built-in) actually runs the pipeline.
2. **Chain 999999999 exists.** It answered `eth_chainId` with `0x3b9ac9ff`. "A
   chain nobody serves" needed a `u32`-max id, not a big-looking one.

### A number reported to the founder, withdrawn

050's closeout said *"adding a network takes 42–90 seconds, and the drawn 检查中
state has to carry a full minute"*. **That was my settle loop timing out**, not a
measurement of the wizard — it was waiting for a verdict on a chain the core had
already refused.

Re-measured on Zora, which actually runs index → resolve → RPC race → eleven
`eth_getCode` reads → P256:

| | |
|---|---|
| A real add | **3.6 s** |
| A built-in's refusal | **2.3 s** |

Nothing needs redesigning around it. 050's results.md carries the withdrawal in
place rather than a quiet edit, because the wrong number had already been
reported and a handoff that silently improves is one nobody can trust.

### Gates

Tests 203 → **215**, plus 6 live behind `-DVELA_LIVE_TESTS`. Device UI suite
green. Literal violations 35 → 35. Zero lines under
`rust/crates/vela-core/src/app/`; zero corpus delta.

---

## Phase 2a — the ABI gap closed, and the unit trap tested

### D1 settled, and measured

`multicall3_encode_aggregate3`, `multicall3_decode_aggregate3` and
`erc20_encode_balance_of` now live in **`vela-core-uniffi/src/multicall.rs`**,
built on `alloy-dyn-abi` (added as a direct dep of that crate).

`aggregate3((address,bool,bytes)[])` is a dynamic array of tuples with a dynamic
member — head/tail offsets nested two deep. The bridge's `abi_encode_address` /
`_uint256` / `_bytes32` cannot express it, and a hand-rolled encoder is correct
right up until the first tuple with two dynamic members. **I proved that on
myself**: a first version of the Swift test hand-wrote the return-side ABI and
was rejected by the decoder with `type check failed for "offset (usize)"`. The
test now asserts what Swift actually owns — that garbage is *refused* rather
than decoded into a plausible empty list — and the slot-preservation property
stays where alloy can build the fixture, in Rust.

Bridge growth for the three exports: committed bindings **215,916 → 260,699
bytes (+44,783)**. The wasm bundle pays nothing, because `vela-core-wasm` does
not link this crate — which was the point of putting it here rather than in
`vela-core`.

### The unit trap, tested with a number two clients agree on

`balance_dashboard.rs:298` requires a **human decimal string, never a JSON
number**. Desktop's 031 wrote raw units and the defect was invisible for as long
as prices were `None`.

`TokenReads.scaled` converts in **decimal string arithmetic, not `Double`** — a
`Double` carries 15–16 significant digits and a balance routinely needs more, so
going through one rounds somebody's money quietly. The tests pin
`0xa8867319d2da000 → "0.75897"` (the figure the live pool returned and desktop's
031 measured independently) and a 21-digit balance that a `Double` would mangle.

Tempo's exclusion is by the chain's **declared gas model**, never a "that number
looks too big" threshold — a threshold would also reject a genuine whale.

Tests 215 → **224**. Literal violations 35.

---

## Phase 2b — the home screen shows the person's own money

`BalanceExecutor` (7 operations, every chain read through `RpcPool`),
`WalletLive` (a partial sibling of `WalletFixtures`, like `SettingsLive`),
`WalletStore` (resident) and `RootView`'s `.wallet` case.

`VELA_ACCOUNT=0x…` seeds a **key-less** account so the read path has an address
without a passkey ceremony (research D3).

### Seen on screen

`$1,383.28` is gone. The home now reads **xDAI · Gnosis · 0.75897** — the golden
Safe's real balance, fetched through the pool and converted to a human decimal.

### Two defects the live screen found, both invisible before it

1. **Every unpriceable holding was rendered twice.** `assetRows` concatenated
   `tokens + unpricedTokens`, and `unpricedTokens` is **not the complement** —
   the core's own doc calls it *"the detail sheet's 'couldn't be priced' list"*,
   a subset built for a different surface. One address, one chain, two identical
   xDAI rows. With a fixture there was nothing to duplicate.

2. **The dev seed persisted, which is exactly the FR-010 hazard.** It only ever
   *wrote* the account, so the next launch without the pin was still signed in as
   a key-less record. The XCUITest suite found it within one run — two onboarding
   tests failed because the app booted into a wallet nobody had asked for. The
   seed now **removes its own record when the pin is absent**, which makes "never
   inherited as a way to skip a ceremony" structural instead of a promise.

### A known intermediate state, not a defect

The total reads **`0.00` with the warning triangle** while a real 0.75897 xDAI
sits below it. There is no price source yet: `display_currency::resolve_rate` and
`network_admin::fetch_fiat_rates` are both still on this cut's own
`// live in 051` list. The core is answering "the priced sum is zero", the drawn
warning is saying so, and it resolves when the price path lands later in 051.

Tests 224 (hermetic) + 8 UI, all green. Literal violations 35.

---

## Phase 2c — the price path

### The handoff's own claim, corrected

The previous "Next" said flipping `resolve_rate` and `fetch_fiat_rates` would
stop the total reading `0.00`. **It would not have.** Those two are the *fiat*
rate — USD → the person's currency — and the total was zero because every
`BalanceToken` carried `price_usd: nil`. Two different price paths were sharing
one name. This phase does both, because they share the machinery (Chainlink
feeds read through Multicall3), but the one that fixes the hero is the token
price.

### `choose_native_price`, exported rather than re-decided

The ladder — DEX preferred, but a DEX price outside ratio (0.5, 2.0) against the
best Chainlink read means low liquidity, so Chainlink wins — has been in
`balance_dashboard.rs` since spec 017 while **every platform re-decided around
it** in `wallet-api.ts`. Web opened the door in 025; this is the same door on the
uniffi side (`vela-core-uniffi/src/prices.rs`).

It is not a style point. The band exists because one near-empty X Layer pool
quoted WOKB at ~$5 against a real ~$81, and a shell that re-implements it is a
second opinion nobody diffed.

Bindings **260,699 → 263,535 bytes (+2,836)**. `best_native_dex_price` is
deliberately **not** exported: iOS has no DEX quotes to fold, and 050's D10 says
an export arrives with the code that calls it.

### What is NOT priced, and says so

The DEX rung needs per-chain master data this client does not have
(`fetchChainTokens` → router, wrapped native, the chain's stables). Until it
lands, `dex` is passed as **`nil` rather than approximated**, and custom ERC-20s
stay unpriced. A stablecoin priced at "$1 because the symbol looks like one" is
exactly the invention the core's unpriced notice exists to avoid.

### The fiat waterfall

`FiatRates` (Chainlink's 16 fiat feeds, ENS-addressed on mainnet, per-feed
`decimals()` because PHP's is 18 where most are 8) then `FiatFx` (the
configurable endpoint, both provider response shapes). Both persist under web's
own keys and TTLs — `vela.fiatRates.v1`, `vela.fiatFeedAddrs.v1`,
`vela.fxRates.v1` (FR-005).

The order is the whole rule, so it lives in **one place**: the array of
`FiatRateSource`s the executor is built with. The hermetic tests construct it
with no sources at all, which is why they never touch the network — and is a
state a real device reaches too, on a plane.

### The hero converts, or it says USD

`display_currency` owns the rate and the shell owns formatting, so the home hero
multiplies and wears the chosen code and glyph — and when the rate is `nil`, it
shows the **USD figure under USD**. Relabelling the same digits with a ¥ is the
lie FR-009 exists to prevent. `SettingsLive.currencyRowValue` already applied
that rule to a settings row; it now applies to the biggest number in the app,
from one `WalletLive.Display`.

The currency machine is booted from the **home** screen's `.task` as well, since
which currency somebody reads their money in is app-wide — waiting for a visit to
设置 would show a dollar figure to a person who chose CNY and never went looking.

### Seen, live

```
[live] mainnet feeds: ["AVAX": 7.93, "DAI": 0.9996, "MATIC": 0.0973, "ETH": 2518.39]
[live] golden Safe xDAI: 0.75897 at $0.99975138
[live] USD→EUR (Chainlink): 0.861      [live] USD→HKD (endpoint): 7.8403
[live] USD→CNY (waterfall): 6.7107
```

On the phone, and on a simulator whose stored currency is CNY: **总余额 · CNY,
¥5.02**, with the xDAI row reading `0.74797 / ¥5.02`. The warning triangle is
gone, because there is now a price behind the figure.

### Two things the world disagreed with

1. **The mainnet BNB/USD feed is dead.** `0x14e613AC…75d25`, carried verbatim
   from web's `price-service.ts`, answers `0x` — no contract. Nothing on screen
   is wrong today, because BSC's *local* feed prices BNB through the ladder's
   `chainlink_local` rung. But web has the same dead entry, and a coin that ever
   appears off its home chain would be unpriced for this reason.
2. **The fiat endpoint quotes 30 currencies, not "~160 incl. VND".** The
   deployed `vela-currency` instance returns the ECB set. Of the picker's eight,
   seven price (five via Chainlink, HKD via the endpoint) and **VND prices
   nowhere** — it degrades honestly to USD, which is the rule working, but a
   person who chooses VND gets a dollar figure forever. Founder's call: drop VND
   from the catalog, or fix the deployment. It is not a code defect and was not
   "fixed" by making the code agree with the comment.

### Two tests that were wrong about the world, again

The VND live test above (asserted a rate the endpoint has never quoted), and a
device acceptance test that pinned `总余额 · USD` — the simulator had CNY stored
and drew `总余额 · CNY`, **correctly**. Both assertions were rewritten to say
what is actually durable; neither the endpoint nor the app was changed to make a
wrong test pass.

### Gates

Hermetic tests 224 → **253**; live (flagged) 6 → **13**; device UI 8 → **9**.
Literal violations 35 → 35. Zero lines under `rust/crates/vela-core/src/app/`.

**A gate command in tasks.md needs correcting**: `git diff origin/main --
rust/crates/vela-core/src/app/` is no longer empty, because `main` has moved
under this branch — it now reports somebody else's `send.rs` work. The gate this
cut owes is against the **merge base**:
`git diff --stat $(git merge-base origin/main HEAD) -- rust/crates/vela-core/src/app/`.

---

## Phase 3 — `activity_feed` + `token_trust`, because the feed is the scan

### Two machines, one screen, and why they could not be split

tasks.md planned `activity_feed` for phase 3 and `token_trust` for phase 4. They
shipped together, and the reason is structural rather than convenient:
`ScanIncomingTransfers` — the operation that makes the feed anything at all — is
**routed through `token_trust` on every client**. Its judged incoming list is
what a receipt record is built from.

Without it, `activity_feed` on this client would read a local store that nothing
has ever written (sends are 052) and render an empty screen. That is not a phase;
it is a phase's worth of code with nothing to show.

### What was ported

| | |
|---|---|
| `Core/TxRecords.swift` | `vela.transactionHistory` — load, merge (de-dupe by id, newest-first, 200 cap), delete, and the stored→wire projection |
| `Core/TokenMetadata.swift` | `symbol()` + `decimals()` over Multicall3, memoised and persisted under web's `vela.tokenMeta.*` keys |
| `Core/ChainTokens.swift` | the registry's stablecoins + wrapped native, which ARE the scan's allowlist |
| `Core/HeldTokens.swift` | what the last balance read found — web gets it from `fetchTokens`' cache; there is no such cache here, so it is explicit |
| `Core/Multicall.swift` | one batch shape, shared by balances, prices and metadata |

### The range cap, where it always belonged

Web classifies an `eth_getLogs` failure by **matching the provider's wording** in
its executor. Here it does not have to: `rpc_pool` already parses the cap, and
the shell reads `RpcOutcome.rangeCap`. Phase 1's results.md predicted that case
would earn its keep in phase 3; this is where.

### A bug the live suite caught in one run

`pool.call(kind: "logs")` — `kind` is the endpoint **class** (`rpc` or
`bundler`, the two tiers the pool scores separately), not a label for the
method. The core rejected the event outright (`unknown variant \`logs\``) and the
scan hung to its 60-second deadline. A hermetic test would not have found it: the
core only sees the event when something actually calls a chain.

### Seen, live

```
[live] receipts found: 108                        (last 100 Ethereum blocks)
[live] newest: 1069120000 USDT from 0xde195056…   (raw, 6 decimals)
[live] stored: 1069.12 USDT usd=$1,069.12         (human decimal + ingest valuation)
[live] rescan of the same window: 0 new           (no double celebration)
[live] golden Safe receipts on Gnosis: 0
```

On the simulator, seeded with that same public address, the home draws a 今天
group of real receipts — `+1,069.12 USDT`, `+406.926592 USDT`, `+10,000 USDT`,
each 来自 its shortened sender — under a hero of ¥1,086,541,760.24. The fixture's
两 rows (已发送 −2 POL, 已收到 +120 USDT) are gone.

`1069120000 → "1069.12"` is the metadata gate doing its job: with the
18-decimals fallback web's original had, that receipt would have been stored as
`0.00000000106912` — the "+0 tokens" defect `token_trust`'s invariant ③ exists to
prevent.

### The feature's honest shape: a watcher, not a history

**This deserves a founder decision, and it is not a defect.**
`token_trust::LIVE_SCAN_BLOCKS` is **100 blocks**, and a native coin transfer
emits no `Transfer` log at all on a chain without EIP-7708. So:

- the golden Safe's own feed is **empty**, correctly — its xDAI arrived as a
  native transfer, and outside a hundred-block window besides;
- a fresh install shows an empty feed for an account with years of on-chain
  activity, because the local store is the source of truth and this device has
  never recorded anything;
- what the feed *does* catch is an ERC-20 receipt landing **while the app is
  running**, plus (from 052) whatever this device sends.

Every client behaves this way — it is the same core — so it is a product fact,
not an iOS gap. Giving somebody a real history would need an indexer, which is a
different feature with a running cost.

### Waiting is not empty

An unread machine and an account with no history both publish zero rows, and
they must not draw the same. The section stays on the drawn **skeleton** until
the store read lands, and only then says 暂无交易记录 (FR-008). "Nothing has
happened here" is a claim, and it must not be made before anybody looked.

### Still unreachable, and named

The core owns tap-to-hide and pull-to-refresh; **no gesture reaches either** —
`WalletStore.togglePrivacy` and `refresh(pull:)` have no call site, and neither
does the chain-filter pill or the receipt toast. Phase 2b left them and this
phase did not fix them; they are listed in tasks.md rather than quietly carried.

### Gates

Hermetic tests 253 → **273**; live (flagged) 13 → **16**. Literal violations 35.
Zero lines under `rust/crates/vela-core/src/app/`; no Rust change at all in this
phase — the seven bridge exports were all landed in phase 1.

---

## Next

Phase 4, the rest of `manage_tokens` (the token list surface; `token_trust`
landed with phase 3). The remaining order is in **[tasks.md](./tasks.md)**.
