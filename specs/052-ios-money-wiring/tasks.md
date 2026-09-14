# Tasks — 052 iOS Money Wiring

**Read this first if you are picking the branch up cold.** It is written to be
enough on its own: the commands, what is done, what is next, and the traps.

Branch `052-ios-money-wiring`, stacked on `051-ios-read-wiring`, in the
worktree `/Volumes/data/production/vela-wallet-ios`.

---

## The six commands

```bash
# 0. A fresh checkout cannot build until this has run once (Artifacts/ is
#    gitignored). This cut does NOT change vela-core-uniffi, so it should run
#    exactly once — if you find yourself re-running it, something was added
#    that the spec says is not needed.
rust/scripts/build-ios-xcframework.sh                      # ~5 min cold
rust/scripts/build-ios-dev-fixtures.sh                     # phase 2 onward

cd app-ios/VelaWallet                                      # xcodebuild needs this cwd

# 1. Build + hermetic tests. iPhone 15 Pro is on iOS 17.5, so `OS:latest` does
#    NOT match it — address the simulator by id.
xcodebuild -project VelaWallet.xcodeproj -scheme VelaWallet \
  -destination 'id=84146B7B-C679-46AC-8426-41AA42E6F403' test

# 2. The live suites (real relay, real chain). They read; they never submit.
xcodebuild ... -only-testing:VelaWalletTests/RelayLiveTests \
  OTHER_SWIFT_FLAGS='$(inherited) -DVELA_LIVE_TESTS'

# 3. On the founder's iPhone. Run it ALONE — sharing the CPU with cargo makes
#    the ten-second waits flake. Screenshots come back inside the .xcresult.
xcodebuild ... -destination 'platform=iOS,id=00008130-001C68C804E1401C' \
  -only-testing:VelaWalletUITests/LiveWiringAcceptanceTests \
  -resultBundlePath /tmp/accept.xcresult test
xcrun xcresulttool export attachments --path /tmp/accept.xcresult \
  --output-path /tmp/accept-images

# 4. The parallel space.
SIMCTL_CHILD_VELA_PARALLEL_SPACE=1 xcrun simctl launch \
  84146B7B-C679-46AC-8426-41AA42E6F403 app.getvela.VelaWallet
xcrun devicectl device process launch --device 00008130-001C68C804E1401C \
  -e '{"VELA_PARALLEL_SPACE":"1"}' app.getvela.VelaWallet

# 5. Gates. The literal audit is NOT green on main — 35 pre-existing
#    violations. The gate is "no new ones". Diff against the MERGE BASE:
#    main has moved under this branch.
node app-ios/scripts/audit-literals.mjs                    # expect 35
B=$(git merge-base origin/main HEAD)
git diff --stat $B -- rust/crates/vela-core/src/app/                              # empty
git diff --stat $B -- rust/crates/vela-core/src/i18n_catalogs/                    # empty
git diff --stat $B -- app-ios/VelaCoreKit/Sources/VelaCore/vela_core_uniffi.swift # empty
git diff --stat $B -- app-web/ app-desktop/ app-android/ app-browser-extension/   # empty
```

---

## Done

### Phase 0 — baselines and the papers ✅

The spec set: [spec.md](./spec.md), [plan.md](./plan.md),
[research.md](./research.md) (D0–D13), [data-model.md](./data-model.md),
[contracts/shell-operations.md](./contracts/shell-operations.md) (all 30
operations), [contracts/i18n-keys.md](./contracts/i18n-keys.md),
[quickstart.md](./quickstart.md), [checklists/requirements.md](./checklists/requirements.md).

Baselines in [results.md](./results.md).

**The finding that changed the cut's shape**: all 23 machines and all eight
`user_op_*` functions are **already exported and already in the committed Swift
bindings** (research D0). So this cut adds no bridge line and no bindings
regeneration — the first iOS cut that does not.

---

### Phase 1 — the money plumbing ✅ committed `0e2fcb1e` · `8d1e6282`

`Core/{RelayClient,UserOpSpine}.swift`, `Features/Send/Core/UserOpSigner.swift`,
`CoreHTTP.getREST`, `RpcPool.bundlerBase`, `TxRecords.{writeRecords,patch,
pending}`. 331 → 356 hermetic tests. No screen changed.

Two defects found, both in results.md: the pool was dropping the relay's own
sentence (`RpcOutcome.rpcError` now exists), and `relay_error_message` can eat
the `[existingHash:]` marker that prevents a double spend.

### Phase 2 — the parallel space ✅ committed `dbdab017` · `866da5c5` · `bafa82e8`

A second **staticlib** xcframework linked only by Debug.
`rust/scripts/build-ios-dev-fixtures.sh` builds it. SC-010 measured with a
control (Release 0 / Debug 567 fixture symbols). Device-verified: the space
opens on `0x88cC…6894` and closes cleanly.

**The whole acceptance suite is 12 of 12 on the founder's iPhone.**

---

### Phase 3 — `fee_policy` + `send`, up to confirm ✅ committed `9baacaeb`

