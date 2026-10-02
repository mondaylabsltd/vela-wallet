# 087 results — mobile beta device pass

Devices: **Xiaomi alioth** (Android 13, 1080×2400) and **iPhone 11** (iOS 26.5.2), plus iOS 18 / 26.2 and Android 11 / 14 emulators and simulators. The owner's own wallet was read only. Real sends (dust, Gnosis) used the parallel space's golden Safe. Every fix was re-checked on an integration build of `main` plus every 086/087/088/089 branch (local branch `087-integration`).

## What works end to end (both phones)

- **Home:** balance, activity, assets, Receive (QR decodes; save; explorer).
- **Send:** real 0.001 xDAI on Gnosis.
  - Android: about 5 s.
  - iPhone: submitted → confirm countdown → "已发送" with tx hash.
- **Scan → send:** photo picker on Android; pay links on iOS.
- **Contacts:** import, including the encoding cases.
- **Explore and the dApp browser:**
  - connect sheet, `personal_sign`, SafeTx refusal, unlimited-approval warning with cap editor, EIP-6963 announce, checksummed `accountsChanged`;
  - failure panel with retry on a blocked site.
- **Settings:** language, text size, theme, region formats, advanced, feedback preview, Sign Out, keys, backup sheet.
- **Trusted signer:** iOS round trip (#318) and the Android owner's route.

## Findings → status

| # | Sev | Finding | Status |
|---|---|---|---|
| F01/F02 | S2/S3 | "Touch ID or Windows Hello" on a Face ID iPhone; "create" in sign-in | **#354**, verified on the iPhone ("Face ID", "扫码") |
| F03 | S3 | iOS empty Assets blank | **#355** |
| F04/F05 | S1/S2 | dApp records "处理中" forever; record id shown as the hash | **#353**, verified on the Xiaomi (未知, no hash row) |
| F07/F11/F15 | S3 | Chainlist chip split; blank amount cell; a refused page called "unstable network" | **#358**, Chainlist verified |
| F08 | S2 | "N 个网络 RPC 不可用" permanent on Home in China (networks with zero holdings) | **Owner ruling 2026-10-02:** keep the notice for every unreachable network (holdings are unknowable while it is down); no jargon; tap shows all → **spec 092** (`092-unreachable-network-notice`) |
| F09/F13 | S3 | a11y: test ids spoken (slider); "扫描二维码" on a show-QR button | **#357**, F13 verified on the iPhone |
| F10 | S3 | hero total stale next to fresh rows until refresh | open (minor) |
| F12 | S2 | fee shown is limit × max bid (Arbitrum ≈ CN¥4.3 to deploy + send); default speed 超快 | **Owner ruling 2026-10-02:** the fee display and the 超快 default are kept |
| F14 | S3 | Android notification permission re-asked | **#356** |
| F16 | S3 | favourite labelled with the full page title | open (minor) |
| F17 | S3 | activity rows truncate at the largest text size | open |
| F18 | S3 | iOS/Android differences in Settings value previews | open (minor) |
| F19 | S3 | every dApp row titled "dApp 交易" | open |
| F20 | S3 | unnamed contact shown in lowercase | open (minor) |
| F21 | S2 | Android browser white screen (no bar or ×) during a slow first load | **#360** (root cause: unclipped WebView); emulator-verified |
| F22/F31 | S3 | test ids as a11y labels (iOS `send.amount`, `explore.searchField`; Android split rows) | open; same class as #357 |
| F23 | S3 | empty recipient shows an identicon | open (minor) |
| F24 | S3 | feedback "无法连接的 RPC" lists a chain id ("1625") | open (minor) |
| F25 | S3 | approval sheet abbreviates the spender two ways | open (minor) |
| F26/F27 | S2 | iOS: leaving Send left the journey alive, so the next 转账 resumed it with a stale scanned recipient and scope | **#375**, verified on the iPhone |
| F28 | S2 | iOS decimal keypad can't be dismissed; covers 继续; balance truncates | **#376** (+ **#377** on top of #348), verified on the iPhone |
| F29 | S3 | iOS pending receipt says "UserOp 哈希"; hash copy button labelled "复制地址" | open |
| F30 | S3 | Android splash icon is a hard-edged square | open (088 #352 already replaced the robot) |
| F32 | S2 | backing up public keys on Ethereum mainnet quoted ≈ CN¥68 | **Owner ruling 2026-10-02:** the Ethereum backup cost is accepted |
| F33 | S1 | iOS 17: attaching a screenshot to a bug report crashed the app; 21 stored async closures had the same hazard, reachable by reflection | **fix/ios17-runtime-crash**: each function type states its isolation; a binary check in CI, the package workflow and the owner checklist. `VelaWalletTests` pass on iOS 17.5 and 26.2 (below) |

## F33 — the iOS 17 runtime crash

**Symptom.** On an iOS 17.5 simulator, `FeedbackScreenshotTests`, `ScreenshotViewerTests`, `BugReportTests` and `SettingsFeedbackRowTests` crashed the test host: `EXC_BAD_ACCESS (KERN_INVALID_ADDRESS at 0x0)`, in "type metadata accessor for nonisolated(nonsending) () async -> Data?", called from `FeedbackSender.add(datas:)`. This was reproduced on `main` @ 7392b9b61. The same four suites pass on iOS 18.0 (62 tests) and on 26.2. In the app, the photo picker builds the same `[() async -> Data?]`, so every iOS 17.4–17.x user who attached a screenshot would have crashed.

**Root cause.**
- `SWIFT_APPROACHABLE_CONCURRENCY = YES` turns on `NonisolatedNonsendingByDefault`, so every async function type without an annotation is `nonisolated(nonsending)`.
- Swift 6.2.4 (Xcode 26.3, on this Mac and on the macos-15 runner) builds that type's metadata with `swift_getExtendedFunctionTypeMetadata` and links it **weakly with no fallback**.
- That entry point first shipped in the Swift 6.0 runtime. `nm` shows it is missing from the iOS 17.5 runtime's `libswiftCore` and present in 18.0 and 26.2. On iOS 17 the call jumps to NULL.
- The compiler rejects the two other users of this entry point below iOS 18, with "runtime support for typed throws / `@isolated(any)` function types is only available in iOS 18.0". It misses this third one. Upstream this is swiftlang/swift #85017 / #86468. The Swift Forums thread reports an IRGen fix in Swift 6.3 (Xcode 26.4), which neither this Mac nor the runner (newest: 26.3) has.
- Metadata is requested in two ways:
  - **Directly**, when the type is a generic argument (`Array`, `ArraySlice`, `map`). This is the loaders.
  - **Lazily, through a field descriptor**, when something reflects a stored property of that type: `Mirror`, `dump`, or SwiftUI's AttributeGraph walking a View's fields. The latter is the "segfault" that 051 boxed `RefreshAction` against without finding the cause.

**Fix: state the isolation.** `@MainActor` and `@concurrent` function types use ordinary function metadata, which every runtime since iOS 15 can build. The choice for each site keeps the behaviour it had under nonsending:
- `FeedbackSender.Loader = @concurrent () async -> Data?` (the crash). The loaders run in task-group children, so they were already off the main actor.
- `@concurrent @Sendable` for `RegistryClient.Transport` and `RegistryResolver.ethCall`. These are Sendable values called from an actor.
- `@MainActor` for every other stored port. Each one is called from main-actor code and wired to closures that forward to main-actor objects:

| File | Stored property |
|---|---|
| `BugReport.swift` | `Transport` (`FeedbackSender.transport`) |
| `BalanceExecutor.swift` | `chainFacts` |
| `BatchExecutor.swift` | `fiatRate` |
| `BrowserController.swift` | `Ports.poolCall`, `Ports.resolveUserOp` |
| `DbrExecutor.swift` | `Ports.poolCall`, `Ports.resolveUserOp` |
| `CoreDriver.swift` | `perform`. The pass-through parameters in `CoreStore`, `SettingsStore.networkPerform` and the test `NetworkAdminStub` are annotated too, so no new Sendable warning appears. |
| `FeeExecutor.swift` | `MeasureCall` |
| `HomeBalancePoller.swift` | `sleep` |
| `RegistryBackup.swift` | `ethCall` |
| `RegistryNameLookup.swift` | `ethCall`, `indexGet` |
| `RelayClient.swift` | `builtinBase` |
| `SignExecutor.swift` | `Ports.switchAccount` |
| `TrustedSignerChannel.swift` | `reachable` |
| `UserOpSpine.swift` | `measureCall` |
| `WalletKeys.swift` | `ethCall` |
| `WalletScreen.swift` | `RefreshAction.run` |

**Scan: the binary, not grep.** The scan is `nm` plus disassembly of every object file in the Debug build, looking for accessors whose mangling ends in `YCcMa`.
- Before the fix: 17 distinct nonsending function types in the app and 1 in the tests. Only the loaders were reached by generic instantiation. The other 21 stored properties were reached by field descriptor only.
- After the fix: 0 accessors in 724 Mach-O files, products and intermediates.
- Ruled out by the same scan: async function types that are only passed or returned and never stored or used as a generic argument, such as `RelayClient`'s `run:`, `UserOpSpine`'s `writeAhead:` and the test timing helpers. Their layout is fixed, so no metadata is requested.
- The other weak Swift-runtime imports in the binary have back-deployment shims, such as `swift_task_deinitOnExecutor` behind `…MainActorBackDeploy`. The full suite on 17.5 is green.

**Why not the build flag.** `SWIFT_UPCOMING_FEATURE_NONISOLATED_NONSENDING_BY_DEFAULT = NO` would remove the implicit types in one line. It was not chosen because it also changes where every nonisolated async function and every stored async closure runs, across the whole module. That is a semantic change made the day before TestFlight to dodge an IRGen bug that 6.3 fixes. It would also still miss an explicit `nonisolated(nonsending)`. The explicit annotations are behaviour-preserving, and the guard below covers both routes.

**Guard.** `app-ios/scripts/check-ios17-function-metadata.mjs` fails when any built Mach-O imports `swift_getExtendedFunctionTypeMetadata` and names the function types to annotate. `--self-test` compiles a bad and a fixed snippet for iOS 17.4 and proves the check bites on the current toolchain. It runs in three places:
- CI, after the iOS tests, with `-derivedDataPath`.
- `ios-package.yml`, on the Release archive.
- Step 3 of the 088 owner checklist, on the local archive.

**Why there is no iOS 17 job in CI.** The macos-15 image (20260907) carries iOS 18.5, 18.6 and 26.x simulator runtimes, and no iOS 17 one. iOS 18 has the entry point, so it would not catch this. Downloading 17.5 costs 7.3 GB plus a second full run per push, on minutes billed at ten times Linux. The symbol check covers every code path for seconds. Before a TestFlight build, still run `VelaWalletTests` on an iOS 17.5 simulator locally.

**Tests.** Full `VelaWalletTests`, Swift Testing, on the fixed tree:
- iOS 17.5 (21F79): 1124 tests in 145 suites passed.
- iOS 26.2 (23C54): 1124 tests in 145 suites passed.
- The four feedback suites on 17.5 alone: 62 tests passed. Before the fix the host crashed twice, Xcode restarted it twice, and the run ended `TEST EXECUTE FAILED`.
- The check on a Release device build, the configuration that ships: the `main` build imports the entry point and the same 17 types. The fixed build is clean.

## Not covered by the device pass

- **Onboarding (create / sign-in) on a fresh Android install:** it would wipe the owner's phone. Copy is covered by #354; the iOS create round trip is covered by #318's test.
- **Final Face ID / fingerprint on the trusted-signer page:** needs a person. The page, the slide and the system passkey sheet were reached on the iPhone.
