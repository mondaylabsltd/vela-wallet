# 088 — Results

Branch `088-store-readiness`, from `origin/main` @ `67e2d193d`. Nothing has been pushed,
uploaded or deployed. Owner steps are in [owner-checklist.md](owner-checklist.md).

## Commits

| Commit | What |
|---|---|
| `968429fbd` | fix(i18n): the erase sheet names what the servers keep and what no one can delete |
| `87166c694` | fix(core): the wallet is offered only to secure contexts; core names the host an outside link opens |
| `d679f6509` | fix(android): upload-key signing or refuse; versionCode from history; JNA 5.17; explicit 16 KB |
| `a1d0d7391` | fix(android): release ignores debug launch extras; outside link asks first |
| `2cc3457dd` | fix(android): background tracker runs on Android 10/11 |
| `f59b07f1b` | fix(ios): outside link asks first; truthful Info.plist; iPhone portrait; build number |
| `43630d328` | fix(site): `/support`, `/delete`, privacy erase paragraph |
| `ed7200123` | fix(android): splash shows the Vela icon, not Android's default robot |
| (this commit) | docs(088): store docs, owner checklist, plan, tasks, results |

## 1. Android release signing and build numbers (FR-001, FR-012)

### What changed

`app/build.gradle.kts` reads four values, each from an env var or else from the gitignored
`app-android/vela-wallet/keystore.properties`:

| Env var | Property key |
|---|---|
| `VELA_UPLOAD_STORE_FILE` | `storeFile` |
| `VELA_UPLOAD_STORE_PASSWORD` | `storePassword` |
| `VELA_UPLOAD_KEY_ALIAS` | `keyAlias` |
| `VELA_UPLOAD_KEY_PASSWORD` | `keyPassword` |

- **Release without a key fails.** The task `velaCheckUploadKey` runs before
  `preReleaseBuild` and names each missing value.
- **Unsigned output only on request.** It needs `-PvelaUnsignedRelease`; the CI packaging
  job passes it.
- **`.gitignore`** ignores `keystore.properties` and `*.keystore`.
- **versionCode:** `-PvelaVersionCode`, else `VELA_VERSION_CODE`, else
  `git rev-list --count HEAD`. A bad value fails the build.
- **iOS build number:** `CURRENT_PROJECT_VERSION` is set the same way in
  `ios-package.yml` and in the owner checklist.
- **Both package workflows** check out full history.
- **`docs/NATIVE-LAUNCH-CHECKLIST.md` §A1** is corrected. A release never fell back to the
  debug key; it came out unsigned.

### Evidence (throwaway RSA-2048 key in the scratch dir, never committed)

- **No key:**
  - `./gradlew :app:bundleRelease` fails at `:app:velaCheckUploadKey`, listing all four
    missing values and how to set them.
  - `velaCheckUploadKey -PvelaUnsignedRelease` passes.
  - `-PvelaVersionCode=abc` fails with "must be a whole number…".
- **Key from env vars:** `bundleRelease` succeeds.
  - `jarsigner -verify -verbose:summary -certs`: "jar verified", signed by `CN=Throwaway 088`.
  - SHA-256 `6D:F8:5C:83:…:41:85`.
  - Merged manifest `versionCode="3211"`, the commit count at that point.
- **Key from `keystore.properties`:** with `-PvelaVersionCode=3212`, the bundle is
  "jar verified" and `versionCode="3212"`. The file was deleted afterwards.
- **Release APK:** `assembleRelease`, then `apksigner verify` (V2, cert `6df85c83…`).
  `zipalign -c -P 16 4` reports "Verification successful".
- **Not done:** `~/.android/vela-release.keystore` was **not** opened. Listing its
  fingerprint needs its password, which is the owner's.

## 2. 16 KB pages (FR-002)

### What changed

- JNA 5.14.0 → **5.17.0**.
- `rust/scripts/build-android.sh` passes `-C link-arg=-Wl,-z,max-page-size=16384` for
  `aarch64` and `x86_64` through `CARGO_TARGET_*_RUSTFLAGS`. The cargo `-v` log confirms the
  flag reached the aarch64 and x86_64 rustc lines and not armv7.

### Evidence

