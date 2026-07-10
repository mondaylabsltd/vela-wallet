# R1 Spike — Increment 1: Packaging Skeleton (on-device runbook)

**Goal:** get a Safari Web Extension target that **builds, installs on a physical iPhone, appears in Safari settings, and injects a content script that logs to the console** (visible via Safari Web Inspector) — with an App Group entitlement on both the app and the extension target. **Nothing else** (no App Group I/O, no native messaging, no signing). This de-risks packaging (`@bacons/apple-targets` on SDK 55) before any logic.

**Environment required:** Xcode 16 + macOS 15 Sequoia + CocoaPods ≥ 1.16.2 (Ruby 3.2.x). A physical iPhone. Local Xcode build (no EAS).

**What's already in the repo (written for you):**
- `packages/safari-extension/` — the web-extension source (`src/manifest.json`, `src/content.js`, `src/inpage.js`, `src/popup.html`), `build.mjs` (esbuild → `targets/safari/assets/`), `package.json`, and `expo-target.config.template.js` (our canonical target config).
- `plugins/with-native-modules.js` — patched to add App Group `group.app.getvela.wallet` to the **app** target's entitlements (alongside the existing `webcredentials:getvela.app`).

**What you run (creates the tool-owned `targets/safari/` and builds):** below.

> **✅ STATUS — Steps 1–3 already done & verified by tooling (no device needed), 2026-07-05:**
> dependency install (`@bacons/apple-targets@4.0.7`), Safari target assembled under `targets/safari/`, App Group entitlement on **both** targets, `app.json` plugin added (last), esbuild web bundle built, **`expo prebuild -p ios` PASSED on SDK 55** (the #1 risk — cleared), **`pod install` completed** (workspace `ios/VelaWallet.xcworkspace` generated), and **`ios.appleTeamId: "F9W689P9NE"` set** — both targets sign non-interactively (verified `DEVELOPMENT_TEAM = F9W689P9NE` on app + extension, `CODE_SIGN_STYLE = Automatic`).
> **Do NOT run `npx create-target safari`** — it refuses this repo's dynamic `app.config.js` and scaffolds nothing. The target was hand-assembled (handler from the tool's template + our `expo-target.config.js` + our `assets/`) and is now version-controlled. Steps 1–3 below are kept for reference only.
> **Your remaining work: Step 0 (Apple portal) + Steps 4–5 (Xcode signing + device run).**

---

## Step 0 — Apple Developer portal (one-time, no iPhone needed)

1. Identifiers → **App Groups** → **+** → create `group.app.getvela.wallet`.
2. Identifiers → **App IDs** → `app.getvela.VelaWallet` → enable **App Groups** → assign the group → Save.
3. The extension App ID `app.getvela.VelaWallet.safari` doesn't exist yet — Xcode's **Automatic** signing will create it and register the group on the first device run (Step 5). (Manual fallback: create that App ID, enable App Groups, assign the same group.)

---

## Step 1 — Scaffold the Safari target

```bash
cd /Volumes/data/production/vela-wallet

# Pin the plugin exactly (create-target would install it too, but this pins the version):
npm i -D -E @bacons/apple-targets@4.0.7

# Scaffold targets/safari/ (Info.plist, SafariWebExtensionHandler.swift, template assets,
# and an expo-target.config.js). It also adds "@bacons/apple-targets" to app.json plugins.
npx create-target safari
```

**Verify after create-target:**
- `app.json` → `expo.plugins` contains **exactly one** `"@bacons/apple-targets"`, and it is the **last** entry (after `"./plugins/with-release-signing"`). Remove any duplicate; move it last if needed.
- `targets/safari/` now exists with `Info.plist`, `SafariWebExtensionHandler.swift`, `expo-target.config.js`, `assets/`.

> Do **not** hand-edit `targets/safari/Info.plist` or `SafariWebExtensionHandler.swift` — the tool owns them (the handler stays dormant until Increment 4).

---

## Step 2 — Apply our target config + build our web assets

