# Native (iOS / Android) launch checklist — items that need you

Context (2026-07-01): at the time, the app had only ever been tested as the
Expo web build, and a cross-platform audit found the native-only defects
below. **That app was retired in spec 039 (2026-09-11); the native shells
`app-ios/VelaWallet` and `app-android/vela-wallet` are its successors.**
Every item was re-read against them (2026-09-23; §A1 again 2026-10-01, spec 088): §A1's code half is done and its console half is still
launch-blocking, §A2 still holds, **all of §B is now settled in code**, §C is
down to one Android-only item, and §D has been rewritten for shells that have
no React Native in them. This doc is the remainder — things that need your
**Apple/Google accounts** or a **real device**.

---

## Fixed in the retired tree (history — none of these files exist now)

These were the 2026-07-01 native fixes to the Expo app. They are listed so the
class of defect is on the record; the paths are gone and there is nothing here
to verify. The equivalents in the current shells are platform-native and were
wired by specs 040–049 (Android) and 050–058 (iOS).

| Fix | Where (deleted) |
|-----|-------|
| Hermes polyfills: `crypto.getRandomValues` + `btoa/atob` + `Buffer` | `src/polyfills.ts` (+ `.web.ts` no-op), loaded first in `src/app/_layout.tsx` |
| Android passkey `register()` JSON now escaped (`JSONObject`/`JSONArray`) | `modules/vela-passkey/android/.../VelaPasskeyModule.kt` |
| QR scanner Android hardware-Back dismiss (`onRequestClose`) | `src/components/QRScanner.tsx` |
| Android keyboard avoidance (`behavior` no longer `'height'`) + `softwareKeyboardLayoutMode: resize` | `src/components/ui/ScreenContainer.tsx`, `app.json` |
| Android coroutine scope cancelled via `invalidate()` (New-Arch-safe) | `VelaPasskeyModule.kt` |
| iOS passkey presentation anchor → foreground-active key window | `modules/vela-passkey/ios/VelaPasskeyModule.swift` |
| iOS passkey error mapping (`.canceled` + `.notInteractive` → CANCELLED) | `VelaPasskeyModule.swift` |

Their regression tests (`src/__tests__/polyfills.test.ts`,
`src/__tests__/native-fixes.test.ts`) went with the tree.

---

## A. Blockers requiring your Apple / Google accounts

### A1. Android release signing + Play App Signing cert (signing in code since spec 088; the console half is yours)
**Code (done, spec 088, 2026-10-01).** `app-android/vela-wallet/app/build.gradle.kts`
now has a `release` `signingConfig` read from four values — each an environment
variable, else a line in the **gitignored** `app-android/vela-wallet/keystore.properties`:

| env | `keystore.properties` |
|---|---|
| `VELA_UPLOAD_STORE_FILE` | `storeFile` (absolute, `~/…`, or relative to that file) |
| `VELA_UPLOAD_STORE_PASSWORD` | `storePassword` |
| `VELA_UPLOAD_KEY_ALIAS` | `keyAlias` |
| `VELA_UPLOAD_KEY_PASSWORD` | `keyPassword` |

