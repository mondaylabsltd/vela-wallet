# Native (iOS / Android) launch checklist — items that need you

Context: the app was only ever tested as the **Expo Web** build. A 2026-07-01
cross-platform audit found the native-only defects below. The low-risk, code-only
fixes are **already done** (see "Fixed in code"). This doc is the remainder —
things I can't do for you because they need your **Apple/Google accounts**, a
**real device**, or a **product decision**.

---

## Fixed in code (no action needed, but verify on device — see §D)

| Fix | Where |
|-----|-------|
| Hermes polyfills: `crypto.getRandomValues` + `btoa/atob` + `Buffer` | `src/polyfills.ts` (+ `.web.ts` no-op), loaded first in `src/app/_layout.tsx` |
| Android passkey `register()` JSON now escaped (`JSONObject`/`JSONArray`) | `modules/vela-passkey/android/.../VelaPasskeyModule.kt` |
| QR scanner Android hardware-Back dismiss (`onRequestClose`) | `src/components/QRScanner.tsx` |
| Android keyboard avoidance (`behavior` no longer `'height'`) + `softwareKeyboardLayoutMode: resize` | `src/components/ui/ScreenContainer.tsx`, `app.json` |
| Android coroutine scope cancelled via `invalidate()` (New-Arch-safe) | `VelaPasskeyModule.kt` |
| iOS passkey presentation anchor → foreground-active key window | `modules/vela-passkey/ios/VelaPasskeyModule.swift` |
| iOS passkey error mapping (`.canceled` + `.notInteractive` → CANCELLED) | `VelaPasskeyModule.swift` |

Regression tests: `src/__tests__/polyfills.test.ts` (behavioral) and
`src/__tests__/native-fixes.test.ts` (source guards).

---

## A. Blockers requiring your Apple / Google accounts

### A1. 🔴 Android release signing + Play App Signing cert (launch-blocking)
**Problem:** `android/app/build.gradle` signs the `release` build with the **debug
keystore**, and the config plugin (`plugins/with-native-modules.js:398`) forces
`~/.android/debug.keystore`. When you upload the `.aab`, **Google Play re-signs it
with a Google-managed key** whose SHA-256 is in neither entry of
`getvela.app/.well-known/assetlinks.json`. Android Credential Manager then fails
Digital Asset Links verification → `createCredential`/`getCredential` throw →
**users can't create a wallet or sign anything.** Passkeys are the only auth path.

**What you need to do:**
1. Create a real **upload keystore** (or let EAS manage credentials) and add a
   `release` `signingConfig` in gradle that uses it — not `signingConfigs.debug`.
2. Enroll in **Play App Signing** (default for new apps). After the first upload,
   open **Play Console → Test and release → App integrity → App signing key
   certificate** and copy the **SHA-256**.
