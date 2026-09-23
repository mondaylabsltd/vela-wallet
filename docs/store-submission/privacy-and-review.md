
# Store Submission — Privacy Forms & App Review Notes

Copy-paste-ready answers for the **Apple App Privacy (Nutrition Label)**, **Google Play Data Safety** form, and the **App Review notes** for both stores.

Everything below is grounded in the actual app behavior, the published privacy policy (getvela.app/privacy), and the service code.
Operator / data controller: **MONDAY LABS LTD**, UK.

> ⚠️ These forms are legal attestations you personally sign. I've flagged the genuine judgment calls with **【你来定】**. Everything else is a factual mapping of what the code does.

> 🔴 **Re-checked 2026-09-23 against the service code. See [`privacy-evidence.md`](privacy-evidence.md) — it supersedes this file wherever the two disagree.**
> The audit overturned this sheet's central premise. It said Vela's own servers do not store the wallet address. They do:
> the relay **logs the sender address on every accepted transaction** (`vela-relay-cf/src/admission.rs:203-205`) and stores
> the full signed UserOperation for up to **14 days** (`lane_do.rs:1400-1412`, `:1784-1787`); the passkey index stores the
> wallet address and the user-typed wallet/key names for **7–30 days** (`p256-index-cf/src/submitter.rs:399-411`, `:57-58`)
> and then **publishes them permanently to Gnosis Chain** (`p256-registrar/src/protocol.rs:27,88`). Declaring Identifiers →
> User ID and Financial Info → Other Financial Info, both **Linked**, is now the *accurate* answer, not the cautious one.
> The other corrections are marked ⚠️ inline below.

---

## 0. Data inventory (the source of truth both forms derive from)


| Data                                                                     | Leaves device?                  | To whom                                                | Real-identity link                    | Purpose                                 | Req/Opt                                 |
| -------------------------------------------------------------------------- | --------------------------------- | -------------------------------------------------------- | --------------------------------------- | ----------------------------------------- | ----------------------------------------- |
| Passkey**public key**                                                    | Yes                             | Vela Passkey Index → Gnosis chain (publicly readable) | No — pseudonymous, cannot move funds | Wallet creation + cross-device recovery | Required                                |
| **Account name** (user-chosen label)                                     | Yes                             | Vela Passkey Index (off-chain + on-chain)              | No — user told to keep pseudonymous  | Identify wallet during recovery         | Required                                |
| **Wallet address** ⚠️                                                     | Yes (in queries / signed tx)    | Third-party RPC nodes, **Vela Relay (logged + stored ≤14d)**, **Vela Index (stored 7–30d, then published on-chain forever)** | No — on-chain pseudonymous           | Read balances, submit transactions      | Required (functional)                   |
| **Transaction content** ⚠️ (calldata: recipients + amounts, signature, receipt logs) | Yes | **Vela Relay — stored ≤14 days** (`lane_do.rs:1400-1412`) | No — pseudonymous | Submit and retry the transaction | Required (functional) |
| **Token contract addresses held** ⚠️                                      | Yes (one logo GET per token)    | `ethereum-data.getvela.app` (static assets + Cloudflare edge) | No — but the path sequence approximates holdings | Render token logos | Functional |
| **IP address**                                                           | Yes (implicit in every request) | RPC / Relay / Index / chain-data / rates; Cloudflare as processor | No — relay reads none at all; index keeps a salted 64-bit hash ≤60 s for rate limits | Network connectivity, rate-limiting     | Functional                              |
| **Diagnostics** (app version, OS version, language, RPC-failure metrics) ⚠️ | Only if user files a bug — **and then only by the user's own browser, not by the app** | github.com (public issue), via a prefilled URL the OS browser opens | No — no address/keys/balances       | Bug fixing                              | Optional, user-initiated, preview shown |
| Private keys / seed phrases / tx contents                                | **Never leaves device**         | —                                                     | —                                    | —                                      | Not collected                           |
| Name / email / phone / gov ID                                            | **Never asked**                 | —                                                     | —                                    | —                                      | Not collected                           |
| Contacts, browsing history, settings, RPC prefs                          | No — on-device store (`vela.*` keys) | —                                                     | —                                    | —                                      | Not collected                           |

