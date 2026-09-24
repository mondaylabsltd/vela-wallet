# Results — 081 (in progress)

Branch `081-audit-product-gaps`, worktree `vela-wallet-081`, from `main` at `746e2259`.

## Baseline (T001)

Measured before any change, so "did I break it" has an answer:

| Gate | Baseline |
| --- | --- |
| `cargo test --workspace --features vela-core/i18n-all` | green (48 suites) |
| web wallet `pnpm test` | **6 failures, pre-existing**: 3 × `extension/package.test.ts` (needs `pnpm build:extension` first), `explore/fixtures.test.ts`, `i18n/messages.test.ts` (FLOW_KEYS scan), `tokens/tokens.test.ts` (a `36px` literal in `SigningHeader.svelte`) |
| desktop `cargo check` | 7 warnings, pre-existing |

## User Story 1 — a dApp cannot take over the wallet (FR-005/006) — **done, device run pending**

**The rule** (`rust/crates/vela-core/src/app/self_call_guard.rs`, new): 13 Safe control selectors refused when the target is the signing account; recursion through `multiSend` and `execTransaction` payloads (depth 4); **any inner `delegatecall`, whatever its target** — a dApp cannot express one through `{to, value, data}`, so crafted calldata carrying one is hostile; `SafeTx` typed data refused because it is a valid EIP-1271 authorisation if this account owns another Safe. Empty-data self-calls stay allowed: the in-band fee leg and the gas estimator use exactly that shape.

**Two hooks** in `sign_request.rs`: at arrival, and again at the submit chokepoint (a shell can hand back rewritten `params_override_json`, and those are the bytes that get signed).

**The decision the e2e forced.** Answering the dApp at arrival closed the window that was explaining the refusal — the extension worker closes a request window the moment the request settles, because that window *is* the answer surface. So the request now stays pending and unsignable until the person closes the sheet, and the dismissal answers `self_call_blocked`, not 4001: the wallet refused it, not the person. A sheet that refuses also shows no fee row and no slider — a dead "Slide to confirm · Enable module" under a refusal reads as an option someone merely failed to use.

**Verification**

| Check | Result |
| --- | --- |
| `tests/app_self_call_guard.rs` | 13 tests: every selector, batch leg (names the step), nested multiSend, delegatecall, SafeTx; negatives (empty self-call, view selector, ordinary call); the machine tests (refused before answering, answered on dismissal, unsignable even if the shell asks) |
| core suite | green, no regressions |
| i18n gate | `gen:i18n` (path pin 1687 → 1691, reason recorded), `lint:i18n` clean, `verify:i18n` 74045 comparisons zero divergences, `dump:vectors`, `build:wasm`, `gen:core-types` |
| web `pnpm check` | 0 errors, 0 warnings |
| web `pnpm test` | 3 pre-existing failures remain (see baseline); the `tokens` one no longer includes any file this work touched |
| web e2e `extension-signing` | **5 passed**, including the new blocked-request test end to end |
| desktop `cargo check` | 7 warnings, unchanged |
| Android `assembleDebug` + `testDebugUnitTest` | green (the strict `when` over error kinds caught the missing branch at compile time — exactly what that pattern is for) |
| iOS build | xcframework built; app build pending |
| Device run — Android (FR-019) | **done**, Xiaomi `9d5f42fb`, 2026-09-22: the dev test dApp (served over `adb reverse`, opened through `velawallet://open?url=`) asked for `enableModule` on the wallet's own address; the sheet showed "Vela can't sign this" and the full sentence naming `enableModule`, with no confirm control in the view tree. Nothing was signed — the account on that phone holds real funds, and the refusal path never reaches a signature |
| Device run — iPhone 11 (FR-019) | **pending** |

## FR-014 — the iPhone app declares what it uses (T083/T084/T085) — **done**

**The generator stopped shipping.** `rust/crates/vela-core-uniffi` enabled `uniffi/cli`, which put the whole bindings generator inside the library the iOS app links and the Android app loads. It is now a build-time-only crate, `rust/crates/vela-uniffi-bindgen`, and `vela-dev-fixtures-uniffi` lost its duplicate for the same reason. Nine callers swapped `-p vela-core-uniffi` for `-p vela-uniffi-bindgen`; `generate --library` reads the metadata out of the **built** library, so one generator serves both and depends on neither. It is in `members` but **not** `default-members`, because cargo unifies features across one invocation and a bare workspace build would otherwise fold `cli` straight back in.

Measured on `aarch64-apple-ios`, `libvela_core_uniffi.a`:

| | before | after |
| --- | --- | --- |
| size | 169,581,752 B | 122,825,640 B |
| objects | 554 | 516 |
| `uniffi_bindgen`, `uniffi_udl`, weedle, nom, clap, goblin, `cargo_metadata`, rustix, tempfile, askama, toml objects | present | **none** |

**The research's causal claim was wrong, and the measurement says so.** `nm -u` over the archived arm64 `.app` is **byte-identical** before and after the split (2,235 undefined symbols both times; the executable moved 19,362,880 → 19,355,640 B). `ld` was already dead-stripping the unreachable generator, so the App Store binary never imported anything because of it. `llvm-nm --print-file-name` attributes `_stat`/`_fstat`/`_lstat`/`_fstatat` to **Rust `std`'s own codegen unit** inside the static archive, which a Mach-O link pulls in whole. Research also listed `_fstatfs` (DiskSpace) and `_fstatat`; neither reaches the linked binary, so neither is declared. What the split does buy is real but narrower: 46 MB of build-time-only code and eleven crates of supply chain are no longer *inside* the artifact a wallet ships, held out of users' hands by nothing but a linker optimisation.

**What the archived device binary actually imports** (Release, whole-module, `nm -u`):

