# Tasks — 051 iOS Read Wiring

**Read this first if you are picking the branch up cold.** It is written to be
enough on its own: what is done, what is next, and the exact commands.

Branch `051-ios-read-wiring`, stacked on `050-ios-live-shell`, in the worktree
`/Volumes/data/production/vela-wallet-ios`.

---

## The five commands

```bash
# 0. A fresh checkout cannot build until this has run once (Artifacts/ is gitignored).
#    Re-run it after ANY change to rust/crates/vela-core-uniffi/.
rust/scripts/build-ios-xcframework.sh                       # ~5 min cold

cd app-ios/VelaWallet                                       # xcodebuild needs this cwd

# 1. Build + hermetic tests. iPhone 15 Pro is on iOS 17.5, so `OS:latest` does
#    NOT match it — address the simulator by id.
xcodebuild -project VelaWallet.xcodeproj -scheme VelaWallet \
  -destination 'id=84146B7B-C679-46AC-8426-41AA42E6F403' test

# 2. The live suite (real endpoints). Behind a compile flag so a flaky network
#    never fails an unrelated change.
xcodebuild ... -only-testing:VelaWalletTests/NetworkAdminLiveTests \
  OTHER_SWIFT_FLAGS='$(inherited) -DVELA_LIVE_TESTS'

# 3. On the founder's iPhone. Screenshots come back inside the .xcresult.
xcodebuild ... -destination 'platform=iOS,id=00008130-001C68C804E1401C' \
  -only-testing:VelaWalletUITests/LiveWiringAcceptanceTests \
  -resultBundlePath /tmp/accept.xcresult
xcrun xcresulttool export attachments --path /tmp/accept.xcresult \
  --output-path /tmp/accept-images

# 4. Gates. The literal audit is NOT green on main — 35 pre-existing violations.
#    The gate is "no new ones".
node app-ios/scripts/audit-literals.mjs                     # expect 35
git diff --stat origin/main -- rust/crates/vela-core/src/app/   # expect empty
```

---

## Done

### Phase 0 — baselines
203 tests · device Debug dylib 57,346,720 bytes · bindings 215,916 bytes ·
five `// live in 051` markers inventoried.

### Phase 1 — `rpc_pool`, the routing authority ✅ committed `870a86bd`
- Seven `bridge_object!` exports (all of this cut's machines, at once).
- `Core/ChainCatalog.swift` — twelve chains + provider slugs + public RPCs.
- `Core/RpcEndpoints.swift` — the collector, the ban map's storage shape.
- `Core/RpcPool.swift` — the `async` facade over `CoreStore`, `call_id`
  correlated. **The only new plumbing shape in this cut.**
- `CoreHTTP.rpcEnvelope` — a classified reply (the pool bans on the difference).
- Proven live: the golden Safe's Gnosis balance, `0xa8867319d2da000`.

---

## Next — Phase 2: `balance_dashboard`

**The screen this whole cut exists to fix.** `WalletScreen` renders
`WalletFixtures.buildMobileState(.h1)` — `$1,383.28` of somebody else's money
under the person's real name and identicon (`App/RootView.swift`,
`signedInOrWelcome`).

### The seven operations

`FetchTokens` · `FetchAccountAssets` · `ReadBalanceCache` ·
`ReadBalanceCacheMany` · `WriteBalanceCache` · `StartRetryTimer` ·
`WritePrivacy`

`FetchTokens { address, force, pull }` **names no chain and no URL** — the core
delegates the whole multi-chain fetch and rules only on what comes back. That is
the shape 050 never had, and it is why this phase ports a service layer.

### What to build, in order

1. **`Features/Wallet/BalanceWire.swift`** — `Decodable` mirrors of
   `BalanceDashboardView`. Read the Rust first:
   `rust/crates/vela-core/src/app/balance_dashboard.rs` (1,096 lines).
2. **`Core/TokenReads.swift`** — the multi-chain fetch, ported from
   `app-web/vela-wallet/src/lib/services/wallet-api.ts` (788) and
   `token-reads.ts` (86). Native balance is `eth_getBalance`; ERC-20 balances
   are **Multicall3 `aggregate3`** — see the gap below.
3. **`Features/Wallet/BalanceExecutor.swift`** — the seven operations, every
   chain read going **through `RpcPool.call`** and never through `CoreHTTP`
   directly (FR-002: one pool, one ban map).
4. **`Features/Wallet/WalletLive.swift`** — sibling of `WalletFixtures`,
   producing the same display models.
5. **`Features/Wallet/WalletStore.swift`** — resident, constructed in
   `RootView.init` beside `contacts` and `settings`.
6. Wire `signedInOrWelcome`'s `.wallet` case to it.

### The gap that needs deciding first (research D1)

`aggregate3((address,bool,bytes)[])` is a dynamic array of tuples and the bridge
exports **no encoder for it**. `abi_encode_address/uint256/bytes32`,
`function_selector` and `decode_calldata` exist and are not enough.

D1's decision: a Rust encoder in **`vela-core-uniffi`** (add `alloy-dyn-abi` as
a direct dep of that crate), exported through the bridge. **Not** in
`vela-core`, because that would force a `rust/pkg-web` rebuild and commit while
another session holds the web client — a scheduling reason, recorded as a
consolidation debt rather than dressed up as a design.

Measure the bridge growth when it lands: the `.xcframework` and the committed
`vela_core_uniffi.swift` both grow, and 019 recorded that linking crux cost
+785,864 stripped bytes on Android. Baselines are in Phase 0 above.

### Two traps this phase will hit

- **`BalanceToken.balance` is a HUMAN DECIMAL, not raw units.** Desktop's 031
  wrote it the other way and the defect was invisible while prices were `None` —
  with a price, the total would have been out by 10^18.
- **Tempo (4217) has no native coin** and its RPC returns the same ~4.24e75
  constant for every address; its symbol is `USD`, so a stablecoin peg would
  price the garbage at $1 and put ~4e57 dollars into the total. The guard is
  the core's own `fee_policy::TEMPO_CHAIN_IDS`, never an invented "too big"
  threshold. `ChainCatalog` already marks it `gasModel: .tempo`.

---

## Then

| Phase | What |
|---|---|
| 3 | `activity_feed` — real transfers grouped by day; `ScanIncomingTransfers` uses `eth_getLogs` through the pool, and the range-cap verdict is why `RpcOutcome.rangeCap` exists |
| 4 | `manage_tokens` + `token_trust` — the token list and the security verdict. **`token_trust` is the one machine here whose job is security**: a token that arrived by transfer is untrusted until the core says otherwise, and a shell that pre-filters has made the decision the core was written to make |
| 5 | `receive_watch` + `payment_request` — the deposit watcher and the ack |
| 6 | Flip 050's four `// live in 051` arms; `contacts::load_send_history` is the only one left, and it waits for 052 |
| 7 | Device acceptance + closeout |

---

## The seeded read account (research D3), still to build

`VELA_ACCOUNT=<address>` seeds `vela.accounts` with a **key-less** record so the
read path has an address. Legitimate here and not in 052: reading needs an
address, signing needs a key, and a record with an empty `public_key_hex` can
fund a read and can never produce a signature.

Use `0x88cCA0EeDbF2C4426110bbFc998F048689266894` — the golden multi-key Safe,
whose Gnosis balance is checkable with one `eth_getBalance`.

**FR-010 exists so this is never inherited as a way to skip a ceremony.**
