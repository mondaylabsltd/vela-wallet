# Phase 0 Research — 051 iOS Read Wiring

**Date**: 2026-09-05 · **Branch**: `051-ios-read-wiring` (on `050-ios-live-shell`)

050's thirteen decisions still hold and are not restated. These are the ones
this cut has to make on top of them.

---

## D1 — Where the Multicall3 encoder lives

**The gap**: `aggregate3((address,bool,bytes)[])` is a dynamic array of tuples.
The bridge exports `abi_encode_address`, `abi_encode_uint256`,
`abi_encode_bytes32`, `function_selector` and `decode_calldata` — enough for
`balanceOf(address)` and nothing like enough for this. Web hand-rolled it in
`services/abi.ts`; desktop's 031 hand-rolled it again in `executor/abi.rs`.

**Decision**: a Rust encoder in **`vela-core-uniffi`**, exported through the
bridge, with `alloy-dyn-abi` added as a direct dependency of that crate.

**Why not `vela-core::abi`**, which is the better home and would let desktop
share it: changing `vela-core` forces a `rust/pkg-web` rebuild that must be
committed, because CI's `build-web.mjs --check` rebuilds and compares. Another
session holds the web client right now and its artifacts are mid-flight
(028 is unmerged). That is a **scheduling** reason, not an engineering one, and
scheduling reasons expire — so it is recorded as a consolidation debt rather
than dressed up as a design.

`vela-core-uniffi` is not linked by `vela-core-wasm`, so this costs the web
bundle nothing at all.

**Why not hand-roll it in Swift** — a fourth copy: the constitution's rule is to
use the primitive where it exists, and `alloy-dyn-abi` is already a workspace
dependency two crates away. A hand-rolled head/tail encoder is also exactly the
kind of code that is correct until the first tuple with two dynamic members.

**The measurement that must accompany it**: the `.xcframework` and the committed
`vela_core_uniffi.swift` both grow. 019 recorded that linking crux cost
arm64-v8a +785,864 stripped bytes on Android, so bridge growth is not free and
is measured per cut. Phase 0 records the before, phase 1 the after.

---

## D2 — One pool, and where it lives

**Decision**: `rpc_pool` is an app-level resident, constructed beside the other
machines in `RootView.init` and shared by every read store — not one per
machine, and not one per screen.

FR-002 is the whole point: a ban is a fact about the network, not about a
screen. Two callers with their own endpoint lists is how the Expo client got a
ban map that disagreed with itself.

**The shape this forces**: the other six machines do not talk to the network.
They ask the shell, and the shell asks the pool. So the executors take a
reference to the pool store, and `JsonRpcPost` is the only place a chain read
leaves the app.

**Not a `CoreStore` consumer like the others.** The pool's callers need an
`async` answer (`give me a result for this call`), while `CoreStore` is
fire-and-forget with a view callback. So the pool gets a thin `RpcPool` facade
over its `CoreStore` that turns "dispatch a request, wait for the view to carry
its answer" into one `await`. That facade is the only new plumbing shape in this
cut, and it is written once.

---

## D3 — A seeded account is a read fixture, and must not become a signing one

**Decision**: a dev pin (`VELA_ACCOUNT=<address>`) seeds `vela.accounts` with a
**key-less** record so the read path has an address to read for.

**Why this is legitimate here and would not be in 052**: reading a balance needs
an address and nothing else. Signing needs a key, and the whole point of this
wallet is that the key is in the Secure Enclave and cannot be seeded. A record
with an empty `public_key_hex` can fund a read and can never produce a
signature — the failure, if 052 ever leaned on it, is a refusal rather than a
wrong signature.

**FR-010 makes that explicit** so the pin is not inherited as a way to skip a
ceremony. The golden Safe `0x88cCA0EeDbF2C4426110bbFc998F048689266894` is the
address, because its Gnosis balance is independently checkable with one
`eth_getBalance`.

---

## D4 — The cache renders first, and that is not "fixtures"

`ReadBalanceCache` exists so a cold start shows the last known figure while the
fetch runs. FR-008 says waiting is a neutral surface; a cached figure is **not**
waiting — it is the last thing the core knew, and the drawing has a state for
it.

The rule that keeps this honest: a cached figure is rendered **as the core hands
it over**, including whatever staleness the core marks it with. The shell never
decides a cached number is fresh enough to present as current.

---

## D5 — What the shell may decode, and what it may not

The shell decodes **wire formats**: hex quantities, ABI return data, JSON-RPC
envelopes, a token list's JSON. Those are shapes.

The shell decides **nothing about meaning**: not whether a token is trustworthy
(`token_trust`), not whether a balance set is complete (`balance_dashboard`),
not whether an endpoint should be retried (`rpc_pool`), not which transfers are
a person's own (`activity_feed`).

The line matters most at `token_trust`, because it is the one machine here whose
job is *security*: a token that arrived by transfer is untrusted until the core
says otherwise, and a shell that pre-filters is a shell that has made a security
decision the core was written to make.

---

## D6 — Ported, and the provenance is in the file

Every ported service file carries a header naming its web source. 050 proved why
that pays: the `u32` chain-id defect was findable in web's `decodeSearchIndex`
*because* the iOS file said where it came from, and the fix could be sent back.

Sources for this cut: `wallet-api.ts` (788), `rpc-pool.ts` (334), `abi.ts`
(342), `chains.ts` (231), `activity.ts` (203), `token-metadata.ts` (159),
`balance-cache.ts` (72), `price-service.ts` (130), `token-reads.ts` (86),
`tokens-model.ts` (123), `transactions-model.ts` (46),
`incoming-transfers.ts` (56).