**No tracking anywhere.** No advertising ID, no third-party analytics SDK, no crash reporter, no cross-app tracking — proven from the dependency manifests of all four shells (privacy-evidence.md §4). iOS ships exactly one third-party package (`lottie-ios`); Android's merged manifest contains **no `com.google.android.gms.permission.AD_ID`**.

`NSPrivacyTracking=false` is set in **`app-ios/VelaWallet/VelaWallet/PrivacyInfo.xcprivacy`** (the old `ios/…` path belonged to the Expo tree, where no manifest ever existed). That file **is** on `main` — it landed with PR #309 (`505dc3e2`); an earlier reading of this said otherwise because it was checking a stale local `main`. What still needs doing is reconciling its contents with §1 below: `Linked` must be `true`, and a second `NSPrivacyCollectedDataTypeUserID` entry is needed, because a manifest that disagrees with the nutrition label is itself a rejection reason (privacy-evidence.md §6.3). The same file declares the one required-reason API the archived binary imports (`NSPrivacyAccessedAPICategoryUserDefaults`, reason `CA92.1`), and `.github/workflows/ios-package.yml` re-measures the archive with `nm -u` on every build.

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

**B. Financial Info → Other Financial Info** ⚠️ *(the judgement is settled — declare it)*

- Collected: **Yes**
- Linked: **Yes** · Tracking: **No** · Purpose: **App Functionality**
- What it is: the **wallet address and the transactions sent from it**.
- ⚠️ **The old rationale here was wrong.** It said "Vela's own servers do not store the wallet address." They do. The relay logs the sender on every accepted submit (`vela-relay-cf/src/admission.rs:203-205`) and stores the full signed UserOperation — sender, calldata with recipients and amounts, signature, and the on-chain receipt logs — for up to **14 days** (`lane_do.rs:1400-1412`, `:1784-1787`). Declaring is now the accurate answer, not the conservative one, and the published privacy policy already says the same thing (`privacy/+page.svelte:110-117`), so a reviewer can check it in two minutes.
- The remaining judgement is only about the **RPC nodes** (partners or user-directed infrastructure?) — see privacy-evidence.md §6.5 #3. It changes the review-notes wording, not what gets ticked.

**C. Diagnostics → Other Diagnostic Data** ⚠️ *(changed to Not Collected)*

- Collected: **No** · (Crash Data: **No** — there is no crash reporter.)
- ⚠️ **No shipped store binary transmits diagnostics.** A repo-wide grep for `api/bug-report` finds **zero call sites** in `rust/`, `app-android/`, `app-ios/`, `app-desktop/`, `app-web/vela-wallet/`. iOS's feedback sheet has **no send action at all** (`SettingsSheet.swift:307-330`); Android builds a prefilled `github.com/…/issues/new` URL and hands it to the **system browser** (`VelaNavHost.kt:1693-1699`) — the app itself sends nothing, and the user reviews the body first. Android's local `VelaLog` is a no-op in release builds (`VelaLog.kt:37`).
- **If** the in-app reporter is ever wired to `getvela.app/api/bug-report`, flip this to Collected / Not Linked / App Functionality **in that same release** — and re-implement the scrubbing server-side first, because the client that guaranteed "never keys, addresses or balances" was deleted with the Expo tree.

### Declare these as NOT collected (verify each is "Not Collected" in the form):

Contact Info (name/email/phone/address), Health, Location, Sensitive Info, Contacts, Browsing/Search History, Purchases, Payment Info (credit cards — you have none), Audio/Photos/Video content, Gameplay, and any Advertising/Tracking identifiers.

### Export compliance (separate question, every build)

