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