| Category | Symbols | Source |
| --- | --- | --- |
| UserDefaults | `_OBJC_CLASS_$_NSUserDefaults` | the app's own Swift (`VelaStore.swift:108`, `AccountStore.swift:27`) |
| FileTimestamp | `_lstat`, `_NSFileModificationDate` | Lottie (which ships its own manifest declaring FileTimestamp/C617.1) |
| FileTimestamp | `_stat`, `_fstat`, `_lstat` | Rust `std`, inside `libvela_core_uniffi.a` |
| — | none | no DiskSpace (`statfs`/`fstatfs`/`NSURLVolume*Capacity` absent), no SystemBootTime (`mach_absolute_time` absent — the one clock call is `clock_gettime`, not a required-reason API), no ActiveKeyboards |

**The manifest**: `app-ios/VelaWallet/VelaWallet/PrivacyInfo.xcprivacy` — `NSPrivacyTracking` false, empty `NSPrivacyTrackingDomains`, `UserDefaults`/`CA92.1`, `FileTimestamp`/`C617.1`, and one collected-data entry (`OtherFinancialInfo`, not linked, not tracking, AppFunctionality) for what getvela.app/privacy says the relay receives and keeps. **The folder-synchronised-group claim was verified, not trusted**: nothing was added to `project.pbxproj`, and the archive carries the file at the `.app` root, byte-identical to the committed source.

**The gate** (`.github/workflows/ios-package.yml`, after the archive step): bundle-root presence, `plutil -lint`, `NSPrivacyTracking` still false, then `nm -u` over the archived binary mapped to Apple's five detectable categories, failing on any the manifest does not declare. Run against the real archive it prints `ok: FileTimestamp` / `ok: UserDefaults`; with `FileTimestamp` deleted from a copy of the manifest it fails with the four symbols named.

**Verification**

| Check | Result |
| --- | --- |
| `cargo build --release -p vela-core-uniffi` (host + `aarch64-apple-ios` + `-ios-sim`) | green |
| bindgen, exactly as CI runs it | Swift output **byte-identical** to the committed `app-ios/VelaCoreKit/Sources/VelaCore/vela_core_uniffi.swift`; Kotlin and the dev-fixtures library generate too |
| `rust/scripts/smoke-swift.sh` | **47,383 conformance cases green** through bindings made by the split-out generator; golden multi-key Safe agrees |
| `rust/scripts/build-ios-xcframework.sh` | green, xcframework rebuilt |
| `rust/scripts/build-ios-dev-fixtures.sh` | green; the committed `Dev/vela_dev_fixtures.swift` came back byte-identical too |
| `xcodebuild archive` (device, Release, `-O -whole-module-optimization`) | **ARCHIVE SUCCEEDED**, Xcode 26.3 / Swift 6.2.4 |
| `xcodebuild build` (iPhone 16 simulator, Debug) | **BUILD SUCCEEDED**. Two traps for the next person: `name:iPhone 16` no longer resolves, because `OS:latest` on Xcode 26 is a runtime that has no iPhone 16 — pass the simulator's `id=`; and a fresh worktree fails Debug with `cannot find type 'RustBuffer'` until `rust/scripts/build-ios-dev-fixtures.sh` has run, since `app-ios/VelaDevFixturesKit/Artifacts/` is gitignored |
| privacy gate against that archive | passes; negative test (FileTimestamp removed) fails as intended |
| `plutil -lint` | OK |
| Device run — iPhone 11 (FR-019) | **pending** |

**Left for the founder**: `docs/store-submission/privacy-and-review.md` §1.B recommends answering **Linked: Yes** in App Store Connect for the same data the manifest declares as `Linked: false`. Both feed one nutrition label and the form is a legal attestation — the file and the doc now both flag it, but the answer is his.

## FR-009 — the network check asks for what a multi-key wallet needs (T040–T043) — **done**

**What was wrong, precisely.** `REQUIRED_CONTRACTS` asked for eleven contracts. One of them, Safe's `CompatibilityFallbackHandler`, a Vela account never uses — the account's fallback handler *is* the 4337 module (`safe.rs` setup calldata), so chains were turned away over a contract that would never be called. Two it did not ask for at all: `SafeWebAuthnSignerFactory` and its singleton, which every key after the first is a proxy of. A wallet's address is derived from **all** its keys, so on a chain without that factory a wallet made from two to seven keys cannot be deployed at all — and the screen said "Compatible".

**The shape of the fix.** `REQUIRED_CONTRACTS` is now twelve `(name, address, multi_key_only)` triples. `NetCompatibility` carries two verdicts instead of one: `compatible` (a one-key wallet works here) and `multi_key_ready` (and so does a wallet with two to seven keys). Both are true statements about the same chain, and the screens say both. The dead `safe::FALLBACK_HANDLER` constant is gone — a named constant for a contract the wallet does not use is how the wrong bar got written in the first place.

**Provenance of the two new addresses.** Not copied from a doc: the factory's creation transaction was read off Gnosis (block 36,371,460, `0xfa7c318b…`), its creation bytecode taken from that transaction, and `keccak256(0xff ‖ 0x914d7Fec… ‖ 0 ‖ keccak256(initCode))` recomputed to `0x1d31f259ee307358a26dfb23eb365939e8641195` — the address the wallet has been deriving addresses against all along. The singleton is `CREATE(factory, 1)`, confirmed with `cast compute-address`: the factory's constructor deploys it, which is why the setup page offers **one** step for the two of them.

**What each surface now says**

