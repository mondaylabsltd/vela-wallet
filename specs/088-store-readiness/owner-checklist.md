# 088 — Owner checklist: Google Play testing track + TestFlight

Everything here needs your accounts, your keys or a deploy. Do it in this order;
each step names what it unblocks. Code-side, branch `088-store-readiness` is ready
([results.md](results.md)). Sources: [audit.md](audit.md) §4 C,
[research-requirements.md](research-requirements.md).

**Timing, read first.** Google Play reviews an app's **first** release even on the
internal track — "a few hours or up to seven days". Testers may not have the
Android build tomorrow even if you upload it tomorrow. TestFlight **internal**
testing has no review: a build is testable once it has processed. **External**
TestFlight waits for Beta App Review on its first build (usually under a day; no SLA).

**Build numbers.** Both apps now use the number of commits behind HEAD:
`git rev-list --count HEAD` (3217 on this branch as of its last code commit; it
only grows along `main`). Override on Android with `-PvelaVersionCode=N` or
`VELA_VERSION_CODE=N`; on iOS pass `CURRENT_PROJECT_VERSION=N` to `xcodebuild`.
Every upload must be higher than any earlier one for the same app — check step 0.

---

## 0. Before anything (both stores, today)

- [ ] **Merge `088-store-readiness`** (after review) and build from that commit.
- [ ] **Expo-era uploads.** The retired Expo tree used EAS with remote auto-increment.
  - App Store Connect → the app → TestFlight: note the highest build number under any version.
  - Play Console → the app (if it exists) → App bundle explorer: note the highest versionCode.
  - If either is ≥ the commit count, pass a higher number explicitly (see "Build numbers").

## 1. Apple — TestFlight (internal tomorrow, external later)

1. [ ] **Agreements.** As Account Holder, sign in at developer.apple.com and App Store Connect
   and accept every pending agreement — the updated Program License Agreement (EU
   Attachment 14) took effect **2026-10-01**. An unaccepted agreement is commonly reported
   to block uploads.
2. [ ] **App record.** App Store Connect → Apps: the app for bundle id
   `app.getvela.VelaWallet`, team **MONDAY LABS LTD (F9W689P9NE)**. Create it if it is missing
   (name, primary language, SKU).
3. [ ] **Archive** (Xcode 26.x; this machine has 26.3):
   ```bash
   cd app-ios/VelaWallet
   xcodebuild archive -project VelaWallet.xcodeproj -scheme VelaWallet -configuration Release \
     -destination 'generic/platform=iOS' -archivePath ~/Desktop/Vela.xcarchive -allowProvisioningUpdates \
     CURRENT_PROJECT_VERSION="$(git rev-list --count HEAD)" VELA_GIT_COMMIT="$(git rev-parse --short=7 HEAD)"
   ```
   (Rebuild the core first if `app-ios/VelaCoreKit/Artifacts` is stale:
   `bash rust/scripts/build-ios-xcframework.sh`.)
4. [ ] **Upload.** Xcode → Window → Organizer → the archive → **Distribute App → App Store
   Connect → Upload**. Or from the terminal, `xcodebuild -exportArchive` with
   `method = app-store-connect`, `destination = upload`, `teamID = F9W689P9NE`,
   `signingStyle = automatic`.
   - **The first time, macOS asks for your login keychain password** so `codesign` can use
     the "Apple Distribution: MONDAY LABS LTD" key. Choose **Always Allow**. The 088 dry run
     stopped exactly there, because only you can answer it (results.md §6).
5. [ ] **Export compliance.** It is answered in the build: `ITSAppUsesNonExemptEncryption = NO`.
   The plist comment says why — TLS and OS passkey crypto, plus the Rust core's AES/AES-GCM
   used only inside authentication protocols. It is your declaration to make; get counsel's
   view before the production release.
6. [ ] **Internal testers.** TestFlight → Internal Testing → add a group with up to 100 App
   Store Connect users. Testable as soon as processing finishes.
7. [ ] **Try a TestFlight install yourself:** create a wallet, receive, send, open a dApp,
   open a `velawallet://open?url=https://…` link (the sheet must name the host), and the
   **USB security key** route. The `com.apple.security.smartcard` entitlement is dropped at
   signing, so if the security key does not work there, drop the security-key line from the
   App Store text (`docs/store-submission/store-listing-copy.md`, owner check in §1).
8. [ ] **Before external testing** (Beta App Review):
   - Test Information: Beta App Description, feedback email, review contact (name, email,
     phone), **sign-in required = No**, review notes from
     `docs/store-submission/privacy-and-review.md` §4, privacy policy URL
     `https://getvela.app/privacy`.
   - **Demo video:** record 2–3 minutes (create → receive → send → dApp connect and sign),
     upload it unlisted, and replace every `DEMO VIDEO:` placeholder in that doc.
   - Deploy the site (step 3 below) first: the notes link `/support` and `/delete`.
9. [ ] **Pricing and Availability:** opt out of **Mac (Apple silicon)** and **Apple Vision Pro**
   until passkeys, camera and Bluetooth are tested there.
