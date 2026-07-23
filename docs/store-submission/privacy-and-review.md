
# Store Submission — Privacy Forms & App Review Notes

Copy-paste-ready answers for the **Apple App Privacy (Nutrition Label)**, **Google Play Data Safety** form, and the **App Review notes** for both stores.

Everything below is grounded in the actual app behavior (`package.json` has **no analytics/crash/tracking SDK**), the published privacy policy (getvela.app/privacy), and the bug-report code (`src/services/bug-report.ts`). Operator / data controller: **MONDAY LABS LTD**, UK.

> ⚠️ These forms are legal attestations you personally sign. I've flagged the 3 genuine judgment calls with **【你来定】**. Everything else is a factual mapping of what the code does.

---

## 0. Data inventory (the source of truth both forms derive from)


| Data                                                                     | Leaves device?                  | To whom                                                | Real-identity link                    | Purpose                                 | Req/Opt                                 |
| -------------------------------------------------------------------------- | --------------------------------- | -------------------------------------------------------- | --------------------------------------- | ----------------------------------------- | ----------------------------------------- |
| Passkey**public key**                                                    | Yes                             | Vela Passkey Index → Gnosis chain (publicly readable) | No — pseudonymous, cannot move funds | Wallet creation + cross-device recovery | Required                                |
| **Account name** (user-chosen label)                                     | Yes                             | Vela Passkey Index (off-chain + on-chain)              | No — user told to keep pseudonymous  | Identify wallet during recovery         | Required                                |
| **Wallet address**                                                       | Yes (in queries / signed tx)    | Third-party RPC nodes, Vela Relay                    | No — on-chain pseudonymous           | Read balances, submit transactions      | Required (functional)                   |
| **IP address**                                                           | Yes (implicit in every request) | RPC / Bundler / Passkey Index / getvela.app            | No — ephemeral, connection only      | Network connectivity, rate-limiting     | Functional                              |
| **Diagnostics** (app version, OS version, language, RPC-failure metrics) | Only if user files a bug        | getvela.app → GitHub issue                            | No — scrubbed, no address/keys       | Bug fixing                              | Optional, user-initiated, preview shown |
| Private keys / seed phrases / tx contents                                | **Never leaves device**         | —                                                     | —                                    | —                                      | Not collected                           |
| Name / email / phone / gov ID                                            | **Never asked**                 | —                                                     | —                                    | —                                      | Not collected                           |
| Balances / tx history / RPC prefs                                        | No — on-device`AsyncStorage`   | —                                                     | —                                    | —                                      | Not collected                           |

**No tracking anywhere.** No advertising ID, no third-party analytics SDK, no cross-app tracking. `NSPrivacyTracking=false` already set in `ios/VelaWallet/PrivacyInfo.xcprivacy`.

Website analytics (cookieless, self-hosted) covers **getvela.app**, not the app — it is **not** part of either app-store data form. It belongs only in the privacy policy (already there).

---

## 1. Apple — App Privacy (App Store Connect → App Privacy)

For every category Apple asks: *Collected? · Linked to the user? · Used for tracking?* **Used for Tracking = No, for every single item.**

### Declare these as COLLECTED:

**A. Identifiers → User ID**

- Collected: **Yes**
- Linked to the user: **Yes** (it identifies the wallet/account — though pseudonymous, with no real-world identity)
- Used for tracking: **No**
- Purpose: **App Functionality** (account creation + cross-device recovery)
- What it is: the passkey **public key** and the user-chosen **account name** stored on the Passkey Index.

**B. Financial Info → Other Financial Info** 【你来定】

- Collected: **Yes** (recommended)
- Linked: **Yes** · Tracking: **No** · Purpose: **App Functionality**
- What it is: the **wallet address**, transmitted to third-party RPC nodes / the bundler to read balances and submit transactions.
- **Judgment call:** Vela's own servers do **not** store the wallet address — it's only sent in-transit to third-party infrastructure to make the wallet work. Apple still expects disclosure of data handled by integrated third parties, so declaring it (App Functionality, not tracking) is the safe, honest answer. Choosing *not* to declare is defensible only if you treat RPC nodes purely as user-directed infrastructure. **Recommend: declare it.**