| Surface | Before | After |
| --- | --- | --- |
| core | one verdict | `compatible` + `multi_key_ready`, and `multi_key_only` per contract |
| getvela.app `/chain-setup` | eleven rows, "Vela works here" | twelve rows; the two multi-key ones name what they are for; the factory is a real deploy step (salt 0 through the Safe singleton factory, creation code in `deployment-data.json`), the singleton none, because its step does not exist |
| web / desktop / Android / iOS wallet | "Compatible" | "Compatible", plus one sentence when `multi_key_ready` is false; the check rows a one-key wallet depends on stay green |

**A decision made without the founder.** The wallet could go further and *refuse* to add a network that is not `multi_key_ready` when this wallet actually has more than one key — on such a chain that wallet's address cannot be deployed at all, so adding the network offers a deposit address that can never be spent from. It is not done here: the `network_admin` machine has no idea how many keys the signed-in wallet holds, and feeding it that fact means a new event and four shell changes. The warning is the honest minimum; the gate is a founder call, and it is written down rather than quietly skipped.

**Verification**

| Check | Result |
| --- | --- |
| `tests/app_network_admin.rs` | **71 pass**, including the two new ones: a chain answering for everything except the signer factory comes back `compatible && !multi_key_ready` and can name exactly which two are missing; and a guard that the CompatibilityFallbackHandler is not in the list while the factory is |
| core suite (`--features crux,i18n-all`) | **41 suites green**, no regressions |
| site `required-contracts.test.ts` | green — the drift test now parses the 3-tuple and asserts the multi-key flags match the Rust file, so the page cannot drift from the wallet in either direction |
| site `verdict.test.ts` | green, including the new case: only the passkey pair missing → `needs-setup`, two missing contracts, **one** runnable step |
| desktop `cargo check` / `cargo test settings::` | 7 warnings (baseline), 0 errors / **27 pass**, including a new one proving the multi-key pair does not redden the rows a one-key wallet needs |
| Android `:app:testDebugUnitTest` | green, including a new `SettingsLiveTest` case for the warning callout and its absence on a whole chain |
| iOS `SettingsLiveTests` | **blocked** on another session's in-flight `App/RootView.swift`; the tests are written (counts moved 8 → 7 with the fallback handler gone) and re-run is pending |
| i18n gate | one new key, `settingsModals.addNetwork.singleKeyOnly`, 15 locales; path pin 1691 → 1692 with the reason recorded; `lint:i18n` clean, `verify:i18n` 74,060 comparisons zero divergences |

## FR-008 — "verified" is now a word the wallet has earned (T030–T036) — **done, device runs pending**

A descriptor fetched over plain HTTP from a base URL the person can edit was labelled **verified** on the signing sheet. Nothing signed it, nothing checked it; the label meant only "a descriptor for this contract was found".

`ClearSignResult` now carries `ClearProvenance` — `BuiltIn`, `PinnedMatch`, `Fetched`, `Standard`, `SelectorDb`, `None` — and `verified` is **derived in `view()`** (`r.verified = r.provenance.is_verified()`) rather than set by whichever builder happened to run. The builders cannot grant the word by hand any more; they say where the words came from and the projection decides. `PinnedMatch` is a fetched file whose parsed JSON equals the built-in copy for the same target: in transit nothing changed it, so it is as good as built in.

The bool that used to be threaded through `try_calldata` / `try_eip712` became `DescriptorSource { specific, provenance }`, because it had been answering two different questions at once — may we print the contract's name, and who wrote these words.

**T031's real content**: ERC-2612 `Permit` and Permit2's `PermitSingle` / `PermitTransferFrom` are now built-in typed descriptors, matched on the whole `encodeType` (the typehash preimage), never on a primary-type name — so DAI's `Permit(…allowed)` deliberately falls through. Without them, the most common signature a person ever makes would have been downgraded to unverified by this very fix. They outrank the fetched contract entry, which costs a permit sheet the descriptor's `contractName` and gains it a verified label and zero round trips.

**What each sheet says now**

| provenance | web | desktop | Android / iOS |
| --- | --- | --- | --- |
| built-in, pinned, standard, deploy | nothing (was "selector not listed" — a lie about three of them) | nothing | nothing |
| fetched | "came from Vela's descriptor service and nothing authenticated it" | same | same |
| selector DB | best-effort line only | unchanged | unchanged |
| partial decode | best-effort line | **"no ERC-7730 descriptor"** (was the *verified ABI* line — the opposite of what happened) | already right |

**Corpus**: one key, `componentsUi.signing.descriptorFetchedWarning`, 15 locales, path pin 1692 → 1693. It arrived through `pending-corpus/` while another session held the corpus; it is now merged, the gate is green, and the temporary `pending()` resolver in `engine.server.ts` is gone — the key resolves like every other, so a future corpus regression fails loudly instead of silently dropping the warning.

**Verification**: `app_clear_signing` **72 pass** (6 new provenance tests, 3 rewritten); full core suite **42 suites green** with `crux,i18n-all`; `cargo clippy -p vela-core --features crux --all-targets` clean; desktop `cargo check` 0 errors and `cargo test signing::` 28 pass; web `pnpm check` 0 errors and 213 signing + settings tests pass; Android `:app:testDebugUnitTest` **634 tests, 0 failures**, including a `CoreWireDriftTest` assertion pinning the Kotlin `@SerialName`s to the generated `ClearProvenance`; iOS `xcodebuild build` succeeded.

## The device run found a P1 that the tests could not, because the test was vacuous

Putting the refusal on a real phone found the worst bug in this feature, and it was **in the work this feature added**.

Press "Add owner (blocked)", then dismiss the sheet — the only way out on a phone is the scrim or Back. The dApp is **never answered**. Every later request, from that page or any other, comes back `-32002 "Another request is open"`, until the process is killed. The wallet's dApp browser can no longer sign anything at all.