Order matters: **overwrite the config and build AFTER `create-target`** (create-target's copy would otherwise clobber ours).

```bash
cd /Volumes/data/production/vela-wallet

# Our target config (App Group entitlement + deploymentTarget) over the empty generated one:
cp packages/safari-extension/expo-target.config.template.js targets/safari/expo-target.config.js

# (optional cleanup) template extras we don't use in Inc 1:
rm -f targets/safari/assets/background.js
rm -rf targets/safari/assets/_locales

# Build our web bundle into targets/safari/assets/ (overwrites template manifest/content.js):
cd packages/safari-extension && npm i && npm run build && cd ../..
```

After this, `targets/safari/assets/` contains **our** `manifest.json`, `content.js`, `inpage.js`, `popup.html` (plus the template `images/`, which is harmless).

---

## Step 3 — Prebuild + open Xcode

```bash
cd /Volumes/data/production/vela-wallet
npx expo prebuild -p ios --clean   # --clean is REQUIRED after touching a target / config / app.json
xed ios                            # opens ios/velawallet.xcworkspace
```

---

## Step 4 — Signing (already config-driven — no Xcode Team-clicking)

`ios.appleTeamId: "F9W689P9NE"` is in `app.json`, so `expo prebuild` writes `DEVELOPMENT_TEAM = F9W689P9NE` onto **both** targets with `CODE_SIGN_STYLE = Automatic`. **You do not set the Team in Xcode.** (F9W689P9NE is the app's org team — the same one the passkey AASA `webcredentials.apps = ["F9W689P9NE.app.getvela.VelaWallet"]` is bound to; see `docs/NATIVE-LAUNCH-CHECKLIST.md` A2.)

**Two prerequisites:**
1. Xcode signed in with an Apple ID that is a **member of team F9W689P9NE** (Xcode → Settings → Accounts). Automatic signing then creates/updates certs + provisioning profiles.
2. **The App Group `group.app.getvela.wallet` MUST be registered and assigned to both App IDs (Step 0) — this is NOT optional.** CLI automatic signing (`expo run:ios` / `xcodebuild`) does **not** create a new App Group *identifier*; it only regenerates profiles once the capability already exists. **Verified failure without Step 0:** `expo run:ios` → *"Provisioning Profile … does not support the App Groups capability … doesn't support the group.app.getvela.wallet App Group"* on both targets. Do Step 0 via the portal (~3 min, one-time), **or** open `ios/VelaWallet.xcworkspace` in Xcode.app once (the Signing & Capabilities pane can register the group when you have admin rights), **or** headlessly via an App Store Connect API key (Fastlane `produce` / EAS). After it's registered, re-run `npx expo run:ios --device` — automatic signing regenerates the profiles and the build proceeds.

---

## Step 5 — Run on device, enable, inspect

1. **CLI (no Xcode GUI):** plug in the iPhone, then `npx expo run:ios --device` — pick your device; it builds, signs (auto), installs, and launches. *(Or open `ios/VelaWallet.xcworkspace` and Run.)* If prompted, trust the profile: iPhone **Settings → General → VPN & Device Management**.
2. iPhone **Settings → Apps → Safari → Extensions → Vela Wallet → toggle ON** (some builds: **Settings → Safari → Extensions**). Set permission **Allow** for the test site (or All Websites).
3. iPhone **Settings → Safari → Advanced → Web Inspector = ON**.
4. Mac **Safari → Settings → Advanced → "Show features for web developers"** (enables the Develop menu).
5. Connect iPhone via USB. On the iPhone open an https page (e.g. `https://example.com`). Mac **Safari → Develop → [your iPhone] → [the page]** → open its console.

---

## ✅ Increment 1 PASS criteria (all must hold)

> **VERIFIED ON DEVICE 2026-07-05** — both logs observed in Safari Web Inspector on a physical iPhone (`Vela R1 injected @2026-07-05T11:28:57.329Z` + `Vela R1 inpage running in page context`). Packaging path de-risked. Proceeding to Increment 2.

- App **builds and installs** on the physical iPhone; the extension appears under **Settings → Safari → Extensions** as "Vela Wallet".
- With the extension enabled + granted, loading an https page shows in **that page's** Web Inspector console:
  - `Vela R1 injected @<ISO timestamp>` (content-script world), then
  - `Vela R1 inpage running in page context` (page/MAIN world — proves the `<script>` injection).
- Both the **app** and **safari** targets show the `group.app.getvela.wallet` App Group in Xcode → Signing & Capabilities.
- Nothing else runs (no I/O, no native messaging, no signing).

---

## Most likely failure points → fixes

- **appex missing manifest/JS (highest risk, apple-targets #143).** Symptom: extension enables but no console log ever appears. In the synchronized-folder model there is **no manual "Copy Bundle Resources" phase to drag into** — membership is the folder itself. Fix: ensure the files physically exist in `targets/safari/assets/` **before** `expo prebuild` (re-run `npm run build` in `packages/safari-extension`), and **re-run the build after every `--clean` prebuild**. Keep everything flat under `assets/` (never rename to `Resources/`).
- **Deployment-target mismatch (most common build failure).** Symptom: link/compile error on the extension. Fix: set `deploymentTarget` in `targets/safari/expo-target.config.js` to match the app's `IPHONEOS_DEPLOYMENT_TARGET` (read `ios/Podfile`), then `--clean` prebuild again.
- **App Group codesign failure** ("Provisioning profile doesn't include the application-groups entitlement"). Fix: complete Step 0 for **both** App IDs, or use **Automatic** signing on both targets and re-run.
- **Extension absent from Settings.** It only appears after a successful device install of a build containing the `.appex`. Rebuild; toggle Web Inspector + site permission.
- **Empty Web Inspector console.** Web Inspector off on device; or you selected the **extension/service-worker context** instead of **the page** (Inc 1 logs are page-level — pick the page under Develop); or you didn't grant the extension on **that** site.
- **prebuild toolchain errors.** Confirm Xcode 16 / macOS 15 / CocoaPods ≥1.16.2 / Ruby 3.2.x. Recovery: `rm -rf ios && npx expo prebuild -p ios --clean`; if still failing `watchman watch-del-all; rm -rf ~/Library/Developer/Xcode/DerivedData` and retry. To isolate whether `@bacons/apple-targets` is the culprit vs an unrelated pod failure, temporarily drop `"@bacons/apple-targets"` from `app.json` plugins and re-prebuild.
- **Fingerprint/OTA trap (apple-targets #145):** changes under `targets/` may not bump the `runtimeVersion.policy: "fingerprint"` hash. Treat any extension change as needing a **fresh native build**, never an OTA update.

---

## Verified facts baked in (from tool-source inspection)

- `@bacons/apple-targets@4.0.7` uses Xcode-16 `PBXFileSystemSynchronizedRootGroup`; `targets/safari/assets/` links as the extension's Resources. `safari` type → `com.apple.Safari.web-extension`, principal class `$(PRODUCT_MODULE_NAME).SafariWebExtensionHandler`.
- `safari` does **not** auto-add App Groups — hence the two explicit declarations (app via `plugins/with-native-modules.js`, extension via `expo-target.config.js`). App Group id `group.app.getvela.wallet` is identical in both. Generated entitlements land at `targets/safari/generated.entitlements`.
- Extension bundle id defaults to `app.getvela.VelaWallet.safari` (child of the app id). Manifest is MV3, content script at `document_start`, `inpage.js` in `web_accessible_resources`.
- SDK 55 is **not** positively confirmed by the tool author (peer is `expo >=52`); Step 3 is where you'd discover an incompatibility — the isolation fix above tells them apart.

---

## Next increments (not in this skeleton)

- **Inc 2 — App Group echo:** a shared `AppGroupStore.swift` (container URL + atomic JSON file + `NSFileProtection`) compiled into **both** targets; the handler writes a file, the app reads it. Proves FACT-2 (App Group I/O both processes).
- **Inc 3 — gesture launch + deep link:** injected UI button → synchronous `velawallet://sign?rid=` (FACT-1); new `src/app/sign.tsx` route + inbound deep-link handling.
- **Inc 4 — return poll + fake sign:** handler writes a **fake** result + Darwin notification; content script polls **via the native handler, not the background** (FACT-3); resolve the page promise. This is where **R1** is actually measured (see `ARCHITECTURE.md` §12.5 GO/NO-GO).
