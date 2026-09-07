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

## Next

Phase 2, `balance_dashboard` — the `$1,383.28` screen. The plan, the two traps
that cost desktop a defect each, and the ABI gap that has to be settled first
are all in **[tasks.md](./tasks.md)**.