- "Does your app use encryption?" → **Yes**, but only **standard/exempt** encryption (HTTPS/TLS + OS-provided passkey crypto). Qualifies for the exemption.
- ✅ **Done (2026-09-23).** It had regressed: the key lived in the Expo `app.json`, which spec 039 deleted, and the old "✅ Done" was describing that. `ITSAppUsesNonExemptEncryption = false` is now in the hand-maintained `app-ios/VelaWallet/VelaWallet/Info.plist`, so App Store Connect stops asking on every upload.

### Account deletion (Guideline 5.1.1(v))

- In-app path exists: **Settings → Erase this device** clears on-device data. ⚠️ **Erasing is not yet complete on iPhone** (`privacy/+page.svelte:172-177`) — do not claim a full in-app deletion path on the iOS form until it is; uninstalling removes the rest.
- Caveat to put in review notes: the registry record on Gnosis is **append-only and permanent** (`p256-registrar/src/protocol.rs:5-6`) and contains the **public key, credential ID, AAGUID, wallet address, wallet name and each key's label** — pseudonymous, cannot move funds, and deletable by nobody, including us. The off-chain copy expires by itself (7 days after success, 30 after failure) and can be removed on request at **hello@mondaylabs.ltd**. Don't hide this — explain it.

---

## 2. Google Play — Data Safety form (Play Console → App content → Data safety)

**Q: Does your app collect or share any required user data types?** → **Yes**

### Security practices

- **All user data encrypted in transit?** → **Yes** (HTTPS/TLS everywhere).
- **Do you provide a way to request data deletion?** → **Yes** — Settings → Erase this device, or uninstall, removes on-device data; the off-chain index record expires on its own (7 d after success, 30 d after failure) and is removable on request at **hello@mondaylabs.ltd**. ⚠️ The on-chain registry record is **append-only and cannot be deleted by anyone** — say so rather than implying full erasure.
- **Independent security review?** → **No.** ⚠️ Do **not** claim an audit — none exists and none is scheduled.
- **Committed to Play Families policy?** → **No** (not directed at children).

### Data types — declare COLLECTED:


| Play data type                                                           | Collected | Shared                       | Ephemeral?                         | Req/Opt      | Purpose                               |
| -------------------------------------------------------------------------- | ----------- | ------------------------------ | ------------------------------------ | -------------- | --------------------------------------- |
| **Financial info → Other financial info** (wallet address + the transactions sent from it) | Yes       | **Yes**                      | **No** — relay stores the operation 1 h–14 d; the address is published on-chain | Required     | App functionality, Account management |
| **Personal info → User IDs** (wallet address as account id, passkey public key, credential ID) | Yes | **Yes** (published on-chain) | No                                 | Required     | Account management, App functionality |
| **Personal info → Other info** (user-chosen wallet name and per-key labels)                | Yes       | **Yes** (published on-chain) | No                                 | Required     | Account management, App functionality |
| ~~**App info & performance → Diagnostics**~~ ⚠️                          | **No**    | —                            | —                                  | —            | — (no shipped binary transmits diagnostics — see §1.C) |