`llvm-readelf -lW` (NDK 27.1) on every `.so` in the signed `app-release.aab`. The value
shown is the smallest LOAD `p_align`:

| ABI | Library | LOAD align | 16 KB OK |
|---|---|---|---|
| arm64-v8a | libandroidx.graphics.path.so | 0x4000 | yes |
| arm64-v8a | libdatastore_shared_counter.so | 0x4000 | yes |
| arm64-v8a | libimage_processing_util_jni.so | 0x4000 | yes |
| arm64-v8a | libjnidispatch.so (JNA 5.17.0) | 0x4000 | yes |
| arm64-v8a | libsurface_util_jni.so | 0x4000 | yes |
| arm64-v8a | libvela_core_uniffi.so | 0x4000 | yes |
| x86_64 | libandroidx.graphics.path.so | 0x4000 | yes |
| x86_64 | libdatastore_shared_counter.so | 0x4000 | yes |
| x86_64 | libimage_processing_util_jni.so | 0x4000 | yes |
| x86_64 | libjnidispatch.so (JNA 5.17.0; was 0x1000 on 5.14.0) | 0x4000 | yes |
| x86_64 | libsurface_util_jni.so | 0x4000 | yes |
| x86_64 | libvela_core_uniffi.so | 0x4000 | yes |
| armeabi-v7a | the same six | 0x4000; libvela_core_uniffi 0x1000 | n/a (32-bit is outside the rule) |

**Dev fixtures are not in the release bundle.**

- **Native:** no `libvela_dev_fixtures.so` in `base/lib`.
- **Dex:** the only `ParallelSpace` class is the release no-op
  `app.getvela.wallet.dev.ParallelSpaceBinding`.
- **Fixed keyset:** the keyset scalar (`d80133c5…`, from `vela-core/src/dev_fixtures.rs`)
  appears in no dex and no `.so` of the bundle.
- **Control:** the same scalar is present in the debug-only `libvela_dev_fixtures.so`.

## 3. Release ignores debug launch extras (FR-003)

### What changed

`LaunchExtras.kt` lists the device-pass extras that only a debug build reads:
`vela.startDestination`, `vela.flowState`, `vela.settingsState`, `vela.settingsDark`,
`vela.gallery`, `vela.openUrl` and `vela.parallelSpace`. `MainActivity` reads every one of
them through it. `vela.openUrl` used to load any page in release with no question asked;
it is now debug-only as well.

### Evidence

- `LaunchExtrasTest` (4 tests) passes.
- On the API 30 emulator, the throwaway-signed **release** APK was started with
  `--es vela.startDestination gallery --ez vela.gallery true --es vela.openUrl https://example.com`.
  It opened on **Welcome**: no gallery, no page.

## 4. External `velawallet://open` links (FR-004), Android and iOS

### Rules (vela-core)

- **`dapp_permissions::is_secure_context`.** True for https, or http on exact loopback:
  `localhost`, `*.localhost`, a full `127.x.y.z`, or `[::1]`.
- **`dapp_browser::page_message`.** Drops every message from a frame that is not a secure
  context, before any hello or request is read.
- **`dapp_rpc::provider_script`.** Returns early unless `window.isSecureContext`.
- **`dapp_rpc::external_page_host`** (UniFFI `dapp_external_page_host`). An https page with
  a plain host qualifies; the function returns that host. Anything else gets `None`:
  `http`, other schemes, user-info, or a missing host.

The listener on Android stays `"*"`. The androidx docs allow only plain hosts, `*.host`
and IP literals as origin rules, so there is no "any https" rule, and the core gate does the
filtering. The rule also reaches the desktop and web in-app browsers through the core.

**Behaviour change:** spec 070 let an **http LAN** test dApp connect and sign
(`a LAN test dApp may sign`). It now gets no provider, per the owner's rule "https plus
loopback for dev". A loopback test dApp still signs (`a_loopback_test_dapp_may_sign`). A
real phone testing a dApp served from the Mac needs https or `adb reverse`.

**Owner ruling 2026-10-02:** LAN http dApps get the wallet only in the hidden debug mode →
[spec 091](../091-debug-mode-lan-dapps/spec.md). Seven taps on the version in Settings → About reveal
the switch. With it on, the core's `offers_wallet(origin, debug_mode)` adds http on the device's own
network. Off, the default, this section holds unchanged.

