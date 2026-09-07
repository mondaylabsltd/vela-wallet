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

# 2. The live suites (real endpoints). Behind a compile flag so a flaky network
#    never fails an unrelated change. Three now: the add-network wizard, the
#    price path (feeds, the fiat waterfall, the golden Safe priced), and the
#    receipt scan (real ERC-20 receipts on Ethereum, decoded and persisted).
xcodebuild ... -only-testing:VelaWalletTests/NetworkAdminLiveTests \
  -only-testing:VelaWalletTests/PriceLiveTests \
  -only-testing:VelaWalletTests/ActivityLiveTests \
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
# Against the MERGE BASE, not origin/main: main has moved under this branch and
# now carries somebody else's send.rs work, which a plain diff would blame here.
git diff --stat $(git merge-base origin/main HEAD) -- rust/crates/vela-core/src/app/   # expect empty
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

### Phase 2 — `balance_dashboard` ✅ committed `f2b518e1` · `f0c58f76` · phase 2c

The screen this cut exists to fix. `$1,383.28` of somebody else's money is gone:
the home reads the seeded address's own holdings, priced, in the currency the
person chose.

- **2a** — the ABI gap closed in Rust (`multicall3_encode_aggregate3`,
  `multicall3_decode_aggregate3`, `erc20_encode_balance_of`), `TokenReads`, and
  `BalanceWire`. Bindings 215,916 → 260,699.
- **2b** — `BalanceExecutor`'s seven operations, `WalletLive`, `WalletStore`,
  `RootView`'s `.wallet` case, and `VELA_ACCOUNT`'s key-less seed. Two defects
  the live screen found: every unpriceable holding rendered twice, and the dev
  seed persisted past its own pin.
- **2c** — the price path. `choose_native_price` exported through the bridge
  (bindings → 263,535) rather than a fourth re-decision of the ladder; the
  Chainlink rungs (mainnet batch + per-chain feed) filling `price_usd`; the fiat
  waterfall behind `resolve_rate`; `fetch_fiat_rates` live; and the hero
  converting into the chosen currency or saying USD.

Two facts about the world, recorded in results.md and **needing a founder
decision** rather than a code change:

- the mainnet **BNB/USD feed is dead** (`0x14e613AC…75d25` answers `0x`) —
  web carries the same dead entry; BSC's local feed covers BNB today;
- the fiat endpoint quotes **30 currencies, not "~160 incl. VND"**, so **VND is
  the one code in the picker's eight that nothing can price**. It degrades
  honestly to USD. Drop it from `CurrencyCatalog`, or fix the `vela-currency`
  deployment.

**Still owed for 2c**: the device acceptance run. `testHomeShowsRealMoneyRather
ThanTheFixtureTotal` passes on the simulator and the phone dropped off USB
mid-run — a simulator run is preparation, never proof (SC-008).

---

### Phase 3 — `activity_feed` + `token_trust` ✅

Shipped together, and they had to be: `ScanIncomingTransfers` is routed through
`token_trust` on every client, so an `activity_feed` without it would render a
local store nothing has ever written.

- `Core/TxRecords.swift` (`vela.transactionHistory`), `Core/TokenMetadata.swift`
  (symbol + decimals over Multicall3), `Core/ChainTokens.swift` (the registry's
  stables = the scan allowlist), `Core/HeldTokens.swift` (web's `fetchTokens`
  cache, made explicit), and the two machines' executors, stores and wires.
- Proven live: **108 real USDT receipts** in the last 100 Ethereum blocks,
  `1069120000` raw → `"1069.12"` stored, a rescan of the same window answering
  **0 new**.
- The one bug: `pool.call(kind: "logs")` — `kind` is the endpoint *class*
  (`rpc` / `bundler`), and the core rejected the event outright. Only a live
  test could find it.

**Read the feed's honest shape in results.md before promising anybody a
history**: the scan is 100 blocks, native transfers emit no log, and the local
store is the source of truth. A fresh install shows an empty feed for a funded
account, on every client.

---

## Next — Phase 4: `manage_tokens`

The token list surface. `token_trust`, phase 4's other half, landed with phase 3.

- Read `rust/crates/vela-core/src/app/manage_tokens.rs` first.
- The metadata resolver it needs already exists (`Core/TokenMetadata.swift`), and
  so does the `vela.customTokens` writer (`TokenTrustExecutor.write`).

---

## Then

| Phase | What |
|---|---|
| 5 | `receive_watch` + `payment_request` — the deposit watcher and the ack |
| 6 | Flip 050's remaining `// live in 051` arms: `contacts::resolve_name` (the name-service waterfall — the activity feed's alias resolver wants it too) and `contacts::check_is_contract` (`eth_getCode` through the pool). `contacts::load_send_history` waits for 052 |
| 7 | Device acceptance + closeout |

### Wired but unreachable — the list to close before 7

The core owns these and no gesture reaches them:

- **tap-to-hide** (`WalletStore.togglePrivacy`) and **pull-to-refresh**
  (`WalletStore.refresh(pull:)`) — no call site at all;
- the **chain-filter pill** (`ActivityStore.chainFilter`) — still a fixture;
- the **receipt toast and row glow** (`FeedView.toast` / `newItemId`) — decoded,
  never drawn;
- **swipe-to-delete** an activity row (`ActivityStore.deleteRequested`).

### Deferred out of phase 2, on purpose

The **DEX quote rung** of the native price ladder — it needs per-chain master
data (`fetchChainTokens`: router, wrapped native, the chain's stables) and its
own dynamic-ABI encoders. `dex` is passed as `nil` today, which the core reads
as "no quote", and `best_native_dex_price` is left unexported until something
calls it. ERC-20 prices wait on the same work.

---

## The seeded read account (research D3) ✅ built in phase 2b

`VELA_ACCOUNT=<address>` seeds `vela.accounts` with a **key-less** record so the
read path has an address. Legitimate here and not in 052: reading needs an
address, signing needs a key, and a record with an empty `public_key_hex` can
fund a read and can never produce a signature.

Use `0x88cCA0EeDbF2C4426110bbFc998F048689266894` — the golden multi-key Safe,
whose Gnosis balance is checkable with one `eth_getBalance`.

**FR-010 exists so this is never inherited as a way to skip a ceremony.**
