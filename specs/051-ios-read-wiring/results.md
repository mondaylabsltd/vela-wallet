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

## Next

Phase 2c, the price path — flip `resolve_rate` and `fetch_fiat_rates` live so the
total stops reading `0.00` over real holdings. Then phase 3, `activity_feed`.
The remaining order is in **[tasks.md](./tasks.md)**.