### Shells

Both shells draw the same sheet: the host, the URL, "Open in Vela Wallet"
(`receive.pay.open`) and Cancel (`common.cancel`). **No new strings.**

- **Android:**
  - `BrowserController.askToOpen` asks the core for the host; `answerExternal` acts on the
    reply.
  - `ExternalPageSheet` is hosted next to `CrashSheet`, outside the NavHost.
  - `routeDeepLink` now asks instead of opening.
- **iOS:**
  - `RootView.openLink` → `BrowserController.askToOpen` → `ExternalPageSheet`.
  - On "Open", the page loads and the app switches to 探索.

### Evidence

- **Core tests:** `a_page_off_a_secure_context_is_never_answered`,
  `a_loopback_test_dapp_may_sign`, `secure_context_is_https_or_exact_loopback`,
  `an_external_link_names_the_host_it_will_open`, and the script-guard assertion.
- **Shell tests:**
  - Android `BrowserMachineTest`: the script carries the guard; insecure pages are never
    answered.
  - iOS `ExternalPageTests` (3) and the updated `ProviderScriptTests`.
- **Emulator** (API 30, debug build):
  - `velawallet://open?url=https%3A%2F%2Fapp.uniswap.org%2Fswap%3Fchain%3Dbase` showed the
    sheet over Welcome, headed `app.uniswap.org`, with the full URL below it.
  - Cancel dismissed it.
  - The same link with `http://` showed nothing; logcat: `browser.external link refused scheme=http`.

## 5. Android 10–11 background tracker (FR-005)

### What changed

`TrackerWorker.request(sdk)` marks the work expedited only on API 31 and later. Below
that it is a plain request: no foreground service, no notification, no new string.

### Evidence

- **Before the fix**, on an API 30 emulator: a temporary probe (not committed) enqueued
  the pre-088 request. It **FAILED**:
  `WM-WorkerWrapper: java.lang.IllegalStateException: Not implemented at androidx.work.CoroutineWorker.getForegroundInfo`.
  WorkManager 2.10.5 fails the work rather than crashing the app, so on Android 10 and 11 a
  send in flight was never polled in the background.
- **After the fix:**
  - `TrackerWorkDeviceTest` enqueued through `TrackerWorker.enqueue`, as the wallet does.
    Result: `OK (1 test)`, logcat `Worker result SUCCESS`, and no exception.
  - `TrackerWorkRequestTest` (JVM): 29–30 plain, 31–36 expedited.
- **Emulator note:** `Pixel_6_API_30` was already running as `emulator-5554`, started by
  another process. I left it alone and booted a temporary clone, `Pixel_6_API_30_088`, on
  `emulator-5580`. The clone was shut down and deleted afterwards.

## 6. iOS archive and export dry run (FR-006)

### Archive

```
xcodebuild archive -project VelaWallet.xcodeproj -scheme VelaWallet -configuration Release \
  -destination 'generic/platform=iOS' -archivePath <scratch>/Vela.xcarchive -allowProvisioningUpdates \
  CURRENT_PROJECT_VERSION=$(git rev-list --count HEAD) VELA_GIT_COMMIT=$(git rev-parse --short=7 HEAD)
```

- Result: **ARCHIVE SUCCEEDED**.
- Archived `Info.plist`:
  - `CFBundleVersion` 3217, `CFBundleShortVersionString` 0.9.5, `VelaGitCommit` f59b07f.
  - `ITSAppUsesNonExemptEncryption` false.
  - `UISupportedInterfaceOrientations~iphone` = Portrait only (iPad unchanged).
  - The new `NSLocalNetworkUsageDescription` and Bluetooth strings are present.

### Export

`-exportArchive` with `method app-store-connect`, `destination export`,
`teamID F9W689P9NE`, `signingStyle automatic`. It was run twice: once on an early archive
and once on the final one.

- Xcode created and used **"iOS Team Store Provisioning Profile: app.getvela.VelaWallet"**
  with "Apple Distribution: MONDAY LABS LTD".
- Export entitlements: `application-identifier`, `beta-reports-active`,
  `associated-domains` (`webcredentials:getvela.app`, `applinks:getvela.app`),
  `team-identifier`, `get-task-allow=false`.
