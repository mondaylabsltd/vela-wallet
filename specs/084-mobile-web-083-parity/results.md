# Results: 084 — the 083 findings, checked on Android, iOS and web

**Status**: check pass done 2026-09-29. **Nothing fixed, nothing pushed, no money spent by this pass.** Branch
`084-mobile-web-083-parity` (from `083-windows-dapp-browser-stability`; code under test `c0694047` + the hand-off
commits, `git diff c0694047 HEAD -- rust` is empty). Source plan: `specs/083-windows-dapp-browser-stability/handoff-android-ios-web.md`.
Per-platform detail (verbatim on-screen text, steps, evidence names): `results-android.md`, `results-ios.md`,
`results-web.md`. Evidence: `evidence/{android,ios,web}/`.

**The short answer**: the hand-off's predictions held. Every P0/P1 finding that could be tested without a chain
send reproduces on all three clients. The 083 core fixes reach a client only once its core is rebuilt and its shell
draws the new fields, and none of the three shells does yet.

## How each client was checked

| Client | Where | Core that ran (proof) |
|---|---|---|
| Android | Xiaomi M2012K11AC (Android 13, MIUI V816, WebView 153) for everything; an API 34 emulator (WebView 113) for A-W1 only. Debug build installed over the top (`adb install -r`) | **Post-083**: `The transaction was included but reverted` found in all 3 ABIs of the built APK (1/1/1); the APK on the phone before had 0. JVM suite 802 tests, 1 red = the expected `CoreWireDriftTest.signRequestWiresMatchTheMirrors` |
| iOS | iPhone 11 "ABC" (iOS 26.5.2) for the browser and sheets; iPhone 17 simulator (iOS 26.2) for the hermetic suite and probes. Debug build installed over the top | **Post-083 after a rebuild**: it was stale on arrival (`check-ios-core-fresh.sh` exit 1); after `build-ios-xcframework.sh` it exits 0 (`1b72fc93…`); 083-only strings present in the device binary. XCTest 931 tests, 928 pass, 3 skipped, **0 fail** |
| Web + extension | Chrome for Testing 151 with the real side panel, driven over CDP; scratch profiles | Run twice. **Committed core** `55d58f87…` (pre-083) and **rebuilt** `3b09e97a…`. `build-web.mjs --check` exits 1 before the rebuild, "is current" after. Behavioural proof: same fabricated neighbour receipt fails the landing on the committed core and confirms it on the rebuilt one |

Money: none of the three sessions slid a real transaction. Android and iOS closed every sheet with ✕. The web pass
ran every signature against a synthetic relay (a network-layer guard answered every write method; proven before each
slide). One disclosed slip: the web pass slid an uncapped 2^254 Approve once against that synthetic relay; it never
left the page.

## The matrix (observed)

**P** = the hand-off's BAD was seen. **✓** = handled, confirmed. **A** = absent (the hand-off was wrong).
**Hm** = held, needs money. **Ho** = held, needs the owner (fingerprint, Face ID, second phone). **Ht** = held, needs a person's tap
(no driver). **n.a.** = cannot occur on this client. Detail on how each was observed: the per-platform files.