The chain: a self-call arrival sets `blocked` **and** `sign_error`; `swipe_action()` reads `sign_error.is_some()` and returns `Dismiss`; `dismiss()` deliberately sends no response ("a dismissed-but-committed op proceeds and its real result is still delivered"). No response means the shell never calls `markAnswered`, so the request slot never clears.

**Why the suite was green.** `app_self_call_guard.rs` asserted that "dismissal answers with the refusal" — by dispatching `Event::RejectTapped`. No phone sends that. Android dismisses with `SwipeDismissed` (`VelaNavHost.kt`), iOS likewise (`RootView.swift`); web's close button is the only `RejectTapped` in the product, which is why **web was the only shell that was safe**. The test pressed the one button that happened to work.

**The fix** is one condition in `swipe_action()`: a refused request returns `Reject`, so closing the sheet answers. The new test dispatches `SwipeDismissed`, the event the phones actually send — and was checked for vacuity the way the old one should have been: with the fix disabled it fails, with it enabled it passes.

**Confirmed on the phone after the fix**, in one process (`ETIME 04:38`, never restarted): the refusal, its dismissal, and three later requests — `enableModule` opened its own refusal sheet, `personal_sign` opened an ordinary sheet with a live slider, and an unlimited approval opened the guard. **No `-32002` anywhere.** The page received `-32603` with the refused function for the dismissed refusal, and a genuine `4001 user rejected` for the ordinary sheet it dismissed — distinguishable, which was the point.

That run also showed the answer was thin: `-32603 "addOwnerWithThreshold"` is a label, not an explanation, and Android and iOS dropped the core's `kind` on the way to the page while web forwards it. Both now send the sentence **and** the function (`"This request would change who controls the wallet (addOwnerWithThreshold)"`) plus a machine-readable `error.kind`, matching the web shell. A page that does not know the field ignores it.

The lesson is worth more than the fix: **a test that drives an event no shell emits proves nothing.** The `check-event-payloads.mjs` gate added in this same feature catches the payload half of that class; this half — dispatching a real event nobody sends — is still only caught by using the product.

Three smaller things came from the same run, all fixed:

- The Technical details row drew an **empty card** under a refusal on Android — every field it would show is nulled, and the row promised content it then did not have. Hidden on Android and iOS by a `TechModel.isEmpty`.
- **A typed endpoint did not commit on a tap outside.** The core persists on blur; Compose keeps focus until something takes it, and Back only hides the keyboard. So the page sat showing "Online · 779ms" for `https://index.invalid` — a health badge asserting an unresolvable host was reachable in under a second. The settings page now clears focus on a background tap.
- **The same bug on iOS, worse.** `SettingsUrlField` wired only `.onSubmit`, so only the keyboard's Done key ever committed — and the doc comment on that very parameter had said "on submit **or on losing focus**" since it was written. Now `@FocusState` commits on focus loss, as the comment always claimed.

## The iPhone run did not happen, and a device needs a human

Honest record: the iPhone 11 run was **not completed**, and the phone is now sitting at its passcode screen needing someone physically present to unlock it.

`xcodebuild test` could not start UI automation — three attempts, each failing at 60s with "Timed out while enabling automation mode". The standard remedy is a reboot, and the run took it after reading `devicectl device info lockState` as saying no passcode was set. That field (`passcodeRequired: false`, `unlockedSinceBoot: true`) means "no passcode is needed right now, because it is already unlocked"; `ideviceinfo -k PasswordProtected` says the phone does have one, and that check was not run first. The cable and pairing are fine.

What that cost: **FR-001's device verification, the iPhone half of FR-005/006, FR-009, FR-010 and FR-002, and the iOS erase run (T099) are all unverified on hardware.** The build and install recipe is proven and written down, including a trap worth keeping — `xcodebuild` wants the hardware UDID (`00008030-001A75961445802E`) while `devicectl` wants its own identifier (`F30282CB-…`), and passing the wrong one fails with an error that echoes it back as if it had been understood. A UI-test harness covering all four checks is written and compiles; it reaches every surface without a passkey ceremony through the repo's existing `VELA_PAGE` / `VELA_STATE` / `VELA_PARALLEL_SPACE` knobs.

Two of the three findings above came out of reading iOS code while blocked, so the run was not wasted — but it is not a substitute for looking.



## Looking at the screen found three bugs the tests could not

The founder's rule is that UI is never done from tests alone. Driving the real add-network screen on web and desktop against Zora — a chain I first proved with `eth_getCode` has everything except the signer factory — turned up three things, none of which any suite was ever going to catch:

1. **The desktop callout did not wrap.** Its first sentence ran out of the amber box, out of the dialog, and over the dimmed settings page behind it — about 40% of the sentence sitting on top of a network row, that row's chevron showing through the letters. Cause: `settings/components.rs` gave the text child `.flex_1()` with no `.min_w(px(0.))`, so the flex item kept the text's intrinsic width and never wrapped. Identical at 1280 and 1480, because the dialog width is a fixed token. Fixed, with the reason recorded at the line.
2. **The desktop add-network search returned nothing for any query — for anyone, today.** `decode_search_index` called `value.as_array()` on `/index/fuse-chains.json`, which answers `{"v","t","data":[…2897 rows…],"index":{}}`. Web, Android and iOS all read `json.data`; only desktop did not, so the wizard was unreachable on that shell and had been. Fixed (the bare-array form still decodes, for a self-hosted directory that serves one) with a test over both shapes and two malformed ones. This is not a spec-081 gap; it was found by trying to use the screen spec 081 changed.
3. **The desktop checklist crossed nothing.** Keeping the multi-key pair out of the aggregate rows — right, so a one-key owner is not told their chain is broken — left a warning sitting under four green ticks, which reads as a warning about nothing. The pair now gets its own row, `Safe Passkey Signer`, a product name like `EntryPoint v0.7`.