- ⚠️ **"Shared" is no longer a judgment call — it is Yes.** The index publishes the wallet address, the wallet name, every key label, every public key and every credential ID to **Gnosis Chain, permanently** (`p256-registrar/src/protocol.rs:27,88`; `p256-index-cf/src/chain.rs:465-470`). That is unambiguously public sharing and it covers the address too, not just the public key. Whether address-to-RPC *alone* would count no longer matters, because the on-chain publication already forces the answer. (If `ALCHEMY_API_KEY` is set on the relay or index, Alchemy is an additional recipient — confirm from the secret store, privacy-evidence.md §7 #3.)

### Data types — declare NOT collected:

Location, Contacts, Calendar, Photos/Videos, Audio (no `RECORD_AUDIO`), SMS/Call logs, Health, **Device or other IDs** (no advertising ID, no device ID — `AD_ID` is absent from the merged manifest), Web browsing history (the in-app browser's history stays on device — `browser_history.rs:11-16`), Installed apps, App activity.

- ⚠️ **Location:** still **Not collected**, but be ready to explain the permission. `ACCESS_FINE_LOCATION` is declared at `maxSdkVersion="30"` and `BLUETOOTH_SCAN` carries `neverForLocation` — both exist only because pre-API-31 BLE scanning required them, for the caBLE "sign in with your phone" passkey flow. No location value is ever read or transmitted. (`AndroidManifest.xml:59-71`)
- **IP address 【你来定】:** the relay reads **no** client IP at all; the index keeps a salted, 64-bit-truncated hash for a **60-second** rate-limit window and never stores the raw value (`p256-index-cf/src/edge.rs:758-766`, `submitter.rs:630-649`). Cloudflare, as processor, sees it regardless. Google lets you treat purely-ephemeral connection data as not collected. **Recommend: do not declare** as collected, keep the privacy-policy mention.

---

## 3. Google Play — other "App content" declarations (don't forget these)

- **Financial features declaration** → declare: **"Provides a non-custodial crypto wallet (stores/holds crypto)."** Do **NOT** check exchange/buy/sell/trade — Vela has no fiat on-ramp and no exchange. Exchange checkboxes can trigger regional licensing requirements you don't need.
- **Target audience & content** → adults (18+); not designed for or appealing to children.
- **Content rating (IARC questionnaire)** → finance utility, no objectionable content. Answer honestly; expect a low rating with a finance note.
- ⚠️ **Permissions** → this is no longer "Camera only". The shipped manifest declares `INTERNET`, `CAMERA`, `VIBRATE`, `POST_NOTIFICATIONS`, `BLUETOOTH_SCAN` (`neverForLocation`), `BLUETOOTH_CONNECT`, and legacy `BLUETOOTH` / `BLUETOOTH_ADMIN` / `ACCESS_FINE_LOCATION` capped at `maxSdkVersion="30"` (`AndroidManifest.xml:26-71`). Bluetooth and the capped location permission came back with spec 019's caBLE passkey flow. Microphone and draw-over-apps are still gone. None of these is in Play's *high-risk / sensitive permissions* set (that list is SMS, Call Log, `MANAGE_EXTERNAL_STORAGE`, `AccessibilityService`, `QUERY_ALL_PACKAGES`, full-screen intent), so no special declaration form is triggered — but do not tell a reviewer the app has no Bluetooth.
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

DAPPS (optional): the app has a built-in browser. Connecting to a site and signing
happen in-app; there is no pairing step and no separate companion app.

BLUETOOTH: used for one thing only — the standard WebAuthn/caBLE "sign in with
your phone" flow, where this device shows a QR code and the phone that scans it
is reached over a Bluetooth proximity channel. It is optional; a passkey on this
device needs no Bluetooth. The usage string explains it at the first scan.

BACKGROUND TASK: the identifier app.getvela.VelaWallet.tracker is the
transaction-receipt tracker (it checks whether a submitted transaction confirmed).
It is not an advertising or analytics tracker; the app contains neither.

COMPLIANCE (Guideline 3.1.5(b)): Vela is storage-only / self-custodial. It does
NOT facilitate cryptocurrency exchange or trading, has NO fiat on-ramp, NO in-app
purchases, and NO mining.

ENCRYPTION: standard HTTPS/TLS + OS passkey cryptography only (exempt).

PRIVACY: see https://getvela.app/privacy, which describes every server the app
talks to. The app contains no analytics SDK, no crash reporter and no advertising
identifier, and never asks for a name, email, phone number or ID.

Creating a wallet writes a public record (passkey public keys, credential IDs,
the wallet address and the name the user chose) to an append-only registry
contract on Gnosis Chain. That is how the wallet can be recovered on another
device. It is pseudonymous, it cannot move funds, and — because it is on a public
blockchain — nobody, including us, can delete it. The user is told this before
they create a wallet. Account/data deletion: Settings → Erase this device clears
on-device data; deletion requests for our own off-chain records (which expire by
themselves within 7–30 days): hello@mondaylabs.ltd.

Note: the marketing website getvela.app uses a cookieless analytics tool. The app
does not — no analytics reaches this binary.

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
on-ramp, no in-app purchases. dApps are used through the app's built-in browser.

Bluetooth + the capped ACCESS_FINE_LOCATION (maxSdkVersion=30) exist for one
purpose: the standard WebAuthn/caBLE "sign in with your phone" passkey flow,
where this device shows a QR code and reaches the scanning phone over a Bluetooth
proximity channel. BLUETOOTH_SCAN carries neverForLocation; no location is ever
read or transmitted.

Privacy policy: https://getvela.app/privacy
Contact / data deletion: hello@mondaylabs.ltd
```

> If the Google Play developer account is a **personal** account created after late 2023, production access first requires **closed testing with 20 testers for 14 continuous days**. Organization accounts are exempt — confirm which you have before planning the timeline.

---

## 6. Repo changes made so the forms stay true (2026-06-30) — ⚠️ HISTORICAL, PARTLY REVERSED

> ⚠️ **Read the 2026-09-23 note first.** This section describes the Expo-era binary. Three of its claims are no longer true of what ships:
> - **Bluetooth is back**, on both platforms, for the caBLE passkey flow (spec 019): `AndroidManifest.xml:59-71`, `app-ios/VelaWallet/VelaWallet/Info.plist:93-94`.
> - **`ACCESS_FINE_LOCATION` is back**, capped at `maxSdkVersion="30"`, as a pre-API-31 BLE-scan prerequisite.
> - **`ITSAppUsesNonExemptEncryption` was gone** — it lived in `app.json`, which spec 039 deleted — and is back in `Info.plist` as of 2026-09-23.
>
> Still true: no `RECORD_AUDIO`, no `SYSTEM_ALERT_WINDOW`, no microphone usage string, and the deletion channel.

Bluetooth was dropped entirely (dApp Connect then ran over the WalletPair HTTPS/WebSocket relay), along with the other unneeded permissions:

- ✅ Removed all `BLUETOOTH*` permissions + iOS Bluetooth usage strings + the `bluetooth-peripheral` background mode (`app.json`, `plugins/with-native-modules.js`, committed `Info.plist` + `AndroidManifest.xml`).
- ✅ Removed `RECORD_AUDIO` (disabled expo-camera mic via `recordAudioAndroid:false` + strip `NSMicrophoneUsageDescription`), `ACCESS_FINE_LOCATION` (only existed for BLE scanning), and `SYSTEM_ALERT_WINDOW`.
- ✅ Added `ITSAppUsesNonExemptEncryption=false`.
- ✅ Fixed the generic iOS photo-library usage string.
- ✅ Deleted legacy BLE code: `DAppScreen.tsx`, `(tabs)/dapps.tsx`, `src/modules/ble`, `walletpair-ble-transport.ts`, and native modules `modules/vela-ble` + `modules/walletpair-ble`; removed the BLE branch from `walletpair-transport.ts`.
- ✅ Deletion-request channel set to **hello@mondaylabs.ltd** across both forms (also add it to the privacy policy).

**Follow-up, restated for the native shells (2026-09-11, spec 039):** the Expo app and its config plugins are gone, so there is no prebuild step any more — the Xcode and Gradle projects under `app-ios/` and `app-android/` are hand-maintained. Before a store build, verify the binary directly: `Info.plist` has no `NSBluetooth*` / `UIBackgroundModes` / `NSMicrophoneUsageDescription` beyond what the shell actually uses, and `AndroidManifest.xml` has no `BLUETOOTH*` / `ACCESS_FINE_LOCATION` / `RECORD_AUDIO` / `SYSTEM_ALERT_WINDOW` beyond what the passkey transports need (the caBLE path on Android does use Bluetooth and location — see spec 019 and the OnePlus device notes before deciding what to declare). The `exp+vela-wallet` scheme, `_expo._tcp` Bonjour and the "Expo Dev Launcher" strings no longer exist in any build.

```

```