| ID | Finding | Android | iOS | Web |
|---|---|---|---|---|
| **S3b** | Another operation's hash answered to the dApp | **P** (JVM; device race Hm) | **P** (XCTest; returned the marker `0x1111…`) | **P** (page got the other op's tx hash in 3 s; landing "Confirmed") |
| **S2** | A reverted op answered as success | **P** (JVM; record closes confirmed) | **P** (`success:false` → `succeeded`) | **P** in part: a revert gives -32603 "try again with a higher gas price"; an in-op ExecutionFailure tells the page success while the record says Failed |
| **U1** | Max USDC→ETH drains the fee coin | Hm (precondition: USDC ≥ 0.2) | **P** in code + core tests (no `balance_changes_measured` in `app-ios`); sheet Hm | Hm (control only; sheet has no balances block, W-2 **P**) |
| **S3** | userOpHash answered after a timeout | **P** | **P** | **P** (+121.6 s; `eth_getTransactionReceipt(opHash)` → null) |
| **R3** | A revert reads "funds are safe" | **P** | **P** | **P** (no tx hash, no explorer button; the popup window never draws it) |
| **U1b** | A relay refusal drawn as "check your network" | negative control ✓; BAD half Hm | **P** (and `would_fail` draws no sentence at all) | **P** (committed = rebuilt) |
| **U1c** | Fee coin list unusable over a failed quote | Hm (Gnosis has one fee coin) | **P** (model + code; the tap Ht) | **P** (Gnosis and Base) |
| **H2** | A dApp transaction not in 活动 | **P** (core sends a `dapp` object; the Kotlin `FeedItem` drops it) | **P** (same, `FeedItemWire` has no `dapp`) | **P** (committed: "No activity yet"; rebuilt: rows appear as "Contract interaction · localhost", site without its port) |
| **REC** | Page-written to/value stored | **P** (`−1208925.8 XDAI 至 0x…dEaD`; `hasSentTo` true) | **P** | ✓ |
| **D1b** | Dismissed sheet + cancelled passkey | Ho | core ✓, live Ho | Ho |
| **R2** | A bundle neighbour fails our op | core ✓ | core ✓ | **P** on committed, fixed on rebuilt |
| **U8** | Fee ≈ 14× on-chain cost | sheet: `~0.00003 ETH`, speed 超快; ratio Hm | not measured (Hm) | tiers Fast 0.000029 / Std 0.000019 / Slow 0.000015 ETH; ratio Hm |
| **W20** | iPhone caBLE upper-case tunnel id | Ho | core ✓ (URL upper-case); two-iPhone run Ho | n.a. |
| **W4** | Certificate error / insecure origin | **P** (lock beside expired cert; retry silently loads the previous site) | ✓ | **P** (consent, sheet and signature work on http and past a cert error) |
| **W5** | Crash / reload leaves a live request | ✓ | ✓ (engine level) | **P** (F5, crash and tab close leave the stale sheet) |
| **F1** | Row says nothing about what it moved | **P** | **P** | **P** |
| **F3** | Called contract labelled 接收方 | **P** | **P** | **P** |
| **H4a** | Failed MESSAGE signature shows the transaction sentence | **P** at UI level; slide Hm | **P** (sheet is slide-enabled; free trigger works) | **P** by code; live Ho |
| **H4b** | Passkey failure → -32603 with raw text | Ho | Ho | Ho |
| **B5792** | wallet_sendCalls answer / capabilities | 4200 as predicted; Bundle Hm | 4200 as predicted; Bundle Hm | **P** (answered at once with the op hash, even when the batch reverts) |
| **W13** | A dApp read waits a full timeout per silent node | **P**, worse: ≈ 24.5 s (3 × 8 s) | **P** (8.5 s, then 0.3 s; sim) | partly: first read 20.3 s, then 10 s, 1.9 s |
| **H1** | Sign-in copy | **P** | **P** | **P** (names Windows Hello on a Mac) |
| **W11** | Progress label around the biometric prompt | Ho | partial; Ho | Ho |
| **W10** | Plain transfer drawn as a contract call | ✓ | ✓ | ✓ |
| **W1** | Engine or core cannot start, nothing says so | **P** (emulator: the app dies, `BrowserController.kt:399`) | n.a. | **P** (blank panel; 4900 at +301 s) |
| **W9** | Demo host in a live session | A: part A absent; part B **P** (crashed tab's menu names app.uniswap.org) | ✓ | n.a. |
| **W3** | Engine error page / no recovery | (a) ✓ (b) **P** (the engine's own page is visible ≈ 11 s) (c) inconclusive | ✓ classes; proxy cases Ho | n.a. |
| **W2** | Address bar during a slow load | **A**: follows the new host at once; only the closed lock before commit remains | partial (host at once, old title); typing Ht | n.a. |
| **H8** | Failed load keeps the previous site | ✓ + residue (a favourite saved from a failed tab keeps the old title) | **P** | n.a. |
| **W6** | window.open / target=_blank | ✓ | **P** as "today": a tapped one replaces the dApp in the same tab | n.a. |
| **W7** | External schemes / downloads | ✓ (+ orange hairline stays after a download link) | **P**: schemes leave the app with no Vela prompt; a download gives a raw view or blank, no panel | n.a. |
| **W14** | Connect consent names account and network | ✓ | ✓ | **P** |
| **W19 / H5** | Phone-held key: QR, Cancel, handshake | Ho | Ho | n.a. |
| **EXE** | Headline is the English "Execute" | **P** | **P** | **P** |
| **F2** | Hash runs off the detail panel | ✓ | ✓ (model) | ✓ |
| **DNS** | Lookup failures | ✓ | ✓ | n.a. |
| **D1** | Accidental dismiss | ✓ (Android 13, 3-button; predictive back not reachable) | ✓ swipe never answers; ✕ and background Ht | ✓ for gestures and the window; **panel close: nothing for 295 s, then 4900** |
| **W15** | Back stays within its tab | ✓ | Ht | n.a. |

### "Also noticed" (§10), observed

| Group | Android | iOS | Web |
|---|---|---|---|
| Stray `calls` key sets the headline while the top-level call is signed (**security**) | **A-1 P** | **I-1 P** | — |
| Multi-leg batch whose first leg is a plain transfer drawn as one 发送 | **A-2 P** | **I-2 P** | — |
| `input` calldata dropped, drawn as a bare send | **A-3 P** | **I-3 P** | — |
| JSON-number `value` read as 0 | — | **I-4 P** | — |
| Detail status from the hash, not the record | **A-4 P** | I-6 not measured | — |
| Receipt lookup of an op hash → the bundle's tx | **A-6 P** | **I-8 P** | **W-5 P** (returns null) |
| Pending row shows the record id as the hash | **A-7 P**; A-8 (bare `−`) **P** | — | — |
| Script leaves the app with no tap | — | **I-10 P on the phone**: a script's link click launched the App Store, then Mail | — |
| Cross-origin iframe navigates the whole tab with one tap | — | **I-11 P** | — |
| The comment "the system still asks" is wrong | — | **I-13 P** | — |
| Black-holed site shows no panel for a minute | — | **I-14 P** (60.5 s) | — |
| http:// site blocked with a network sentence | **A-9 P** | — | — |
| Non-http(s) subframe navigations cancelled | **A-13 P** (data:/blob: iframe renders blank) | — | — |
| Crashed tab's menus are the Uniswap fixtures | **A-12 P** | — | — |
| Side panel serves one tab; a second tab waits 300 s | — | — | **W-10 P** (4900 at 300.2 s) |
| Web builds with a pre-083 core | — | (rebuilt, see above) | **W-1 P** |
| Web signing sheet shows no balance changes; records carry `intent:null` | — | — | **W-2, W-3 P** |
| Popup-window request closes before any outcome shows | — | — | **W-8 P** |
| Extension reads ignore the person's RPC settings | — | — | **W-11 P** (catalog dump and code) |
| `transport_dropped` never dispatched | — | — | **W-12 P** (no dispatch in `src`) |
| Wire drift test red | **A-5 P** (the one JVM red) | did **not** fail after the rebuild | — |

## Where the hand-off was wrong, and what it did not list

1. **A gate the hand-off does not list**: `pnpm check` is red on this branch even after `pnpm sync:wasm` — one svelte-check error at `app-web/vela-wallet/src/routes/[locale]/wallet/+page.svelte:654` (`FeeFailure` now includes `"would_fail"` from 380d9014/5825e443; the send page's `SendEstimateFailure` was not extended). Rebuilt or not, it fails identically.
2. `cargo test -p vela-core --test app_tx_tracker` runs **0 tests** without `--features crux` — a false green (found on all three platforms). With it: 26 pass; the R2 rule tests are `--features crux --lib tx_tracker`.
3. The iOS drift tests `CoreWireDriftTests` and `BrowserWireDriftTests` **pass** after the rebuild (the hand-off said they might not); `vela_dev_fixtures.swift` did not change.
4. §7.1 "No existing test drives a signed submit end to end" — `DappSignMachineTest.kt` (in `src/testDebug`) does, and pins the S3 op-hash answer as intended.
5. Android needs **JDK 21** (`gradle/gradle-daemon-jvm.properties`), not 17.
6. Predictions overturned by the device: **A-W2** (the bar follows the new host at once), **A-W9 part A** (the bar is empty, not the fixture host), **A-W3(b)** (the engine's own page shows for ≈ 11 s), **A-W13** (worse: 3 × 8 s), **A-W4(c)** (retry loads the previous site), **A-H1 step 2** (a Bluetooth permission dialog comes before any QR), **iOS I-W7 step 5** (no failure panel for a download; WebKit reports none), **web W-H2** (the site is drawn without its port), **web W-W13** (only the first read costs 20 s), "a new site starts on Ethereum" (not seen on Android: sites keep their last chain).
7. §9.1 says the side panel cannot be driven headless. It can (Chrome for Testing 151 over CDP); the e2e test `extension-live-provider.e2e.ts:317` is stale (looks for `/request.html`).
8. Chrome ≥ ~140 upgrades or blocks public http pages: the W-W4 check needs `--disable-features=HttpsUpgrades,…` and httpforever.com, not neverssl.com.

## Found beyond the hand-off

- **A revoke reads "无限额"** (Android A-1 oddity, iOS extra): an `approve(spender, 0)` sheet shows the allowance editor as "无限额" in red and holds the slide shut until a chip is chosen. Probably the guard treating 0 as "no finite cap".
- `pnpm build` warns `IMPORT_IS_UNDEFINED: verifiedNameStep` (`services/recipient-identity.ts:152`, a file this branch does not touch).
- The existing iOS UI test `testTheDappSheetOffersASpeedAndNeverSignsOneItLeft` is stale (asserts the pre-079 swipe rule); `BrowserAcceptanceTests` is in the scheme's skip list, so `-only-testing` runs it as 0 tests.

## Held items — one plan for the owner (nothing has been sent)

Balances read on chain 2026-09-29 (MultiTest `0x88cC…6894`): **Base 0.000440 ETH and 0.034929 USDC; Gnosis 0.12867 xDAI**
(the hand-off said 0.184; see "An unexplained op" below). The account is shared by all clients: every step below runs
one at a time, each op reaching 已确认 before the next.

**Recommended (Tier A)** — proves what cannot be proven without a chain:

| # | Where | Step | Spend | Unlocks |
|---|---|---|---|---|
| 1 | Base, Android | one ETH→USDC 0.0001 swap in Uniswap (this is A-U8's swap and the top-up) | 0.0001 ETH + ≈ 0.00003 ETH fee → +≈ 0.266 USDC | A-U8 ratio, A-H2/F1/F3 fresh rows |
| 2 | Base, all three | **right after 1**: the Max USDC→ETH **sheet only** (✕, no slide) | 0 | U1, U1b, U1c on all three (needs USDC ≥ 0.2, true only between 1 and 3) |
| 3 | Base, iOS | one 0.05 USDC→ETH swap | ≈ 0.135 USDC (fee in USDC ≈ 0.085) | I-H2/F1/F3/U8/R2 rows |
| 4 | Base, Web | one 0.05 USDC→ETH swap | ≈ 0.135 USDC | W-H2/F1/F3/U8 on chain |
| 5 | Gnosis, Android | one Send dust with the relay cut after accept | 0.001 + ≈ 0.01 xDAI | A-S3, A-R3 and a fresh H2/F3 row on the device |
| 6 | Gnosis, Android | the REC `wallet_sendCalls` and the page's Bundle | 2 × ≈ 0.01 xDAI | A-REC, A-B5792 steps 2/4 |
| 7 | Gnosis, iOS | two `wallet_sendCalls` (one call, value 0), fee only | 2 × ≈ 0.01 xDAI | I-REC, I-B5792 |
| 8 | none (off-chain) | one `personal_sign` slide on Android and one on iOS, fixture key, no fee (lifts my "no slide" rule for message signatures only) | 0 | A-H4a, I-H4a failure sentence |

Totals if all of Tier A is approved: Base ≈ 0.00013 ETH and ≈ 0.27 USDC of USDC swap cost (start 0.035 + 0.266 ≥ 2 × 0.135);
Gnosis ≈ 0.05 xDAI. Steps 3 and 4 both need step 1's top-up, and they use it up (≈ 0.03 USDC left).

**Optional (Tier B)** — only for on-chain provenance; the S3b/S2/S3 BADs are already shown deterministically on all three clients:
the S3b race with two clients sending within ~2 s (2 × ≈ 0.011 xDAI); an S3 device run with Airplane Mode for 125 s (≈ 0.011 xDAI);
the S2 deadline-revert router call on Base (a few cents, racy); a **slid** Max USDC→ETH on iOS (≈ $0.08; it drains the USDC and only proves
something while USDC ≥ 0.2, so it would need its own top-up — the BAD is already visible on the sheet).

**Held for the owner in person (no money):**
- Face ID / fingerprint on your own account: D1b, H4b, W11 (all three), web W-H4a live.
- A second phone: W19, W20 (a live iPhone caBLE run), H5, Android A-11.
- Taps no driver could make: iOS W15, W2 typing/caret, D1 ✕ and background/foreground, I-12 lock, W5 panel, U1c tap, F2 on screen; the iOS fault-proxy cases (Settings ▸ Wi-Fi ▸ proxy).
- **Re-grant Bluetooth to Vela on the Android phone**: the A-H1 check raised the "nearby devices" dialog and the pass answered 拒绝, so QR/caBLE sign-in may now be refused until you allow it again in Settings.

## An unexplained op on the shared Safe

The web pass noticed a real op on MultiTest (Gnosis) at **10:37:20 (+08:00)**, nonce 61, tx `0xf7ffc22b…d675f`, and
the balance fell 0.13967 → 0.12867 xDAI (checked on chain: nonces 54–60 landed between 09:12 and 09:42, before this pass
started; nonce 61 is the only one after). It was not sent by this pass: Android's device log has no `relay.submit`
for 2026-09-29, the iOS pass slid nothing, and the web pass's guard blocked every write. The likeliest source is
the other session the iOS pass saw on the same iPhone at 10:34–10:40 (working tree `/Volumes/data/production/vela-wallet`,
running `DappBrowserStabilityProbeTests`, which has a slide helper that can send Send dust) — **unproven**.
Anyone sending from this account in parallel changes the nonce and balances under every check.

## Side effects left on real devices and the machine

- Android phone: the app is on your real account (parallel space off), `http_proxy` is `:0`, no adb reverse/forward left. The parallel-space profile
  kept 2 saved tabs and new "recent dApp" history; the Bluetooth dialog was answered 拒绝 once (above). One idle Gradle 9.5.0 daemon that was not
  this pass's was stopped by `./gradlew --stop`.
- iPhone: Debug build over the top, launched last with `VELA_PARALLEL_SPACE=0` (your real account). The I-10 check launched Mail and the App Store on the
  phone; both were killed. Mail may still hold an unsent compose draft to `test@example.com` (could not be seen). Another session was using the same
  phone during this pass.
- Machine: `corepack enable` was run once (pnpm shim now the pinned 10.11.1). All test servers and proxies started by this pass are stopped; older ones
  (8137/8138, 8899) belong to other sessions and were not touched.

## Test probes created (test source sets only; no production file touched)

- Android: `app-android/vela-wallet/app/src/test/java/app/getvela/wallet/Probe083ReceiptsTest.kt`, `Probe083ActivityTest.kt` (use the debug fixture
  bindings: they compile only for the debug variant; move them to `src/testDebug` for a release-variant run).
- iOS: `app-ios/VelaWallet/VelaWalletTests/Probes083ParityTests.swift`, `Probes083BrowserTests.swift`, `Probes083SheetTests.swift`.
- Web: scratch scripts only, in the session scratchpad (`web-probe/`, listed in `results-web.md`); none in the repository.

## Not committed on purpose

- Regenerated artifacts: `app-ios/VelaCoreKit/Sources/VelaCore/vela_core_uniffi.swift` (a doc comment and a checksum) is modified in the tree; the web
  `rust/pkg-web` and `assets/wasm` were restored. Commit them only as a deliberate part of a fix.
- `evidence/` (≈ 20 MB, 290 files, one 3.6 MB video) — the size is the owner's call.

## Next (waits for the owner)

Per the hand-off §12: choose which of the fix groups to open a spec for — (1) receipts family S3b/S2/S3/R3 (+ A-4/I-6, B5792's answer, A-6/I-8/W-5),
(2) fees U1/U1b/U1c, (3) Activity H2/F1/F3/REC (+ A-7, A-8, W-3), (4) signing-sheet security (stray `calls`, single-leg, `input`, extension secure-origin rule),
(5) browser (Android W1/W9/W3/W4; iOS H8/W2/W6/W7 + I-10/I-11; web W5/W1/W14 + W-10/W-12), (6) copy H1/H4a/W11/H5, (7) waiting on the owner: W13/H6, U8.
Open owner decisions are in the hand-off §11.2–§11.3 (target=_blank behaviour, external schemes, extension secure origins, EIP-5792, Android cleartext,
the retry-after-signing nonce question).