- There was **no entitlement or profile error**.
- The export stopped at `codesign`. macOS showed the keychain prompt that lets codesign use
  the distribution private key, which only the owner can approve. I stopped codesign there.
  The owner will see the prompt once and should choose **Always Allow**. Nothing was uploaded.

### `com.apple.security.smartcard`

Xcode **drops** this key when signing for iOS. It is absent from the archive's `.xcent` and
from both exports' entitlements, so it does not break export. It is used in code
(`SmartCardCtapCeremony.swift` → `TKSmartCardSlotManager`), so I kept it and recorded the
fact in the entitlements comment. No signed build has ever carried it. The owner should
check the USB security-key route on TestFlight (checklist §1.7).

### Dev fixtures in Release

- `nm` and `strings` on the archived binary find no `vela_dev_fixtures` or
  `ParallelSpaceBinding`, and no fixture key scalar.
- What remains is the `ParallelSpaceProvider` protocol hook, which holds no keys.

## 7. iOS config (FR-007, FR-008, FR-009)

- **Export-compliance comment:** now names TLS, the OS passkey crypto, and the core's
  AES/AES-GCM used only inside authentication protocols. Those are CTAP2 PIN/UV
  (`ctap/pin_uv.rs`) and the caBLE tunnel (`cable/`). The comment cites the EAR
  authentication carve-out. The value stays `false`.
- **Portrait:** iPhone is portrait-only. Landscape came from the Xcode template
  (`a5153073b`), not a decision.
- **Bluetooth string:** narrowed to the one remaining use, "sign in with your phone". The
  Trusted Signer BLE channel was cut on 2026-09-23 (`0801564b7`).
- **Local network:** the app opens no LAN socket of its own and uses no Bonjour. A person
  can still point it at the LAN, through a page typed into 探索 or a self-hosted endpoint,
  so `NSLocalNetworkUsageDescription` is added. `NSAllowsLocalNetworking` is kept: since
  iOS 10 it changes nothing, and the UI-test harness comment relies on it.
- **Tests:** VelaWalletTests on a cloned iPhone 16 simulator: **1075 tests in 139 suites
  passed**. The clone was deleted afterwards.

## 8. Deletion truth (B1)

### Erase sheet

`settings.eraseDevice.keeps` was changed in all 15 locales. English:

> You keep the wallet: its passkeys are with your passkey provider, not this app — sign in again and the same address returns. Not erased: Vela Relay keeps sent transactions up to 14 days, the key index keeps registrations up to 30 days, and the public-key record on chain is permanent; no one can delete it.

### i18n keys

There are **no new keys**. Changed values and byte deltas (en + ja is the budget):

| Key | en | ja |
|---|---|---|
| `settings.eraseDevice.keeps` | 274 → 308 (+34) | 409 → 394 (−15) |
| `settings.eraseDevice.desc` (trimmed, same meaning) | 85 → 69 (−16) | 143 → 128 (−15) |
| `settings.eraseDevice.loses` (trimmed, same meaning) | 164 → 155 (−9) | 219 → 200 (−19) |
| **Total** | **+9** | **−49** |

- Net change is **−40 bytes**; `i18n_residency` reads ja + en 138,631 of 138,800.
- The external-link sheet reuses `receive.pay.open` and `common.cancel` (0 bytes).
- Other locales changed only `keeps`. Their `desc`/`loses` keep the same meaning with
  slightly longer wording.
- **Regenerated:** `src/i18n_catalogs/`, `assets/i18n/` and
  `tests/vectors/i18n-exhaustive.json`, plus the wasm (`assets/wasm/vela_core_bg.9948da0ab2d0.wasm`,
  replacing `…273fc631ccc3`) and `rust/pkg-web`.
- **Gates:** `gen:i18n`, `lint:i18n` ("no new defects"), `verify:i18n` (75,440 comparisons,
  zero divergences) and `dump:vectors`.

### Site (`app-web/getvela.app`)

- **New pages:** `/delete` and `/support`. They are English-only, like `/privacy` and
  `/terms`, and are in the sitemap list.