And one placement change on web: the callout sat **after** the Custom RPC field, so a form field separated the explanation from the crossed rows it explains. It now sits directly under the checklist, where desktop already had it.

A fourth thing came out of the same look: **the desktop dialog had no positive verdict at all.** Web shows a green *Compatible* pill; the desktop live wizard rendered none, and `settings.compatible` had been loaded and never used — a dropped judgement of exactly the kind `reference_dropped_judgement_sweep` describes. It went unnoticed while the checklist was all green; once FR-009 gave it a crossed row, a desktop reader saw a red cross and an amber warning with nothing anywhere saying the chain works. The pill is now drawn above the checklist, from the same `status_pill` component the rest of settings uses.

**And one broken gate that had nothing to do with this feature.** `e2e/locale-render.e2e.ts` asserted the zh hero subtitle as a literal — `用通行密钥签名` — which spec 080 reworded at the founder's request. The test has been red on `main` ever since, saying nothing true about the page. It now reads the headline and subtitle out of `zh.json`, so it tests the rule (the Chinese is in the server response, not added at hydration) rather than yesterday's copy. Site e2e is 56/56 again.

**Still true after the fixes, and worth the founder's eye**: on web the dialog body scrolls, so the green *Compatible* pill and the sentence that resolves it are never on screen at the same time — the pill and two red crosses are what a person sees first, with the explanation ~130–150px below the fold. Making the verdict and its qualification visible together is a design decision, not a bug fix, so it is written down rather than guessed at.

## What FR-009 turned up: 11 of the 24 built-in networks cannot hold a multi-key wallet

Having the check is one thing; running it is another. `eth_getCode` for the signer factory `0x1d31F259…`, over every built-in network's default RPC, 2026-09-22:

| Has Safe's passkey signer factory (13) | Does **not** (11) |
| --- | --- |
| Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base, Avalanche, Gnosis, Arc, Stable, Tempo, Celo, Robinhood Chain | **Ink, Kaia, Mantle, MegaETH, Monad, Plume, Soneium, Unichain, World Chain, X Layer, XRPL EVM** |

On all eleven the EntryPoint and the shared signer (key one) **are** deployed, which is exactly why nothing complained: a one-key wallet works there perfectly. A wallet created with two to seven keys cannot be deployed on any of them — its setup calldata calls `factory.createSigner` for each key after the first — so the first send on such a network fails, after the person has already been given an address to deposit into.

**This is a founder decision, and it is bigger than this spec.** Three things are true:

1. The readiness check runs in the **add-network wizard**, and these eleven are built in — so the new warning never appears for them. Surfacing it for built-in networks (at send time, or in the network list) is a change nobody has specified.
2. It is fixable by anyone, once per chain: CREATE2 through the Safe singleton factory, salt 0, with the creation code now in `deployment-data.json` — about 1M gas each. `getvela.app/chain-setup` will do it today for a chain typed in by hand.
3. Until one of those happens, the honest reading is that multi-key wallets are supported on 13 of 24 networks, not 24.

Nothing here was changed on that basis — deploying contracts on eleven mainnets and rewording the multi-key promise are both the founder's calls, not a subagent's.

## FR-011 — "12+ networks" became 24 (T055) — **done**

Counted from `app-web/vela-wallet/src/lib/services/chains.ts`, not remembered: 24 mainnets, matching the 080 claim ledger. Four corpus keys × 15 locales (`onboarding.welcome.featureOneAddressTitle`, `onboarding.intro.chainsBody`, `onboarding.welcomeWeb.meta.description`, `onboarding.welcomeWeb.features.oneAddress.title`), the extension manifest description, and the three Linux packaging blurbs (`metainfo.xml`, `vela-wallet.spec`, `deb/control.in`) which said "12 EVM networks". `docs/CONTENT-SOURCE-100-CLUES.md` clue 14 was corrected too, including its dead `src/models/chains.ts` path from the retired Expo tree.

Every edit was value-only — no corpus path was added or removed — and the "12 words" seed-phrase strings were left alone deliberately: they are a different twelve. `gen:i18n` → `lint:i18n` → `verify:i18n` → `dump:vectors` → `build:wasm` all green; core suite green afterwards.

## FR-004 (desktop half) — the self-hosting link opens

`app-desktop/vela-wallet/src/wallet/page.rs` drew "Self-hosting guide →" as plain text on the endpoints panel: an arrow that went nowhere, on the one screen whose whole subject is running these services yourself. It now opens `https://getvela.app/docs/self-hosting` (`SELF_HOSTING_URL`, beside `PRIVACY_URL`/`TERMS_URL`). The web half is queued behind another session's edits to the same settings files.

## FR-002 — the index you configure is the index that answers (T063–T067) — **done, device runs pending**

One rule, four transports. The core gained `NetServiceEndpoints::effective(field)` and `default_endpoint(field)`: "blank means the default" is now written once, and the two private helpers that each re-implemented it call it. A test walks all four fields against `""`, `"   "`, `"\t"` and a real URL.

Then each shell stopped keeping its own stale copy:

| Shell | What it did | What it does |
| --- | --- | --- |
| web | `onboarding/core/registry.ts` held a module-global `baseUrl` seeded at import, with a `setRegistryUrl` that had **zero callers** | `registryUrl()` reads `getPasskeyIndexURL()` per call; new `registry-url.test.ts` fails if that ever regresses |
| desktop | the saved endpoint was applied inside `OnboardingPage::new`, so a session that started **already signed in** — the common case — never applied it; and saving from Settings wrote the file without telling the process-wide slot | applied in `main.rs` before `session::boot`, and `WriteServiceEndpoints` now calls `set_registry_url`, so a save takes effect without a relaunch |
| Android | `VelaWalletApplication.registryNames` built `RegistryClient()` with the default and never assigned `baseUrl` | the client's `baseUrl` is set per lookup from `AccountStore.registryUrlOrDefault()` |
| iOS | `OnboardingModel` snapshotted the URL in `init`, for a model that lives as long as the process | `applyConfiguredRegistry()` runs at `init`, at `startCreate()` and at `signIn()` |

