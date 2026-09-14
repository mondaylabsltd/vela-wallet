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

## Next — Phase 1: the money plumbing, provable before anything can spend

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