- **Footer:** "Support" and "Delete your data" links, with labels in all 15 site locales.
  These are the site's own catalogs, not the core corpus, so they cost nothing against the
  budget. The 14 non-English labels were machine-written and have not been reviewed by a
  native speaker.
- **`/privacy`:** the erase paragraph now matches the apps. iPhone and desktop erase have
  shipped, and the paragraph links to `/delete`. A Trusted Signer bullet was added, and the
  page is dated 1 Oct 2026.
- **Checks:**
  - `bun run check`: 0 errors.
  - vitest: 885 passed.
  - `vite build`: OK.
  - Local preview: `/support`, `/delete`, `/privacy` and `/terms` all return 200.
- **Not run or not passing:**
  - Playwright e2e: lists updated, tests not run.
  - `bun run lint` fails on errors that were already there. Prettier reports 266 files; the
    `no-navigation-without-resolve` rule also fires. The new pages are clean. CI runs only
    `bun run check`.
- **Not deployed.**

## 9. Documents (B6, B7, I18)

### `docs/store-submission/privacy-and-review.md`

- Removed:
  - the testnet build offer;
  - the single-use Bluetooth wording;
  - "20 testers" (now 12 testers for 14 days, personal accounts only; Organization
    accounts are outside the rule).
- Added:
  - §3a, how a reviewer creates a wallet with a passkey, plus a `DEMO VIDEO:` placeholder;
  - §3b, the features a reviewer will meet;
  - the deletion URLs.
- Rewritten: export compliance.
- Corrected: guideline citation 3.1.5(b) → 3.1.5(i).
- Pre-flight list updated to show what this branch already fixed.
- Flagged for the owner: the I11 call (wallet and key names on iOS).

### `docs/store-submission/store-listing-copy.md`

Rewritten from the claim ledger:

- no audit claim, and no "your face is your key";
- every way a key can sign is named;
- passkey sync is attributed to the provider;
- "No accounts. No tracking." became "No email or phone number. No ads, and no analytics or
  tracking SDKs in the app.";
- the Chinese App Store text does not name Google;
- field lengths were counted.

### Planning files

`owner-checklist.md`, `plan.md` and `tasks.md`.

## Test totals

| Suite | Result |
|---|---|
| `cargo test -p vela-core --features i18n-all,crux` | 2200 passed, 0 failed |
| `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2243 passed, 0 failed |
| `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings` | clean |
| `cargo fmt --check` and `build-web --check` | clean, current |
| Android `:app:testDebugUnitTest` | 870 passed, 0 failed (see note) |
| Android instrumented `TrackerWorkDeviceTest` (API 30 emulator) | 1/1 |
| iOS VelaWalletTests (simulator) | 1075 tests in 139 suites passed |
| Site `check` / vitest / build | 0 errors / 885 passed / OK |

Note on the Android count: the first run had 1 failure,
`SendMachineTest` "a split send's rows are all written ahead…", a timeout while the iOS
xcframework was compiling at the same time. It passed on the rerun.

## Found along the way

1. **Android splash robot (fixed, `ed7200123`).** core-splashscreen's default icon is
   `sym_def_app_icon`, the Android robot, and every launch showed it. The splash now shows
   the Vela mark on its tile. Verified with an emulator screenshot of the release APK.
2. **Desktop not rebuilt.** The core rules also govern the desktop in-app browser. Its
   tests use loopback (`http://127.0.0.1:8137`) and should be unaffected, but I did not
   compile `app-desktop` (disk: about 43 GB free on `/Volumes/data`). Web wallet unit tests
   were not run either (no `node_modules`). `gen-core-types` produced no diff.
3. **Loopback check in `trusted_signer::is_loopback`.** It accepts any host starting with
   `127.`, for example `127.0.0.1.evil.com`. The browser's own secure-context check still
   protects WebAuthn. Not changed.
4. **Phone caption mentions Windows Hello.** `onboarding.create.methodPlatformBody` says
   "Touch ID or Windows Hello" on phones (found by the docs pass). This is a copy bug, not
   in scope.
5. **Tunnel hosts not in the privacy evidence.** The caBLE tunnel hosts
   (`cable.ua5v.com` / `cable.auth.com`) are missing from privacy-evidence §1.2. The
   `/delete` page invites email for retention gaps: index tasks that never finish, and a
   relay `intent` with no TTL.