The iOS Service Endpoints page itself (FR-001, T060–T062) is separate work and still in flight.

## FR-004 — "Self-hosting guide →" goes to the guide

Web linked the repository root; desktop drew plain text with no handler at all. Both now open `https://getvela.app/docs/self-hosting` (desktop through `cx.open_url`, the app's own idiom, with the URL beside `PRIVACY_URL`/`TERMS_URL`). Phones have no such row, and adding one is a design choice rather than a bug — `fixtures.test.ts` asserts its absence, and that assertion stands.

## Corpus, this round

Two changes, both through the six-step gate:

- **new** `settingsModals.addNetwork.singleKeyOnly` (FR-009), 15 locales, path pin 1691 → 1692.
- **revised** `settings.eraseDevice.keeps`, 15 locales. It said "Your passkey stays in Face ID / fingerprint", which has been wrong since spec 019 for USB security keys, password-manager passkeys and cross-device caBLE — and wrong for the no-GMS Android phones that are the main group. It now says erasing cannot reach the passkeys because the passkey **provider** holds them, and names the three kinds.
- **values only** for FR-011's four network-count keys.

## T003/T004 — a ruler that can see FR-001

The two existing parity rulers compare `"type"` strings: they answer "does this client dispatch this event at all". FR-001 was invisible to both — iOS dispatched `endpoint_edited` with `id` where the core declares `field`, serde rejected the value, the machine never ran, and every ruler stayed green while the Service Endpoints page saved nothing.

Rather than bolt payload logic onto two table-printing scripts, this is a third one that is a **gate**: `scripts/check-event-payloads.mjs`, wired into `ci.yml` beside the reachability check (static, no toolchain, under a second). For every dispatch written as a literal — Swift dictionary, Kotlin JSON builder, TypeScript object — it compares the keys beside `"type"` with the variant's fields in the generated types, and exits 1 on a mismatch.

Getting it to zero false positives took four passes, each of which is a lesson about this repo:

| It reported | Why it was wrong | Fix |
| --- | --- | --- |
| `request_arrived` missing `params_json` | ts-rs carries Rust doc comments into the union, and a comment between two fields hides the second from a "comma then name" scan | blank comments before parsing |
| `account_switched`, `chain_changed`, `open` … | the same wire name is declared by several machines with different fields | score every declaration, report only the closest |
| `submitted`, `receipt_pending` | they are a shell **result** and a nested payload, not events | read every generated tagged union, not just `*Event.ts` |
| `eth_sign`, `json` as keys | Swift ternaries: `method == "eth_sign" ? "eth_sign" : "personal_sign"` looks exactly like a key | a key must follow `{`, `[` or `,` |

**T004, the run**: 491 literal dispatch sites checked, 2 skipped (payload assembled elsewhere), **7 mismatches — every one of them iOS `SettingsStore.swift`**, which is precisely the set research §2 predicted: `endpoint_edited`, `endpoint_blurred`, `provider_key_edited`, `provider_key_blurred`, `provider_test_requested` (all sending `id`), `override_field_edited` (no `field`) and `add_by_chain_id_requested` (`text` instead of `chain_id` + `now_iso`). No other shell has one. The FR-001 work closes them, and this gate keeps them closed.

## FR-010 — a name is shown only when it resolves back (T050–T054) — **done, device runs pending**

Every shell showed a reverse record as the recipient's name. A reverse record is a claim anyone can make about their own address, so the wallet was letting an attacker choose what the person read above the amount.

**The rule is in the core**, `app/name_verify.rs`, as a transcript machine in the shape spec 067 established — and it reuses that walk's vocabulary (`LookupRequest::EthCall` / `LookupAnswer`) rather than inventing a second one, so every shell already had a transport that speaks it. `registry.resolver(namehash)` → `resolver.addr(namehash)`, with an ENSIP-10 wildcard walk up to four ancestors for Basenames, one case-insensitive comparison, and **the proved name handed back** so what is drawn cannot drift from what was checked. Unresolvable strings (a single label, control or bidi codepoints, over 255 bytes) are refused before any call is made.

It **fails closed**: an unanswered call, a CCIP-read revert or a timeout is `Unavailable`, which shows the bare address. Failing open would let whoever poisoned the record pick the moment. Step 1 of the waterfall — a Vela wallet's own name — is not put to this machine: that is a label the chain gives for that address, not a record pointing at it.

No new copy and no corpus key: "no name" is the bare address, a state all four shells already draw.

**Caches were versioned**, since a cache filled under the old, absent rule holds unverified names: `recipient_id:` → `recipient_id.v2:` (web, iOS), `vela.recipientIdentity` → `…v2` (desktop, Android). That turned up a second bug on the way: web's `device-storage.ts` matched the cache by the literal `'recipient_id:'`, and `'recipient_id.v2:'.startsWith('recipient_id:')` is false — erase and the storage scan would have walked straight past it. Both now match without the colon, which also sweeps the retired generation.

**Verification**

| Check | Result |
| --- | --- |
| `tests/app_name_verify.rs` | **15 pass**, including the end-to-end proof: a reverse record saying `vitalik.eth` whose forward resolution is a different address yields `Mismatch` and no name — and the refused name does not even travel back in the JSON |
| core suite | **42 suites green** after the corpus regeneration |
| desktop | `cargo check` 0 errors; `cargo test executor::identity` 5 pass |
| web | `recipient-identity.test.ts` 15/15, `device-storage.test.ts` 7/7; `svelte-check` 0 errors in touched files |
| Android | `:app:testDebugUnitTest` exit 0, 79 suites; `IdentityWaterfallTest` 11 pass — and these run the **real core through the uniffi binding**, so Android proves the rule end to end rather than a stand-in |
| iOS | `xcodebuild build` **BUILD SUCCEEDED** (pass the simulator's `id=`, per `reference_ios_shell_traps`) |
| wasm | `build:wasm` + `sync:wasm` run afterwards, so the web shell's check is live; `recipient-identity.ts` looks the export up rather than importing it by name, and treats its absence as a check that cannot be made |

## FR-001 — the iOS Service Endpoints page (T060–T062) — **done, device run pending**

The page rendered fixtures: a retired host (`p256-index-rs.getvela.app`), four invented latency badges, and a Save that saved nothing. The cause was not the page — it was the **event payloads**. iOS sent `id` where the core declares `field` (and `provider`, and `chain_id` + `now_iso`), so serde rejected each event and the machine never ran. Seven payloads were wrong, exactly the seven the new payload gate finds:

| event | sent | declared |
| --- | --- | --- |
| `endpoint_edited` | `id: "chain-data\|passkey\|relay\|fiat"` | `field: "ethereum_data\|passkey_index\|bundler_service\|fiat_rates"` |
| `endpoint_blurred` | `id` | `field` |
| `provider_key_edited` / `_blurred` / `provider_test_requested` | `id` | `provider` |
| `add_by_chain_id_requested` | `text: "100"` | `chain_id: 100`, `now_iso` |
| `override_field_edited` | `chain_id`, `value` | `chain_id`, `field`, `value` |

With those fixed, `withEndpoints` and `servicePill` project the core's `NetView.endpoints`; a keystroke round-trips because each field's id is the core's own name; blur cleans, persists through `AccountStore`, and — now that `NetworkAdminExecutor` is built **with** the RPC pool — actually flushes the pools.

Two things the task did not name but the fix forced: `withProviders`, because once `provider_key_edited` reached the core the drawing's prefilled `alch_k3y...9fQ2` became live text one keystroke from being saved; and `commitRescueRpc` seeding the card first, because the core drops an override edit for a chain whose card was never expanded, so the rescue would have sent a now-valid event that still did nothing.

**Verification**: `xcodebuild build` succeeded; `xcodebuild test -only-testing:VelaWalletTests` — **663 tests in 85 suites passed**, including eight new `SettingsEndpointsTests` that assert on the core's view or on storage, never on the dictionary. They were checked for vacuity: reverting two payloads to `id` fails four of them. `check-event-payloads.mjs` now reports **0 mismatches across 491 sites**, and was itself proved to cover this file by reintroducing `id` and watching it fail. Device run (quickstart item 001: set each endpoint, relaunch, badges from a real probe, Reset) still owed.

## FR-020 — the docs say what the product does, in fifteen languages

Eight English docs changed as the gaps closed: `bybit-attack`, `clear-signing`, `install`, `networks-and-fees`, `security-audits`, `self-hosting`, `send-and-receive`, `whitepaper`. Three "known gap" bullets left `security-audits.md` outright, the self-call advice ("reject any request whose target is your own address") is gone from three places because the wallet now does it, and `clear-signing.md`'s definition of **verified** was rewritten to the one the code can defend.

The fourteen translated locales were brought back into agreement rather than left stale — a translation that is structurally perfect and describes a product that has changed is worse than one that is missing, because it reads as current.

**A tool was missing and is now there.** `i18n:stamp` records agreement for every doc in every locale at once, which is right after a full round and wrong at every other moment: a translator finishing one file would silently certify thirteen they had not touched. `app-web/getvela.app/scripts/i18n-stamp-doc.ts` (`bun run i18n:stamp:doc <locale>/<file>.md`) stamps exactly the files named. It exists because I proved the failure by doing it — stamping `zh/whitepaper.md` fresh against a translation nobody had updated — and had to undo it by hand.

**Two English defects the translators found**, both fixed, both the kind only a careful reader hits:

1. `networks-and-fees.md` said the wallet needs "**eleven** standard contracts" two paragraphs above the new text saying "two of the **twelve** contracts it checks". Both translation passes reported it independently. The same count was stale in the site's chain-setup FAQ ("Why these eleven contracts?"), now twelve, with the passkey signer factory named.
2. `self-hosting.md`'s closing parenthetical began "Until September 2026 **this table** listed four exceptions" — written while the table was still there, and left pointing at the wrong table once it was deleted. Now "there were four exceptions to that".

And two more, both from the translators reading more carefully than the English had been written:

3. `whitepaper.md`'s clear-signing paragraph still ended "Fetched descriptors are not cryptographically authenticated", beside the sharper wording this pass added further down the same file. Both now say the same thing.
4. `security-audits.md`'s Certora M-01 section said "a dApp can ask your wallet to change its own owners (see 'Gaps' below)" — pointing at a bullet this pass deleted, and asserting something the wallet now refuses. Rewritten: the risk that remains is for somebody removing a compromised key through other Safe tooling.

**A register fact worth not "fixing" later.** zh-HK is written two different ways on purpose, and both are right: the **app corpus** (`rust/crates/vela-core/i18n/locales/zh-HK.json`) is spoken Cantonese — 喺, 冇, 嘅, 同 — while the **site docs** (`app-web/getvela.app/src/content/docs/zh-HK/`) are HK-usage written Chinese with no Cantonese particles anywhere. Each is internally consistent; someone sweeping for consistency across both would break one of them.

## Shipped as PR [#309](https://github.com/mondaylabsltd/vela-wallet/pull/309)

Eight commits on `081-audit-product-gaps`, merged **after** the two service PRs — p256-index [#8](https://github.com/mondaylabsltd/p256-index/pull/8) and vela-relay [#13](https://github.com/mondaylabsltd/vela-relay/pull/13) — because this branch's self-hosting docs already describe those fixes as done. Merging the other way round would publish a claim that is not yet true; both service PRs carry a comment saying so.

**Final gate sweep**: `cargo test --workspace --features vela-core/i18n-all` 48 suites; desktop `cargo check` 0 errors (7 pre-existing warnings) and 443 tests; Android `:app:testDebugUnitTest` green; iOS 663 tests in 85 suites; web `pnpm check` 0 errors, 285 e2e, and the three pre-existing unit failures from the baseline (explore fixtures, i18n FLOW_KEYS, the `36px` in `SigningHeader.svelte`); site 0 errors, 848 unit, 56 e2e, `i18n:status --gate` **0 stale in all fifteen locales**; `check-event-payloads.mjs` 0 mismatches over 491 dispatch sites; `check-native-reachability.mjs` and `gen-passkey-providers.mjs --check` both clean.

`gen-passkey-providers.mjs --check` had been failing on `main` for anyone who ran it: the generator emitted one-line tuples, `cargo fmt` wrapped the long ones, and the comparison then called a byte-identical catalog stale. Regenerating to silence it produced a 198-line diff that changed nothing but whitespace. The generator now runs `rustfmt` on its own output, so the check means what it says.

## CI caught what my own sweep did not: `cargo fmt`

The first push failed both Rust jobs — `rust` and `desktop` — on `cargo fmt --all --check`, and nothing else. Nineteen files, all of them Rust I had written or edited by hand across the feature; the code compiled, clippy was clean and every test passed. My standing-gate sweep ran `cargo test` and `cargo clippy` in both workspaces and never ran `cargo fmt --check`, which CI runs **before** either of them.

It is the same shape as the `gen-passkey-providers.mjs` problem this feature fixed an hour earlier — generated or hand-written Rust that nobody put through rustfmt — and I did not notice I was standing in it. `quickstart.md`'s standing gates now list `cargo fmt --all --check` first, in both workspaces, with the toolchain pin (1.97.1) called out: a different rustfmt is a different answer.

Fixed by running it, not by reformatting by hand. All 48 workspace suites and the desktop's 443 still pass afterwards, and `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings` — the exact command CI uses, which my sweep had also been running without the feature flag — is clean.

## And a second CI failure: the Linux build is the only one that compiles the Linux code

`desktop` failed again, this time on a real compile error — `crate::webview::clear_browsing_data` not found. The erase work (FR-017) added that call to `erase_device`, which is **not** `cfg`-gated, and `mod webview` only exists on macOS and Windows; Linux gets `webview_absent.rs`, a same-named module that is the shape of the browser it does not have. The stub never grew the new function.

Nothing local could have caught it. gpui's Linux build needs a system toolchain this machine does not have, so `cargo check` here compiles the macOS path and is silent about the other one — the same blind spot the workflow already documents for Windows ("only a real Windows build finds that class of bug"). CI is the first place the Linux arm is ever compiled.

The stub now answers `true`, deliberately, not `false`: the bool means "was the platform asked", the caller logs a `false` as "no web view to clear", and on Linux there is no browser and so nothing that failed to be cleared. `false` would print a warning about a permanent condition on every erase.

Swept for more of the same: `clear_browsing_data` is the **only** `webview::` call this branch added, and every other `webview::` name used outside a `cfg` block is already in the stub.

## …and a third: `cargo fmt` invalidated the committed wasm

`rust` failed on `build-web.mjs --check` — "the committed artifact was NOT built from the current Rust source". Correct, and my fault twice over: `rust/pkg-web`'s fingerprint is taken over the Rust **source bytes**, so running `cargo fmt` moved it exactly as a real code change would. I reformatted and pushed without rebuilding.

Rebuilt and synced; `verify-web.mjs` replays 47,443 conformance cases through the shipped artifact and is green. `quickstart.md` now says the coupling out loud, next to the fmt gate: **reformat first, then rebuild** — the stale-artifact check is the one that catches you last.

Three CI failures in a row, all of them the same shape: gates that exist, that I did not run, in an order that matters.

## One more parity tail, from the Android device run

Android and iOS hide the Technical details card under a refusal; web was still putting the raw `params_json` of the very request the wallet refused behind a disclosure. `techModel` is now refusal-aware on web too — the one shell that had stayed lax about it.

## Still open in this feature

Everything else in [tasks.md](tasks.md): descriptor provenance, network readiness, forward-verified names, the iOS endpoints page and index-per-call, `X-Rpc-Url`, release provenance, the dormant routes, feedback, erase, and the two service-repo PRs. The docs sync (FR-020) lands with each gap as it closes.

**Follow-ups noticed while working**, recorded so they are not lost:

- ~~desktop, Android and iOS still render the decoded body and the fee row under the refusal~~ **done**: all three now take the web's treatment. A refused request shows the refusal and nothing else — no decoded body, no simulation, no cap editor, no fee row, and **no confirm control at all** (absent, not disabled: a dead slide reads as an option somebody merely failed to use). `fee` and the confirm pair became optional on Android (`SigningScreenModel`) and iOS (`SigningModel`); desktop takes the `funding` surface's own pattern (`FeeModel::Hidden` plus an empty confirm label the panel skips). Android `:app:testDebugUnitTest` green, iOS **663 tests pass**, desktop **442 pass**.
- `SignResponder.sendResponse` now carries the core's error `kind` — other shells could use it the same way the extension window does.