10. [ ] **Later, for the App Store** (not needed for TestFlight): App Privacy label matching
    `PrivacyInfo.xcprivacy` (decide the I11 call on wallet and key names), age rating
    (Unrestricted Web Access = Yes → 16+; override to **18+** to match the terms), 6.9″ iPhone
    screenshots, Support URL `https://getvela.app/support`, EU DSA trader status, the Paid Apps
    agreement plus tax and banking, and the price.

## 2. Google — Play internal testing (tomorrow), then closed testing

1. [ ] **Account type.** Play Console → Settings → Developer account: it must be an
   **Organization** account (D-U-N-S, verified website). Crypto software wallets must be
   published by an Organization. A Personal account is the wrong type, and would also need
   12 testers opted in for 14 days before production.
2. [ ] **Create the app.** Name "Vela Wallet", app (not game). **Choose Paid now if it will ever
   be paid.** A free app can never become paid, and setting a price needs a payments profile,
   which can be linked to the account only once. Internal testers install a paid app free;
   closed and open testers have to buy it.
3. [ ] **App content.** Do all of it before the first release; the first release is reviewed,
   and the review checks the mandatory declarations.
   - Privacy policy: `https://getvela.app/privacy`.
   - App access: instructions and video link from privacy-and-review.md §5.
   - Ads: **No**.
   - Content rating (IARC): "Unrestricted internet" = **Yes**.
   - Target audience: **18 and over**.
   - Financial features: **Cryptocurrency wallet**. It is non-custodial; tick no exchange boxes.
   - Health: none. Government: No.
   - Data safety: answers in privacy-and-review.md §2, deletion URL `https://getvela.app/delete`.
     Internal-only apps are exempt; closed testing needs it.
4. [ ] **Upload key.** Check that `~/.android/vela-release.keystore` (alias `vela-release`) is the
   key you mean to upload with, and whether it is the `A3:8E:36:FE:…` "Release keystore" that
   `assetlinks.json` already lists:
   `keytool -list -v -keystore ~/.android/vela-release.keystore` (it asks for your password).
5. [ ] **Build the signed bundle** (the build refuses to run without the key):
   ```bash
   cd app-android/vela-wallet
   export VELA_UPLOAD_STORE_FILE=~/.android/vela-release.keystore VELA_UPLOAD_KEY_ALIAS=vela-release
   read -rs VELA_UPLOAD_STORE_PASSWORD; export VELA_UPLOAD_STORE_PASSWORD
   read -rs VELA_UPLOAD_KEY_PASSWORD;   export VELA_UPLOAD_KEY_PASSWORD
   ./gradlew :app:bundleRelease
   jarsigner -verify -verbose:summary -certs app/build/outputs/bundle/release/app-release.aab
   ```
   You can put the four values in `app-android/vela-wallet/keystore.properties` (gitignored)
   instead: `storeFile=`, `storePassword=`, `keyAlias=`, `keyPassword=`.
   This cross-compiles the core (about 6 minutes cold). Add `-PvelaSkipRustBuild` only if
   `app/src/main/jniLibs` was just rebuilt by `rust/scripts/build-android.sh`.
6. [ ] **Play App Signing.** At the first release, either let Play generate the app-signing key
   (the default), or choose "use my own key" and give Play the key behind `A3:8E:…`, which is
   already in `assetlinks.json`. Either works. Only the first skips nothing: you must do step 8.
7. [ ] **Internal testing release.** Testing → Internal testing → Create release → upload
   `app-release.aab` → release notes → Save → Review release → **Start rollout**. Add testers
   (email list, up to 100) and send them the opt-in link. Expect the first review to take
   hours to days.
8. [ ] **Right after the upload — passkeys depend on this.** Test and release → App integrity →
   **App signing key certificate → SHA-256**. Add it as a new line in
   `app-web/getvela.app/src/routes/.well-known/assetlinks.json/+server.ts`, in the
   `app.getvela.wallet` entry's `sha256_cert_fingerprints`: after line 24 (the `BE:1E:…` line —
   add a comma after it), with a `// Play app-signing key` comment. Add the upload key's SHA-256
   too if it is not `A3:8E:…`. Deploy the site (step 3), then check with Google's
   [Statement List Tester](https://developers.google.com/digital-asset-links/tools/generator)
   for `getvela.app` + `app.getvela.wallet` + `get_login_creds`.
   **Until this is live, nobody who installs from Play can create a wallet or sign in.**
   If you use Internal app sharing, add that key's SHA-256 too.
9. [ ] **Before closed testing:** Store listing: 512×512 icon (export `docs/design/icon/app-icon.svg`),
   a 1024×500 feature graphic, at least 2 phone screenshots, the short and full descriptions
   from `docs/store-submission/store-listing-copy.md`, and a contact email. Data safety (step 3).

## 3. Site — getvela.app (you deploy)

- [ ] From the merged tree: `cd app-web/getvela.app && bun install && bun run deploy`.
- [ ] Check that these return 200 and read right: `/support`, `/delete`, `/privacy` (the erase
  paragraph now covers iPhone), and `/.well-known/assetlinks.json` (after step 2.8, it carries
  Play's key).

## 4. After the first installs

- [ ] Android tester on the **Play-installed** build: create a wallet with a passkey. If it fails
  with "RP ID cannot be validated", step 2.8 is not live yet.
- [ ] iPhone tester on TestFlight: create a wallet, then sign.
- [ ] Keep relay, index and chain-data watched while reviews run (Apple 2.1).
