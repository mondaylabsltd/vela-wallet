# Tasks: Android Read Wiring

**Input**: `spec.md`, `plan.md`, `research.md`

**Tests**: required. This feature replaces staged numbers with money figures;
an untested one is a worse lie than the fixture it replaced.

**Paths**: Android module `app-android/vela-wallet/`, Kotlin under
`app/src/main/java/app/getvela/wallet/` (abbreviated `…/wallet/`).

---

## Phase 0 — Measure (GATE)

- [X] **T101** Add the seven `bridge_object!` registrations (`RpcPool`,
      `BalanceDashboard`, `ActivityFeed`, `ManageTokens`, `TokenTrust`,
      `ReceiveWatch`, `PaymentRequest`).
- [X] **T102** Build arm64-v8a release, strip, record against 040's three
      points. Ceiling: the 24-machine figure (7,890,696).
- [X] **T103** Regenerate `rust/bindings/kotlin/` and the three ABIs' jniLibs.

---

## Phase 1 — The transport and the pool (US1)

- [X] **T104** `…/core/net/VelaHttp.kt` — the single okhttp client: timeouts,
      no redirects to other hosts, no cookies. The ONLY HTTP client in the
      read path.
- [X] **T105** `…/feature/wallet/core/RpcWire.kt` — `RpcOperation` (7),
      `RpcShellResult` (7), `RpcPoolView` and the types they carry. Numerics
      transcribed from the Rust (research D5).
- [X] **T106** `RpcPoolExecutor.kt` — `load_pool_config` (seeded from
      `network_admin`'s view, research D4), `json_rpc_post`, `probe_chain_id`,
      `draw_jitter`, `start_backoff`, `persist_bans`, `conclude`.
- [X] **T107** `RpcPool.kt` facade — owns the fetch and the promise; suspends
      the caller until the core concludes the call, keyed by `call_id`.
- [~] **T108** Endpoint admission before any request (FR-104) — **moved to
      phase 2**: nothing untrusted reaches the pool yet (it is seeded from the
      core's own network list), and the tiers that bring untrusted URLs are
      phase 2's. Admission lands with them, not after.
- [X] **T109** `RpcPoolExecutorTest` + `RpcPoolMachineTest` — a fake transport
      over the REAL machine: a failing endpoint is routed around, the ban is
      persisted under the shared key, a 429 is a transient verdict.
- [X] **T110** `NoStrayHttpClientTest` — SC-107: no file under
      `feature/*/core/` may construct an HTTP client except the pool's.
- [X] **T111** Drift-test entries for every 041 wire type.

---

## Phase 2 — The ten network arms (US4)

- [ ] **T112** `fetch_chain_info` + `fetch_search_index` against the chain
      index (this is also what unblocks T131).
- [ ] **T113** `probe_rpc`, `probe_reachable`, `rpc_get_code`,
      `rpc_call_p256` through the pool.
- [ ] **T114** `fetch_service_health`, `fetch_fiat_rates`.
- [ ] **T115** `invalidate_pools`, `clear_bundler_cache` act on the real pool.
- [ ] **T116** Settings health tiles and endpoint rows render live verdicts;
      no fixture latency survives on a signed-in route.
- [ ] **T117** Tests for each arm, including the failure variants they must
      keep for genuine failures.

---

## Phase 3 — The rate (US4)

- [ ] **T118** `CurrencyExecutor.resolve_rate` fetches a real rate; `null`
      still degrades rather than defaulting to 1.
- [ ] **T119** Test: a rate arrives → the view converts; the source is down →
      `rate` stays null and the USD figure shows.

---

## Phase 4 — Balances (US2)

- [ ] **T120** `BalanceWire.kt`, `MtokWire.kt`, `TrustWire.kt`.
- [ ] **T121** `BalanceExecutor`, `ManageTokensExecutor`, `TokenTrustExecutor`.
- [ ] **T122** `WalletController` — the three machines, app-resident.
- [ ] **T123** `WalletLive.kt` — hero, network filter, asset rows, section
      states (loading / empty / rows).
- [ ] **T124** **The two inherited traps** (research D6), each with a test:
      balances cross the wire as human decimals; Tempo is excluded by the
      core's own `TEMPO_CHAIN_IDS`, never by a magnitude threshold.
- [ ] **T125** `VelaNavHost` — the wallet route reads the live builder; the
      gallery keeps its fixtures.
- [ ] **T126** Device check (SC-101/102): real balances; one chain's endpoints
      down → cache renders and the ban persists.

---

## Phase 5 — Activity (US3)

- [ ] **T127** `FeedWire.kt`, `ActivityFeedExecutor`, feed in the controller.
- [ ] **T128** `WalletLive` grows the feed; the activity screens (`A1`–`A3`)
      and the contact-detail block 040 left empty.
- [ ] **T129** Device check (SC-103).

---

## Phase 6 — Receive (US3)

- [ ] **T130** `receive_watch` + `payment_request`: wire, executors,
      controller, the receive flow (`R1`–`R4`).
- [ ] **T131** Device check (SC-104): a deposit noticed without a refresh.

---

## Phase 7 — Contacts backfills (US4)

- [ ] **T132** `resolve_identity`, `classify_recipient`, `load_send_history`.
- [ ] **T133** Tests: a resolvable name resolves; an unreachable lookup stays
      `null` and claims nothing.

---

## Phase 8 — Add a network (US4)

- [ ] **T134** FR-111: the add-network wizard works against the chain index.
- [ ] **T135** Device check (SC-106): added, survives a restart — 040's
      deferred criterion.

---

## Phase 9 — Closeout

- [ ] **T136** `grep -rn 'live in 041'` → zero.
- [ ] **T137** Full gate: unit + instrumented + `assembleDebug` + galleries.
- [ ] **T138** `results.md`: ten criteria verdicted, measurements, debts for
      042, and a handover.

## Dependencies

```text
Phase 0 (GATE) ─► Phase 1 (the pool) ─┬─► Phase 2 ─► Phase 8
                                      ├─► Phase 3
                                      ├─► Phase 4 ─► Phase 5
                                      ├─► Phase 6
                                      └─► Phase 7
                                                      Phase 9 ◄─ all
```

Nothing after Phase 1 can start before it: every one of them reads a chain.