The picker is the person's own money and the tapped token carries into the form
— device-verified. Four defects on the way, all in results.md; the one worth
carrying is that **a machine that only boots on its own page is unreadable from
another page**, which cost Android its contact picker and cost this the chain
list.

---

### Phase 4 — sign and submit ✅ committed `411ff1ab`

**Money moved.** Dust left the golden Safe from the founder's iPhone: user op
`0xcf9fcae6…bd269195`, tx `0x151d63c8…57b9`, Gnosis block `0x2e014dd`,
`success: true`. Five defects on the way, all in results.md; the one that cost
most was the **bundler endpoint missing its `/{chainId}`** — wrong since 051,
invisible because 051 never called a bundler.

The live send is `testDustLeavesTheGoldenSafeAndComesBackAsAReceipt`, behind
`-DVELA_LIVE_SEND` because it spends. `RelayLiveTests` (behind
`-DVELA_LIVE_TESTS`) prints what the real relay answers — it is what found the
endpoint defect, and it is the first thing to run when a quote misbehaves.

---

## Next — Phase 5: `tx_tracker`

- [ ] **T501** `TrackerStore` resident, booted at launch, `LoadPendingTxs` from
      `TxRecords.pending`. A force-quit must lose nothing.
- [ ] **T502** The six operations (`TrackerExecutor`), receipts through
      `RelayClient.userOpReceipt` — which already returns the logs, so
      `notify_confirmed` can feed `token_trust::ReceiptLogsConfirmed`.
- [ ] **T503** Foreground 3-second tick while anything is pending; nothing when
      nothing is.
- [ ] **T504** `scenePhase` → `AppResumed`; the background grace and the
      `BGTaskScheduler` request (research D2). **Claim no cadence** — claim
      that no verdict is lost.
- [ ] **T505** The notification, and the tap that opens the receipt.
- [ ] **T506** Device: submit, force-quit, relaunch — the row is there and
      reaches confirmed (SC-003).

**The hash to poll while testing**: `0xcf9fcae6…bd269195` is already confirmed,
so it is a good fixture for "a receipt that exists".

### Then

Phase 6 refusals and the two sheets · Phase 7 the three markers and closeout.

---

## Phase 4's task list, for reference

The first phase where money can move. The spine is already written and tested
(phase 1); what is missing is the screen reaching it.

- [ ] **T401** SD3's CTA dispatches `slide_confirm`. The core raises
      `submit_user_op`; `SendExecutor` already answers it through the spine.
- [ ] **T402** `signingStarted` reaches the screen, and `cancel_signing`
      cancels the task. **One prompt per attempt** is the test, counted on the
      signer — not a reading of the code.
- [ ] **T403** `persist_tx_records` before `track_submitted` — the core emits
      them in that order and the shell must not make the write asynchronous.
      The feed shows the row from that moment.
- [ ] **T404** SD4a/b/c reachable from the core's `receipt.status`, not from
      `FlowNav.steps` (which only knows `.sd4b`).
- [ ] **T405** Device: dust from the golden Safe on Gnosis, in the parallel
      space. Fund it first, and check the relay's treasury is covered.

**Watch for**: the Tempo branch needs `accountInfo` before it can build a fee
leg, and its quote's recipient must match the collector — the spine refuses
otherwise, before any prompt.

### Then

Phase 5 `tx_tracker` · Phase 6 refusals and the two sheets · Phase 7 the three
markers and closeout.

---

## Phase 3's task list, for reference

The first phase with a screen. The order to follow is web 026's "machine order
to repeat", and it is not negotiable:

- [ ] **T301** `Features/Send/{FeeWire,FeeStore,FeeExecutor}.swift` — the six
      `fee_policy` operations over `RelayClient`. One quote in flight per
      surface, keyed by a generation token, so a stale answer cannot land on a
      newer attempt.
- [ ] **T302** `Features/Send/{SendWire,SendStore,SendExecutor}.swift` — the
      eighteen arms. `estimate_fee` dispatches into the resident fee session and
      awaits its view (research D6); `add_network` answers `Error` (055 owns the
      scanner, its only entry); `simulate_calls` answers `sim_json: null` (055).
- [ ] **T303** `Features/Send/SendLive.swift` — `SendView` → `FlowScreenModel`,
      a sibling of `WalletFlowFixtures` exactly as `WalletLive` is of
      `WalletFixtures`. The gallery must stay pixel-identical.
- [ ] **T304** `FlowHost`/`FlowNav`: SD1 → SD2 → SD3 read the core. **The
      confirm CTA is a button, not a slider** (the signing sheet's is the
      slider — Android 045 recorded the difference).
- [ ] **T305** A warm quote on `select_token` (web 028 ph10): the fee is being
      fetched while the person types an amount, not after they tap 继续.
- [ ] **T306** Drift tests for all three families; `neutralAnswer` per machine.

**Watch for**, from Android's device runs — each of these cost a day there:

- an input bound straight to the machine loses characters on the round trip
  (local echo, ignore the echo);
- the relay's quote rows carry `balance` as **hex** and `feeToken` as **null**
  (already handled in `RelayClient.quoteRow`, with a test);