3. Put that SHA-256 (and the upload key's SHA-256) into
   `getvela.app/src/routes/.well-known/assetlinks.json/+server.ts`
   `sha256_cert_fingerprints`. The current `A3:8E:36:FE:…` "Release keystore"
   value corresponds to no keystore in this repo — replace/augment it.
4. Redeploy getvela.app, then verify with Google's
   [Statement List Tester](https://developers.google.com/digital-asset-links/tools/generator)
   or `adb shell pm get-app-links app.getvela.wallet`.

> Keep the debug fingerprint (`24:EA:D0:…`) for local dev builds — that one is fine.

### A2. iOS — sign with Team ID `F9W689P9NE` (already configured correctly)
The Apple App Site Association is **already correct**:
`getvela.app/.well-known/apple-app-site-association` →
`webcredentials.apps = ["F9W689P9NE.app.getvela.VelaWallet"]`, which matches the
bundle id `app.getvela.VelaWallet`. **Action:** just make sure the iOS build is
signed with the **F9W689P9NE** team and the `webcredentials:getvela.app`
associated domain is in the provisioning profile (the config plugin adds the
entitlement; the App ID must have Associated Domains enabled). Then passkeys work
on device. No code change needed here.

---

## B. Product / security decisions (I need your call before changing)

### B1. iOS iCloud KV entitlement — enable capability or remove?
`plugins/with-native-modules.js:55` emits
`com.apple.developer.ubiquity-kvstore-identifier`, but **no JS code uses it** and
the `vela-cloud-sync` iOS module isn't wired to JS. If the App ID doesn't have
**iCloud (Key-Value storage)** enabled, a Release **Archive can fail codesign**.
- **Option A (recommended for now):** remove the entitlement + the unused
  `vela-cloud-sync` iOS module until it's actually consumed.
- **Option B:** enable the iCloud KV capability on the App ID and regenerate the
  profile (needed if you intend to ship cloud sync soon).
👉 Which do you want?

### B2. Android `allowBackup` for wallet data
The plugin sets `android:allowBackup="true"`, so app prefs get auto-uploaded to
the user's Google backup. Conventional for wallets is `false` (avoid any chance of
sensitive state leaving the device via cloud backup). Note: private keys live in
the OS keystore via passkeys, not app prefs — so this is defense-in-depth, but the
`vela-cloud-sync` module muddies it.
👉 Set `allowBackup=false`, or keep `true` because cloud-sync depends on it?

### B3. Duplicate passkeys on re-registration (iOS)
`register()` sets no `excludeCredentials`, so running "Create Wallet" twice mints a
second passkey → a second address, which can shadow a funded one. Proper fix needs
either (a) passing the device's known credential IDs into `register()` to exclude
them (JS→native API change), or (b) a JS gate in the Create flow that blocks a
second registration when a local account already exists.
👉 Do you want single-account-per-device (gate it), or intentional multi-account?
This drives the design.

---

## C. Small code follow-ups I deferred (low severity)

- **Camera "Open Settings" escape** — when a user *permanently* denies camera,
  the "Grant permission" button can't re-prompt. Adding an "Open Settings" button
  (`Linking.openSettings()`) needs a new i18n key across **all 14 locales**; I left
  it out to avoid a partial/English-only string (your translation-quality rule).
  Say the word and I'll add the key + translations. (User can still close the
  scanner via the X / Android Back.)
- **iOS `isSupported()`** returns `true` on any iOS 16+ device. That's actually
  fine (platform passkeys exist on all iOS 16+), so I left it. No action unless
  you want a biometric-enrolled check.
- **Login "no wallet yet" UX** — on iOS, "no passkey on this device" surfaces as
  `CANCELLED` (platform limitation). Consider routing a login-screen `CANCELLED`
  to a "Create a wallet" prompt rather than a silent no-op. Needs device testing.

---

## D. Device verification matrix (please run on real hardware)

The Kotlin/Swift/keyboard fixes compile-clean in my reasoning but I **cannot build
native here** — please confirm on a device build (`npx expo run:ios` /
`run:android`, or an EAS build):

**iOS (real iPhone, iOS 16+):**
- [ ] Create wallet → passkey sheet appears (verifies presentation-anchor fix)
- [ ] Sign a tx / dApp request → passkey sheet appears and resolves
- [ ] Cancel the passkey sheet → app shows a soft "cancelled", not a hard error
- [ ] WalletPair (Vela Connect): scan a pairing QR → connects (verifies polyfills)
- [ ] Scan a QR from the **photo library** → decodes (verifies Buffer polyfill)

**Android (real device, incl. Android 13+):**
- [ ] After A1 is done: create wallet + sign (verifies assetlinks/signing)
- [ ] Name the wallet with a quote, e.g. `My "Main" Wallet` → registration succeeds
      (verifies JSON-escaping fix)
- [ ] QR scanner open → press hardware **Back** → scanner closes (verifies `onRequestClose`)
- [ ] Send screen: focus the amount/recipient field → keyboard doesn't cover it
      (verifies keyboard-avoidance change)
- [ ] WalletPair connect + gallery QR decode (same polyfill checks as iOS)

**Both:**
- [ ] App boots without a redbox referencing `crypto`, `btoa`, or `Buffer`