A release build without all four **fails before compiling**, naming what is
missing. It is never signed with the debug key and never comes out unsigned by
accident. (The earlier text here said a release "falls back to the debug
keystore" — it did not: under AGP it came out **unsigned**.) The one way to an
unsigned release is to ask for it, `-PvelaUnsignedRelease`, which only CI's
packaging check does. Debug builds and unit tests read none of this.

**versionCode** (Play needs a higher one on every upload): `-PvelaVersionCode=N`,
else `VELA_VERSION_CODE`, else `git rev-list --count HEAD` (it only grows along
`main`). `versionName` stays hand-set per release. iOS: pass the same number as
`CURRENT_PROJECT_VERSION` on the archive command line (see
`specs/088-store-readiness/owner-checklist.md`).

**Build the upload bundle:**
```bash
cd app-android/vela-wallet
export VELA_UPLOAD_STORE_FILE=~/.android/vela-release.keystore VELA_UPLOAD_KEY_ALIAS=vela-release
read -rs VELA_UPLOAD_STORE_PASSWORD; export VELA_UPLOAD_STORE_PASSWORD
read -rs VELA_UPLOAD_KEY_PASSWORD; export VELA_UPLOAD_KEY_PASSWORD
./gradlew :app:bundleRelease          # → app/build/outputs/bundle/release/app-release.aab
jarsigner -verify -verbose:summary -certs app/build/outputs/bundle/release/app-release.aab
```

**Passkeys still need the console half.** Google Play re-signs what it
distributes with the **Play app-signing key**, and Credential Manager accepts
rpId `getvela.app` only for a signing certificate listed in
`getvela.app/.well-known/assetlinks.json`. Until Play's key is listed, testers
who install from Play **cannot create a wallet or sign in**.
1. Enroll in **Play App Signing** (default for new apps) and upload the first
   AAB. Open **Play Console → Test and release → App integrity → App signing
   key certificate** and copy the **SHA-256**.
2. Add it — and the upload key's SHA-256, for builds you install yourself — to
   `app-web/getvela.app/src/routes/.well-known/assetlinks.json/+server.ts`, in
   the `app.getvela.wallet` entry's `sha256_cert_fingerprints` (after line 24).
   Check whether `A3:8E:36:FE:…` ("Release keystore") is your upload key:
   `keytool -list -v -keystore ~/.android/vela-release.keystore`.
3. Deploy getvela.app, then verify with Google's
   [Statement List Tester](https://developers.google.com/digital-asset-links/tools/generator)
   or `adb shell pm get-app-links app.getvela.wallet`.

> Keep the debug fingerprint (`24:EA:D0:…`) for local dev builds — that one is fine.

### A2. iOS — sign with Team ID `F9W689P9NE` (already configured correctly)
The Apple App Site Association is **already correct**:
`getvela.app/.well-known/apple-app-site-association` →
`webcredentials.apps = ["F9W689P9NE.app.getvela.VelaWallet"]`, which matches the
bundle id `app.getvela.VelaWallet`. **Action:** just make sure the iOS build is
signed with the **F9W689P9NE** team and the `webcredentials:getvela.app`
associated domain is in the provisioning profile (the entitlement is committed in
`app-ios/VelaWallet/VelaWallet/VelaWallet.entitlements`; the App ID must have
Associated Domains enabled). Then passkeys work on device. No code change needed
here.

---

## B. Product / security decisions — all three settled, no call needed

The three questions this section asked were answered while the new shells were
built. They are kept with their answers, because each one is a decision someone
taking the project over would otherwise have to make again.

### B1. iOS iCloud KV entitlement — ✅ removed
The old config plugin emitted `com.apple.developer.ubiquity-kvstore-identifier`
for a `vela-cloud-sync` module that was never wired. Neither exists in the
SwiftUI shell: `app-ios/VelaWallet/VelaWallet/VelaWallet.entitlements` carries
only `com.apple.developer.associated-domains` (`webcredentials:getvela.app`,
`applinks:getvela.app`) and `com.apple.security.smartcard` (the CTAP-over-CCID
security-key path, spec 019). There is no cloud sync and no iCloud capability to
enable, so an Archive cannot fail codesign over it.

### B2. Android `allowBackup` — ✅ `false`
`app-android/vela-wallet/app/src/main/AndroidManifest.xml` sets
`android:allowBackup="false"`. Private keys live in the platform authenticator,
not in app prefs, so this is defence in depth — but the account list, contacts
and history no longer leave the device through Google backup.

### B3. Duplicate passkeys on re-registration (iOS) — ✅ excluded
`Features/Onboarding/Core/PasskeyExecutor.swift` sets `excludedCredentials` on
**both** the platform request and the security-key request. The reason is
stronger than "don't mint a second passkey": a wallet's address is derived from
*every* founding key, so a provider that silently replaced an earlier credential
would produce a different address. (This is why the iOS deployment target is
17.4.) Multi-account is intentional and lives in the account switcher, not in a
second accidental registration.

---

## C. Small code follow-ups I deferred (low severity)

- **Camera "Open Settings" escape** — when a user *permanently* denies camera,
  the "Grant permission" button can't re-prompt. **Done on iOS**: the scanner's
  grant button calls `UIApplication.openSettingsURLString` when
  `camera.refusal == .denied` (`App/RootView.swift`,
  `Features/Explore/ExploreScreen.swift`), under the label
  `componentsUi.scanner.grantPermission` — which, like
  `onboarding.create.openSettings`, is translated in all 15 locales. **Still
  open on Android**: `VelaNavHost.kt` renders the same label with
  `componentsUi.scanner.grantPermission`, but nothing anywhere in
  `app-android/` starts `Settings.ACTION_APPLICATION_DETAILS_SETTINGS`, so a
  permanent denial is still a dead end. The translation already exists, so this
  is a Kotlin-only change. (User can still close the scanner via the X / Back.)
- **~~iOS `isSupported()`~~** — moot: that was a native-module API of the Expo
  tree. The SwiftUI shell has no such function; availability is decided by the
  per-method matrix from spec 019.
- **Login "no wallet yet" UX** — on iOS, "no passkey on this device" surfaces as
  a cancellation (`PasskeyFailure(kind: .cancelled)`), which is a platform limit,
  not a bug. Whether the login screen should route that to a "Create a wallet"
  prompt rather than a silent no-op has not been re-checked on the SwiftUI shell.
  Needs device testing.

---

## D. Device verification matrix (please run on real hardware)

Please confirm on a device build of the native shells — `app-ios/VelaWallet`
from Xcode on a real iPhone, and `app-android/vela-wallet` via
`./gradlew :app:installDebug` on a real phone (see
`docs/project-takeover/02-local-development.md`):

**iOS (real iPhone, iOS 17.4+ — the deployment target, see B3):**
- [ ] Create wallet → passkey sheet appears over the foreground window
- [ ] Sign a tx / dApp request → passkey sheet appears and resolves
- [ ] Cancel the passkey sheet → app shows a soft "cancelled", not a hard error
- [ ] Scan a QR with the camera, and decode one from the **photo library**
- [ ] Open a dApp in the in-app browser → connect → sign (spec 053)

**Android (real device, incl. Android 13+; loop and tooling in
`docs/android/install-verify-loop.md`):**
- [ ] After A1 is done: create wallet + sign on a **release-signed** build —
      this is the only check that exercises Digital Asset Links
- [ ] Name the wallet with a quote, e.g. `My "Main" Wallet` → registration succeeds
- [ ] QR scanner open → press hardware **Back** → scanner closes
- [ ] Send screen: focus the amount/recipient field → keyboard doesn't cover it
- [ ] Open a dApp in the in-app browser → connect → sign (spec 044)
- [ ] A device with no Google Play services: creation falls back to USB / caBLE
      (spec 019)

**Both:**
- [ ] App boots and reaches the welcome screen with no crash on a cold start