- the feed only re-reads on focus;
- the system back gesture must not close the whole flow from any step.

### Then

Phase 4 sign and submit · Phase 5 `tx_tracker` · Phase 6 refusals and the two
sheets · Phase 7 the three markers and closeout.

---

## Phase 1's task list, for reference

- [ ] **T101** `Core/CoreHTTP.swift` — add a REST `get(url:headers:timeout:)`.
      It has `rpc` and `rpcEnvelope` and no plain GET with a header, and the
      relay's treasury and account-info endpoints are REST.
- [ ] **T102** `Core/RelayClient.swift` — port `RelayClient.kt` (447 lines).
      The bundler base is whatever the pool names for the chain; the pool's
      best RPC URL rides as `X-Rpc-Url`. Two caches: in-band quote 8 s, account
      info 30 s, both cleared by `clearCaches()`. `probeTreasury`'s **404 is
      "uncovered", not an error**.
- [ ] **T103** `Features/Send/Core/UserOpSigner.swift` — the protocol,
      `PasskeyUserOpSigner` over `PasskeyExecutor.assert`. Nothing else.
- [ ] **T104** `Core/UserOpSpine.swift` — port `UserOpSpine.kt` **literally**.
      The order is the file's content (research D4). Port `signMessage` too,
      even though 052 never calls it: it is 053's, and splitting the file
      across two cuts is how a transfer and a dApp transaction become two
      opinions.
- [ ] **T105** `Core/TxRecords.swift` — add `writeRecords(_:store:)`: one
      atomic write, de-duped by id, newest first, capped at 200. Keep `merge`
      as it is; the two have different jobs.
- [ ] **T106** Tests: the spine's refusal ladder with a stub relay (unreadable
      nonce refuses **before** the signer is called — assert the signer's call
      count is zero); `quotedFeeUsable` rejecting a stale quote; the relay
      client's classification of a rejection, an idempotent re-submit and a 404
      treasury; `writeRecords` round-tripping and capping.

**Gate**: build, tests, literal audit 35, no screen changed.

---

## Then

### Phase 2 — the parallel space (research D1, D2)

`crate-type` += `staticlib`; `rust/scripts/build-ios-dev-fixtures.sh`;
`app-ios/.gitignore` += `VelaDevFixturesKit/Artifacts/`; the Debug-only build
settings; `Core/ParallelSpaceHook.swift` (always compiled) +
`Dev/ParallelSpaceBinding.swift` and `Dev/vela_dev_fixtures.swift` (Debug
only, the latter generated and committed); the door in `RootView.init` after
`DevAccountSeed.applyIfRequested` and before `session.boot()`; the badge.

**Watch for**: a duplicate-symbol link error the first time two Rust archives
meet. It is measured as unlikely (research D1) and the fix stays inside the
build script. **Verify SC-010 in this phase**, not at closeout — a Release
archive that carries the keyset is cheaper to find now.

### Phase 3 — `fee_policy` + `send`, up to confirm
### Phase 4 — sign and submit (the first phase where money can move)
### Phase 5 — `tx_tracker`, the notification, the resume
### Phase 6 — refusals, the fee-token sheet, the contact picker
### Phase 7 — the three markers, the device pass, closeout

---

## The traps, inherited and new

1. **`platform=iOS Simulator,name=iPhone 15 Pro` does not resolve** — it means
   `OS:latest`, and 15 Pro is on 17.5. Use the UDID.
2. **`xcodebuild` needs `cd app-ios/VelaWallet`**; the Bash tool's cwd resets.
3. **Swift Testing's pass line is `✔ Test run with N tests…`** — `Executed 0
   tests` above it is not a failure.
4. **`assertionFailure` is a no-op in Release.** Use `print` and answer anyway.
5. **`CoreStore.onFault` is silent by default** — pass a logger to every store
   or a malformed event stops a screen quietly.
6. **An event dispatched before `boot()` is dropped**, so a facade's
   continuation would never resume. `RpcPool` refuses rather than hangs; do the
   same for the fee session.
7. **`var onRefresh: (() async -> Void)?` stored in a SwiftUI `View` segfaults
   AttributeGraph at launch.** Box async closures in a class. Plain
   `(() -> Void)?` is fine.
8. **One automation session at a time** — a stray `devicectl … --console` holds
   the device and the next run reports `Authentication canceled`.
9. **New**: `DevAccountSeed` replaces the whole account list. The parallel
   space must **upsert** (FR-003). The founder's own wallet is on that phone.
10. **New**: `timestamp` in a stored record is **seconds**. Writing
    milliseconds puts every send in the year 57000 and the feed's day grouping
    silently stops working.
11. **New**: `persist_tx_records` must complete before `track_submitted`. The
    core emits them in that order; do not make the write asynchronous.

---

## Deliberately not in this cut

Split, sweep, the payroll importer (054). The scanner, and therefore
`add_network`, which has no other entry (055). `simulate_calls`, answered
`null` (055). dApp signing (053). `sd1b`, `sd2b`, `sd2c`, `sd2d`, `sd3b`,
`sd3c` and `s1` keep their fixtures and their gallery routes.