**C. Diagnostics → Other Diagnostic Data** (and Crash Data if you ever add crash capture — you don't today)

- Collected: **Yes** · Linked: **No** · Tracking: **No** · Purpose: **App Functionality**
- What it is: optional, user-initiated bug reports (app version, OS version, language, RPC-failure metrics). Scrubbed of keys/addresses; user sees a preview before sending.

### Declare these as NOT collected (verify each is "Not Collected" in the form):

Contact Info (name/email/phone/address), Health, Location, Sensitive Info, Contacts, Browsing/Search History, Purchases, Payment Info (credit cards — you have none), Audio/Photos/Video content, Gameplay, and any Advertising/Tracking identifiers.

### Export compliance (separate question, every build)

- "Does your app use encryption?" → **Yes**, but only **standard/exempt** encryption (HTTPS/TLS + OS-provided passkey crypto). Qualifies for the exemption.
- ✅ **Done:** `"ITSAppUsesNonExemptEncryption": false` is now set in `app.json` → `ios.infoPlist` and the committed `ios/VelaWallet/Info.plist`, so it stops asking every upload.

### Account deletion (Guideline 5.1.1(v))

- In-app path exists: **Settings → Remove/Reset wallet** clears all on-device data.
- Caveat to put in review notes: the Passkey Index **public key is written to an immutable public blockchain** (Gnosis) — it's pseudonymous and cannot move funds, so it cannot be "deleted." The off-chain account-name record can be removed on request at **hello@mondaylabs.ltd**. Don't hide this — explain it.

---

## 2. Google Play — Data Safety form (Play Console → App content → Data safety)

**Q: Does your app collect or share any required user data types?** → **Yes**

### Security practices

- **All user data encrypted in transit?** → **Yes** (HTTPS/TLS everywhere).
- **Do you provide a way to request data deletion?** → **Yes** — uninstall removes on-device data; account-name record removable on request at **hello@mondaylabs.ltd**. (On-chain public key is immutable + pseudonymous — note it.)
- **Independent security review?** → **No.** ⚠️ Do **not** claim an audit — none exists and none is scheduled.
- **Committed to Play Families policy?** → **No** (not directed at children).

### Data types — declare COLLECTED:


| Play data type                                                           | Collected | Shared                       | Ephemeral?                         | Req/Opt      | Purpose                               |
| -------------------------------------------------------------------------- | ----------- | ------------------------------ | ------------------------------------ | -------------- | --------------------------------------- |
| **Financial info → Other financial info** (wallet public key / address) | Yes       | **Yes** 【你来定】           | No (public key is stored on-chain) | Required     | App functionality, Account management |
| **Personal info → User IDs / Other info** (account name)                | Yes       | **Yes** (published on-chain) | No                                 | Required     | Account management, App functionality |
| **App info & performance → Diagnostics** (+ Other app performance data) | Yes       | No                           | No (becomes a GitHub issue)        | **Optional** | App functionality (bug fixing)        |

- **"Shared" judgment call 【你来定】:** Google's definition of *Shared* **excludes** transfers to "service providers processing on your behalf" and "user-initiated" transfers — which could cover RPC nodes/bundler. **But** the Passkey Index publishes the public key + account name to a **public blockchain**, which is unambiguously public sharing. So mark the public key + account name as **Shared**. Wallet-address-to-RPC alone is the borderline part; marking it shared is the conservative honest choice.

### Data types — declare NOT collected:

Location, Contacts, Calendar, Photos/Videos, Audio (the unused `RECORD_AUDIO` permission has been removed), SMS/Call logs, Health, **Device or other IDs** (no advertising ID, no device ID), Web browsing history, Installed apps.

- **IP address 【你来定】:** used only for connectivity + in-memory rate-limiting (not stored, not linked). Google lets you treat purely-ephemeral connection data as not collected; it's disclosed in the privacy policy. **Recommend: do not declare** as collected, keep the privacy-policy mention.

---

## 3. Google Play — other "App content" declarations (don't forget these)

- **Financial features declaration** → declare: **"Provides a non-custodial crypto wallet (stores/holds crypto)."** Do **NOT** check exchange/buy/sell/trade — Vela has no fiat on-ramp and no exchange. Exchange checkboxes can trigger regional licensing requirements you don't need.
- **Target audience & content** → adults (18+); not designed for or appealing to children.
- **Content rating (IARC questionnaire)** → finance utility, no objectionable content. Answer honestly; expect a low rating with a finance note.
- **Permissions** → **Camera (QR scanning) only.** Bluetooth, location, microphone, and the draw-over-apps permission have all been removed from the app, so **no high-risk permissions declaration form is required.**
- **Ads** → app contains **no ads** → declare "No ads."
- **Government / News / COVID** → N/A.

---

## 4. App Review notes — Apple (App Store Connect → App Review Information → Notes)

Paste this (English — review is English-primary):

```
Vela Wallet is a self-custodial, non-custodial Ethereum/EVM smart-account wallet.
Published by MONDAY LABS LTD (organization account).

NO DEMO ACCOUNT NEEDED — there is no username/password and no server-side account.
A wallet is created on-device using a passkey (Face ID / Touch ID). The passkey
private key is held by iCloud Keychain; we never see it.

HOW TO TEST:
1. Launch the app → tap "Create Wallet" → authenticate with Face ID / Touch ID.
   This creates a passkey and a Safe smart-account address.
2. The portfolio shows zero balances until the wallet is funded (this is normal
   for a fresh self-custodial wallet).
3. Tap "Receive" to view the wallet address / QR.
4. Sending requires on-chain funds. To exercise Send, you may send a small amount
   to the displayed address, OR contact us and we will pre-fund the review wallet.
   We can also provide a testnet build on request.

DAPP CONNECT (optional): pairs with a desktop dApp by scanning a pairing QR code
(WalletConnect-style relay over HTTPS/WebSocket). No Bluetooth. It is optional and
not needed to use the wallet. Steps/extension link available on request.

COMPLIANCE (Guideline 3.1.5(b)): Vela is storage-only / self-custodial. It does
NOT facilitate cryptocurrency exchange or trading, has NO fiat on-ramp, NO in-app
purchases, and NO mining.

ENCRYPTION: standard HTTPS/TLS + OS passkey cryptography only (exempt).

PRIVACY: see https://getvela.app/privacy. The app contains no analytics SDK and
collects no personal identity data. Account/data deletion: Settings → Remove
Wallet clears all on-device data; the on-chain passkey public key is immutable,
pseudonymous, and cannot move funds. Deletion requests: hello@mondaylabs.ltd.

Contact for review: hello@mondaylabs.ltd
```

---

## 5. App Review / pre-launch notes — Google Play

```
Vela Wallet — self-custodial, non-custodial EVM smart-account wallet by MONDAY LABS LTD.

No login/account: a wallet is created on-device with a passkey (device biometrics
via Android Credential Manager / Google Password Manager). No demo credentials needed.

Test flow: Create Wallet → biometric/passkey → portfolio (empty until funded) →
Receive shows the address. Send needs on-chain funds; contact us to pre-fund or for
a testnet build.

Crypto: non-custodial software wallet (store/hold only). No exchange, no fiat
on-ramp, no in-app purchases. dApp Connect pairs over an HTTPS/WebSocket relay
(scan a QR code) — no Bluetooth.

Privacy policy: https://getvela.app/privacy
Contact / data deletion: hello@mondaylabs.ltd
```

> If the Google Play developer account is a **personal** account created after late 2023, production access first requires **closed testing with 20 testers for 14 continuous days**. Organization accounts are exempt — confirm which you have before planning the timeline.

---

## 6. Repo changes already made so the forms stay true (2026-06-30)

Bluetooth was dropped entirely (dApp Connect now runs over the WalletPair HTTPS/WebSocket relay), along with the other unneeded permissions:

- ✅ Removed all `BLUETOOTH*` permissions + iOS Bluetooth usage strings + the `bluetooth-peripheral` background mode (`app.json`, `plugins/with-native-modules.js`, committed `Info.plist` + `AndroidManifest.xml`).
- ✅ Removed `RECORD_AUDIO` (disabled expo-camera mic via `recordAudioAndroid:false` + strip `NSMicrophoneUsageDescription`), `ACCESS_FINE_LOCATION` (only existed for BLE scanning), and `SYSTEM_ALERT_WINDOW`.
- ✅ Added `ITSAppUsesNonExemptEncryption=false`.
- ✅ Fixed the generic iOS photo-library usage string.
- ✅ Deleted legacy BLE code: `DAppScreen.tsx`, `(tabs)/dapps.tsx`, `src/modules/ble`, `walletpair-ble-transport.ts`, and native modules `modules/vela-ble` + `modules/walletpair-ble`; removed the BLE branch from `walletpair-transport.ts`.
- ✅ Deletion-request channel set to **hello@mondaylabs.ltd** across both forms (also add it to the privacy policy).

**Required follow-up to make the committed native projects fully BLE-free:** run a clean prebuild so the Xcode/Gradle projects regenerate without the deleted native modules:

```
npx expo prebuild --clean
(cd ios && pod install)
```

After prebuild, re-verify the binary: `Info.plist` has no `NSBluetooth*`/`UIBackgroundModes`/`NSMicrophoneUsageDescription`, and `AndroidManifest.xml` has no `BLUETOOTH*`/`ACCESS_FINE_LOCATION`/`RECORD_AUDIO`/`SYSTEM_ALERT_WINDOW`. Note: a production prebuild should also exclude `expo-dev-client` so the `exp+vela-wallet` scheme, `_expo._tcp` Bonjour, and the "Expo Dev Launcher" local-network string don't ship.

```

```
