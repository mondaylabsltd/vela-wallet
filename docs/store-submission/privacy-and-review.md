
# Store Submission — Privacy Forms & App Review Notes

Copy-paste-ready answers for the **Apple App Privacy (Nutrition Label)**, **Google Play Data Safety** form, and the **App Review notes** for both stores.

Everything below is grounded in the actual app behavior, the published privacy policy (getvela.app/privacy), and the service code.
Operator / data controller: **MONDAY LABS LTD**, UK.

> These forms are legal attestations you personally sign. The genuine judgement calls are flagged **【你来定】** (owner decision). Everything else is a factual mapping of what the code does.

> **Re-checked 2026-09-23 against the service code. See [`privacy-evidence.md`](privacy-evidence.md) — it supersedes this file wherever the two disagree.**
> The audit overturned this sheet's central premise. It said Vela's own servers do not store the wallet address. They do:
> the relay **logs the sender address on every accepted transaction** (`vela-relay-cf/src/admission.rs:203-205`) and stores
> the full signed UserOperation for up to **14 days** (`lane_do.rs:1400-1412`, `:1784-1787`); the passkey index stores the
> wallet address and the user-typed wallet/key names for **7–30 days** (`p256-index-cf/src/submitter.rs:399-411`, `:57-58`)
> and then **publishes them permanently to Gnosis Chain** (`p256-registrar/src/protocol.rs:27,88`). Declaring Identifiers →
> User ID and Financial Info → Other Financial Info, both **Linked**, is the *accurate* answer, not the cautious one.

> **Revised 2026-10-01 (spec 088, store readiness).** What changed, against `specs/088-store-readiness/audit.md`:
> - Removed every offer of a "testnet build on request". All 24 built-in networks are mainnets (claim ledger C-net-1); there is no testnet build (B7).
> - Bluetooth is described as the code does it: one use, the caBLE / hybrid "sign in with another device" flow (§3, §3b, §4, §5). The Trusted Signer's Bluetooth peripheral channel that the iOS purpose string still names was **removed on both platforms on 2026-09-23** (commit `0801564b7`: `AndroidBlePeripheral.kt` and `BLUETOOTH_ADVERTISE` deleted; no `CBPeripheralManager` anywhere in `app-ios`). See the pre-flight check below (I18).
> - New §3a, *How a reviewer creates a wallet*, for both stores, with a demo-video placeholder (B7, research §G7, §A1).
> - New §3b, *Features a reviewer will meet*, for Apple 2.3.1(a) (B7).
> - Deletion rewritten: Settings → Erase This Device exists on iPhone **and** Android; server retention and the permanent on-chain record are stated; `https://getvela.app/delete` and `https://getvela.app/support` are the public URLs (B1, B4).
> - Export compliance rewritten to name the Rust core's AES and the authentication carve-out (I7).
> - Guideline citation 3.1.5(b) → **3.1.5(i)** (research §A2).
> - Play's 12-tester rule replaces the outdated "20 testers" (research §G2).
> - The I11 judgement call (wallet name and key labels on iOS) is flagged as an owner decision in §1, not silently changed.

### Pre-flight before pasting any of this

1. **The build includes spec 088 FR-004** (done on branch `088-store-readiness`): an external `velawallet://open` link shows the site's host and asks before anything loads, only `https` pages are asked about, and the wallet provider is offered only to secure contexts — https, or http on the device's own loopback (Android and iOS; the rule is the core's `is_secure_context`). §3b and §4 describe that behaviour; a build from before it makes the notes wrong.
2. **The iOS Bluetooth purpose string matches the code (I18)** — fixed on the same branch: `Info.plist` now reads *"Vela uses Bluetooth to reach the nearby phone or tablet you sign in with — the one that scans the code on screen."* Archive from a commit that has it, or the permission prompt and the review notes disagree (5.1.1(ii)).
3. **Android passkeys work on the Play-signed build (A5).** Play's app-signing certificate SHA-256 must be in `getvela.app/.well-known/assetlinks.json`, deployed, before the release is sent for review. Without it the reviewer cannot create a wallet.
4. **`https://getvela.app/delete` and `https://getvela.app/support` are deployed** (B1, B4). Their source, and the corrected erase paragraph on `/privacy` (B2), are on branch `088-store-readiness`; deploying getvela.app is yours.
5. **The demo video link is filled in** wherever `DEMO VIDEO:` appears.
6. **Relay, index and chain-data are up and watched** while the review runs (Apple 2.1).

---

## 0. Data inventory (the source of truth both forms derive from)


| Data                                                                     | Leaves device?                  | To whom                                                | Real-identity link                    | Purpose                                 | Req/Opt                                 |
| -------------------------------------------------------------------------- | --------------------------------- | -------------------------------------------------------- | --------------------------------------- | ----------------------------------------- | ----------------------------------------- |
| Passkey **public key**, credential ID, authenticator model (AAGUID)       | Yes                             | Vela Passkey Index → Gnosis chain (publicly readable) | No — pseudonymous, cannot move funds | Wallet creation + cross-device recovery | Required                                |
| **Wallet name and per-key labels** (user-chosen)                         | Yes                             | Vela Passkey Index (off-chain 7–30 d) → Gnosis chain (permanent) | No — user is told it goes on-chain (`onboarding.create.ack0`) | Identify the wallet during recovery | Required                                |
| **Wallet address**                                                        | Yes (in queries / signed tx)    | Third-party RPC nodes, **Vela Relay (logged + stored ≤14d)**, **Vela Index (stored 7–30d, then published on-chain forever)** | No — on-chain pseudonymous           | Read balances, submit transactions      | Required (functional)                   |
| **Transaction content** (calldata: recipients + amounts, signature, receipt logs) | Yes | **Vela Relay — stored ≤14 days** (`lane_do.rs:1400-1412`) | No — pseudonymous | Submit and retry the transaction | Required (functional) |
| **Token contract addresses held**                                        | Yes (one logo GET per token)    | `ethereum-data.getvela.app` (static assets + Cloudflare edge) | No — but the path sequence approximates holdings | Render token logos | Functional |
| **IP address**                                                           | Yes (implicit in every request) | RPC / Relay / Index / chain-data / rates; Cloudflare as processor | No — relay reads none at all; index keeps a salted 64-bit hash ≤60 s for rate limits | Network connectivity, rate-limiting     | Functional                              |
| **Hybrid passkey traffic** ("Phone or tablet" only)                       | Yes, end-to-end encrypted        | Over Bluetooth (L2CAP) to the nearby phone, or through the phone platform's standard caBLE tunnel server (`cable.ua5v.com` / `cable.auth.com`, `rust/crates/vela-core/src/cable/tunnel_domain.rs:13`) | No — Noise-encrypted (`cable/noise.rs`); the tunnel operator cannot read it | Create or use a passkey on another phone | Optional, user-chosen method |
| **Bug report** (typed description + steps; diagnostics: app version, platform, language, unreachable network names, recent failure summaries; ≤ 5 screenshots) | Only when the user taps Send in Settings → Send feedback — **the app POSTs it** (since 078, 2026-09-27) | `getvela.app/api/bug-report` → a **public** GitHub issue; screenshots stored in Cloudflare R2 until deleted, shown in the issue. Fallback: a prefilled GitHub form in the OS browser | No — addresses/URLs scrubbed on device and on the server; screenshots re-encoded on device (no EXIF/GPS); no account id | Bug fixing | Optional, user-initiated, preview + "screenshots are public" shown |
| Private keys / seed phrases                                              | **Never leaves device** (a synced passkey is held, end-to-end encrypted, by the user's passkey provider, never by Vela) | —                                                     | —                                    | —                                      | Not collected                           |
| Name / email / phone / gov ID                                            | **Never asked**                 | —                                                     | —                                    | —                                      | Not collected                           |
| Contacts, browsing history, settings, RPC prefs                          | No — on-device store (`vela.*` keys) | —                                                     | —                                    | —                                      | Not collected                           |

**No tracking anywhere.** No advertising ID, no third-party analytics SDK, no crash reporter, no cross-app tracking — proven from the dependency manifests of all four shells (privacy-evidence.md §4). iOS ships exactly one third-party package (`lottie-ios`); Android's merged manifest contains **no `com.google.android.gms.permission.AD_ID`**.

`NSPrivacyTracking=false` is set in **`app-ios/VelaWallet/VelaWallet/PrivacyInfo.xcprivacy`**. Its collected types match §1 below — Other Financial Info and User ID (Linked), plus the bug report's Other Diagnostic Data, Customer Support and Photos or Videos (Not Linked) — and must keep matching: a manifest that disagrees with the nutrition label is itself a rejection reason (privacy-evidence.md §6.3). The same file declares the required-reason APIs the archived binary imports (`NSPrivacyAccessedAPICategoryUserDefaults` `CA92.1`, `NSPrivacyAccessedAPICategoryFileTimestamp` `C617.1`), and `.github/workflows/ios-package.yml` re-measures the archive with `nm -u` on every build.

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
- What it is: the wallet address, the passkey **public keys** and credential IDs, and — unless you choose option (a) in the I11 box below — the user-chosen **wallet name and key labels**, all stored on the Passkey Index and published on Gnosis.

**B. Financial Info → Other Financial Info** *(the judgement is settled — declare it)*

- Collected: **Yes**
- Linked: **Yes** · Tracking: **No** · Purpose: **App Functionality**
- What it is: the **wallet address and the transactions sent from it**.
- The relay logs the sender on every accepted submit (`vela-relay-cf/src/admission.rs:203-205`) and stores the full signed UserOperation — sender, calldata with recipients and amounts, signature, and the on-chain receipt logs — for up to **14 days** (`lane_do.rs:1400-1412`, `:1784-1787`). The published privacy policy says the same thing (`privacy/+page.svelte:110-117`), so a reviewer can check it in two minutes.
- The remaining judgement is only about the **RPC nodes** (partners or user-directed infrastructure?) — see privacy-evidence.md §6.5 #3. It changes the review-notes wording, not what gets ticked.

**C. The in-app bug report — three types, all Collected / NOT Linked / not tracking / App Functionality** *(both apps send it since branch 078)*

- **Diagnostics → Other Diagnostic Data:** the report's device lines (app version, platform, language, names of unreachable networks, recent failure summaries on Android).
- **User Content → Customer Support:** what the person types — what happened, steps to reproduce.
- **User Content → Photos or Videos:** up to five screenshots the person picks (PhotosPicker / Android Photo Picker, out of process — no photo permission), re-encoded on the device so no EXIF or location leaves it, stored in Cloudflare R2 until deleted and shown in the public issue.
- Crash Data: **No** — there is no crash reporter.
- Why **not linked**: the report carries no wallet address, key or account id (addresses and URLs are scrubbed on the device and again on the server), and nothing on the server ties a report to a wallet. Why **not the optional-disclosure exemption**: one of Apple's conditions is that the user's name or account name appears in the submission form; this form is deliberately anonymous.
- `PrivacyInfo.xcprivacy` declares the same three (Linked=false). Evidence: privacy-evidence.md §3a.

> **【你来定】 I11 — the wallet name and key labels on iOS.** They are free text the person types (`registry_metadata.rs` `key_names`), stored by the index and published on-chain. Play declares them separately as *Personal info → Other info* (§2). On the Apple side they are currently folded into *Identifiers → User ID*, and *Other User Content* is listed as Not collected. Pick one story for both stores:
> - **(a) Declare them on iOS too:** *User Content → Other User Content* — Collected, **Linked**, not tracking, App Functionality. Then, in the **same build**, add a matching `NSPrivacyCollectedDataTypeOtherUserContent` entry (Linked `true`, Tracking `false`, purpose App Functionality) to `PrivacyInfo.xcprivacy`, and drop "Other User Content" from the Not-collected list below. This is the closer match to Play's answer.
> - **(b) Keep them inside User ID**, as §1.A describes them, and leave the manifest as it is.
>
> Either is defensible; what matters is that the label, the manifest and the Play form tell the same story. This file has not changed the answer.

### Declare these as NOT collected (verify each is "Not Collected" in the form):

Contact Info (name/email/phone/address), Health, Location, Sensitive Info, Contacts, Browsing/Search History, Purchases, Payment Info (credit cards — you have none), Audio, Gameplay, Other User Content *(unless you pick I11 option (a))*, and any Advertising/Tracking identifiers. (Photos or Videos and Customer Support are declared — §1.C.)

### Export compliance (every build)

- **Value:** `ITSAppUsesNonExemptEncryption = false` in `app-ios/VelaWallet/VelaWallet/Info.plist:91-92`. With the key present, App Store Connect does not ask on every upload, and a TestFlight build is not held at "Missing Compliance".
- **What the app actually encrypts:**
  - HTTPS/TLS, through the OS (`URLSession`, WKWebView).
  - The OS's own passkey cryptography (AuthenticationServices on iOS, Credential Manager on Android).
  - In the Rust core, AES used **only inside two authentication protocols** (dependencies `rust/crates/vela-core/Cargo.toml:111-118`):
    - the CTAP2 PIN/UV auth protocol that protects a security key's PIN — AES-256-CBC (`rust/crates/vela-core/src/ctap/pin_uv.rs:60-61`);
    - the caBLE / hybrid passkey tunnel to a nearby phone — AES for the advert, AES-256-GCM for the Noise channel that carries the passkey ceremony (`src/cable/crypto.rs`, `src/cable/noise.rs`, `src/cable/session.rs`).
  - The Trusted Signer's own end-to-end AES-GCM session was removed on 2026-09-23 (commit `0801564b7`) and is no longer in the binary.
- **Why `false` is defensible:** Apple says to answer NO if the app "only uses forms of encryption that are exempt". The US carve-out in BIS Technical Note 1 to 5A002.a excludes cryptography limited to authentication, digital signature, data integrity, non-repudiation and the key management that supports them. Both AES uses protect an authentication exchange (the security key's user verification; the hybrid passkey assertion), and nothing encrypts user data for confidentiality beyond the OS's TLS. (research-requirements.md §A5)
- **Owner:** this is a legal attestation. `false` is the practical answer for TestFlight and Play testing. **Get counsel's view before production**, including whether a year-end self-classification report applies (15 CFR 740.17(e)(3)).
- The plist comment above the key (`Info.plist:84-90`) is being corrected under spec 088 FR-007 to name the CTAP/caBLE AES and this rationale. The value stays `false`.

### Account deletion (Guideline 5.1.1(v))

- **In the app, on iPhone and Android:** Settings → **Erase This Device** deletes everything Vela stored on the device — accounts, transaction history, contacts and groups, browsing history and site data, dApp permissions, custom tokens and networks, RPC and endpoint settings, preferences — then checks that it is gone (iOS `Features/Settings/DeviceStorage.swift:158-180`, `App/RootView.swift:3459-3480`; Android `feature/settings/core/DeviceStorage.kt:83,119`). Per wallet there is also Settings → *Remove from this device*.
- **What erase cannot reach:** the passkeys themselves. They belong to the person's passkey provider (iCloud Keychain, Google Password Manager, another password manager, or a security key), which is where they delete them.
- **What our services keep, and for how long:** the relay keeps operations for 1 hour to 14 days; the passkey index keeps the registration record 7 days after success and 30 days after failure. Both expire by themselves; a request to delete them sooner goes to **hello@mondaylabs.ltd** (privacy-evidence.md §2.1, §2.2).
- **What nobody can delete:** the registry record on Gnosis is **append-only and permanent** (`p256-registrar/src/protocol.rs:5-6`). It holds each key's public key, credential ID and authenticator model, the wallet address, the wallet name and each key's label — pseudonymous, unable to move funds, and deletable by nobody, including us. The app says so before the wallet is created (`onboarding.create.ack0`). Don't hide this — explain it.
- **Public pages:** `https://getvela.app/delete` (steps, what is deleted, what is kept and why) and `https://getvela.app/support` (contact). Both are built in spec 088; the owner deploys them. Use them in the review notes and in Play's deletion field.
- Footnotes for the owner, not for the form: an index task that never reaches a terminal state is kept with no expiry (privacy-evidence.md §2.1 "Gap"), and `/api/query` answers sit in Cloudflare's edge cache for 24 h (`edge.rs:736-756`). Neither changes an answer, but both bound what "expires by itself" means.

### Age rating (App Store; not a TestFlight prerequisite)

- **Unrestricted Web Access = Yes** — the in-app browser opens any page (gives 16+ on iOS 26+). The terms set **18+** (`/terms` §7), so use the age-rating override to 18+. Gambling = No. Social-media capability = No. (audit B5, research §A9)

---

## 2. Google Play — Data Safety form (Play Console → App content → Data safety)

**Q: Does your app collect or share any required user data types?** → **Yes**

### Security practices

- **All user data encrypted in transit?** → **Yes** (HTTPS/TLS everywhere; the hybrid passkey channel is end-to-end encrypted).
- **Do you provide a way for users to request that their data is deleted?** → **Yes.** In the app: Settings → Erase This Device (on-device data). Outside the app: **`https://getvela.app/delete`** and **hello@mondaylabs.ltd** for the index and relay copies, which also expire by themselves (index 7 d after success / 30 d after failure; relay 1 h–14 d). The on-chain registry record is **append-only and cannot be deleted by anyone** — the page says so rather than implying full erasure.
- **Account creation / delete-account link:** the app creates an account (a passkey wallet, no username or password). If the form asks how accounts are created, pick the option closest to "Other" and describe it as *passkey*. **Delete-account URL: `https://getvela.app/delete`.** (research §G5: an in-app path **and** a web link that works without sending the person back to the app.)
- **Independent security review?** → **No.** Do **not** claim an audit — none exists and none is scheduled.
- **Committed to Play Families policy?** → **No** (not directed at children).

### Data types — declare COLLECTED:


| Play data type                                                           | Collected | Shared                       | Ephemeral?                         | Req/Opt      | Purpose                               |
| -------------------------------------------------------------------------- | ----------- | ------------------------------ | ------------------------------------ | -------------- | --------------------------------------- |
| **Financial info → Other financial info** (wallet address + the transactions sent from it) | Yes       | **Yes**                      | **No** — relay stores the operation 1 h–14 d; the address is published on-chain | Required     | App functionality, Account management |
| **Personal info → User IDs** (wallet address as account id, passkey public key, credential ID) | Yes | **Yes** (published on-chain) | No                                 | Required     | Account management, App functionality |
| **Personal info → Other info** (user-chosen wallet name and per-key labels)                | Yes       | **Yes** (published on-chain) | No                                 | Required     | Account management, App functionality |
| **App info & performance → Diagnostics** (bug report device lines)    | Yes       | **No** — see note            | No — the issue is kept             | **Optional** | App functionality (bug fixing)        |
| **App activity → Other user-generated content** (the typed report)    | Yes       | **No** — see note            | No                                 | **Optional** | App functionality (bug fixing)        |
| **Photos and videos → Photos** (≤ 5 report screenshots)               | Yes       | **No** — see note            | No — kept in R2 until deleted      | **Optional** | App functionality (bug fixing)        |

- **Bug report:** both apps POST the report to `getvela.app/api/bug-report`, which files a **public** GitHub issue and keeps screenshots in Cloudflare R2. "Shared: No" because Play does not count a transfer the user initiates knowing where it goes: the sheet says the issue and its screenshots are public before Send. No photo permission is requested (the Photo Picker runs out of process). Evidence: privacy-evidence.md §3a.
- **"Shared" is Yes for the on-chain items.** The index publishes the wallet address, the wallet name, every key label, every public key and every credential ID to **Gnosis Chain, permanently** (`p256-registrar/src/protocol.rs:27,88`; `p256-index-cf/src/chain.rs:465-470`). (If `ALCHEMY_API_KEY` is set on the relay or index, Alchemy is an additional recipient — confirm from the secret store, privacy-evidence.md §7 #3.)
- **Keep in step with Apple (I11):** "Personal info → Other info" here is the same data as the I11 box in §1. Whichever story you pick there, this row stays.

### Data types — declare NOT collected:

Location, Contacts, Calendar, Videos, Audio (no `RECORD_AUDIO`), SMS/Call logs, Health, **Device or other IDs** (no advertising ID, no device ID — `AD_ID` is absent from the merged manifest), Web browsing history (the in-app browser's history stays on device — `browser_history.rs:11-16`), Installed apps, App activity other than the typed bug report. (Photos, Diagnostics and Other user-generated content are declared — see the table above.)

- **Location:** **Not collected**, but be ready to explain the permission. `ACCESS_FINE_LOCATION` is declared at `maxSdkVersion="30"` and `BLUETOOTH_SCAN` carries `neverForLocation` — both exist only because Bluetooth scanning on Android 11 and below requires the location permission, for the hybrid "sign in with another device" flow. No location value is ever read or transmitted. (`AndroidManifest.xml:59-71`)
- **IP address 【你来定】:** the relay reads **no** client IP at all; the index keeps a salted, 64-bit-truncated hash for a **60-second** rate-limit window and never stores the raw value (`p256-index-cf/src/edge.rs:758-766`, `submitter.rs:630-649`). Cloudflare, as processor, sees it regardless. Google lets you treat purely-ephemeral connection data as not collected. **Recommend: do not declare** as collected, keep the privacy-policy mention.

---

## 3. Google Play — other "App content" declarations (don't forget these)

- **Developer account type** → must be an **Organization** account: Google requires crypto software wallets to be published from one (answer/10788890). Confirm before creating the app (audit B10).
- **Financial features declaration** → category **"Cryptocurrency wallet"** (non-custodial). Do **NOT** tick exchange / buy / sell / trade — Vela has no exchange, no swap or trade feature of its own and no fiat on-ramp. Non-custodial wallets are outside the crypto exchanges and software wallets licensing policy (answer/16329703), so no country licence applies. Whether the live form asks custodial vs non-custodial is unconfirmed — answer non-custodial if it does.
- **Target audience & content** → **18 and over**; not designed for or appealing to children.
- **Content rating (IARC questionnaire)** → answer **"Unrestricted internet" = Yes**: the built-in browser opens any web page. No other objectionable content. (research §G9, audit B5)
- **Permissions** — the shipped manifest declares `INTERNET`, `CAMERA`, `VIBRATE`, `POST_NOTIFICATIONS`, `BLUETOOTH_SCAN` (`neverForLocation`), `BLUETOOTH_CONNECT`, and legacy `BLUETOOTH` / `BLUETOOTH_ADMIN` / `ACCESS_FINE_LOCATION` capped at `maxSdkVersion="30"` (`AndroidManifest.xml:26-71`). Libraries add `USE_BIOMETRIC`, `USE_FINGERPRINT`, `WAKE_LOCK`, `ACCESS_NETWORK_STATE`, `RECEIVE_BOOT_COMPLETED` and `FOREGROUND_SERVICE` (WorkManager, no foreground-service type). **`BLUETOOTH_ADVERTISE` is not requested** — the phone never advertises. None of these is in Play's restricted set (SMS, Call Log, `MANAGE_EXTERNAL_STORAGE`, Accessibility, `QUERY_ALL_PACKAGES`, full-screen intent, foreground-service types, background location), so no declaration form is triggered — but do not tell a reviewer the app has no Bluetooth.
- **Ads** → app contains **no ads** → declare "No ads."
- **Government / News / Health / COVID** → N/A.

---

## 3a. How a reviewer creates a wallet (both stores)

**There is no demo account, and there cannot be one.** A Vela wallet has no username, password or server-side login. Its keys are passkeys, and a passkey cannot be handed to someone else. The honest route — the one research §G7 and §A1 point to — is: the reviewer creates a wallet on the review device in about two minutes, and a short video shows the parts that need funds.

**DEMO VIDEO: <owner to add unlisted link>** — 2–3 minutes, one take: create a wallet → Receive → Send → open a dApp in Explore, connect, and sign. Put the same link in the Apple notes (§4) and in Play's App access (§5).

### What the review device needs

- **iPhone:** a device passcode, and **iCloud Keychain** turned on (iCloud settings → Passwords and Keychain), so iOS can save the passkey. Bluetooth is needed only for the "Phone or tablet" fallback.
- **Android:** a **screen lock**, and a passkey provider — **Google Password Manager** (signed in to a Google account) or another provider chosen in the system's passwords settings. Bluetooth is needed only for the "Phone or tablet" fallback; on Android 11 and below that fallback also needs Location services on (`onboarding.common.locationNeededBody`).

### The steps (grounded in the create flow)

The flow is the core's create machine (`rust/crates/vela-core/src/app/create_wallet.rs:1-11`), drawn by `app-ios/VelaWallet/VelaWallet/Features/Onboarding/CreatePanel.swift` and Android `feature/onboarding/flow/`. Labels are from `rust/crates/vela-core/i18n/locales/en/onboarding.json`.

1. Open Vela. On the welcome screen tap **Create Wallet** (`welcome.createWallet`).
2. **Name your wallet** — any name, e.g. "Review" (`create.nameTitle`). Tick the three boxes: the public key and name go into the on-chain contract (`create.ack0`); the private key stays in the device's password manager or security key (`create.ack1`); the Privacy Policy and Terms (`create.ack2`). Tap **Create Wallet**.
3. On **Add passkeys** (`create.keysTitle`), tap **This device** (`create.methodPlatformTitle`). The system passkey sheet appears: approve it with Face ID / the passcode (iPhone) or the screen lock (Android). A second system prompt follows at once — it confirms the new key (one create + one sign per key, `create_wallet.rs:1171-1176`). Approve it too.
4. Tap **Create Wallet** again (`create.createWalletBtn`). The app derives the address and writes the key index. When **Wallet created** appears (`create.successTitle`), tap **Enter Wallet** (`create.enterWalletBtn`).

If the key is a passkey the provider syncs (iCloud Keychain, Google Password Manager), one key is enough. If the provider kept it on this device only, the button reads **Add a second key first** (`create.addSecondKeyBtn`; rule `needs_second_key`, `create_wallet.rs:878-884`) — add one more key, then continue.

**Fallback when the device has no usable passkey provider:** in step 3 tap **Phone or tablet** (`create.methodHybridTitle`). The review device shows a QR code; scan it with another phone that has a passkey provider and create the passkey there. On Android, **USB security key** (`create.methodSecurityKeyTitle`) also works with a FIDO2 key over USB.

### Then

- **Receive** (bottom of the wallet screen) shows the address and its QR code. **Save image** writes a share card to Photos.
- **Explore** (tab bar) is the built-in browser. Type a site address — there is no curated dApp list (`feature/browser/ExploreLive.kt:189-190`). Use the site's own Connect button; Vela shows a connection sheet to approve, and every signature request after that is decoded and needs a passkey check.
- **Send** needs funds on a network. The video shows a send; on request we will send a small amount to the review wallet's address.

### What every test wallet leaves behind

Creating a wallet writes a **permanent, pseudonymous public record** to a registry contract on **Gnosis Chain**: each key's public key, credential ID and authenticator model, the wallet name and key labels, and the wallet address (claim ledger C-reg-1). It cannot move funds, and nobody — including us — can delete it. Each review wallet adds one such record. The app says so before the person creates the wallet (`create.ack0`).

---

## 3b. Features a reviewer will meet (Apple 2.3.1(a); the same list helps Play)

| Feature | What it does | Where in code |
|---|---|---|
| **In-app dApp browser** with an injected **EIP-1193** provider (EIP-6963 announced) | Explore opens third-party web apps; a site connects only after the person approves the connection sheet, and every signature needs a passkey check. The provider is injected only into secure (`https`) origins; an `http` page can be read but cannot ask for a signature (`dapp_permissions` refuses `insecure_origin`, `Info.plist:10-14`). | Android `feature/browser/core/ProviderBridge.kt`, `BrowserController.kt`; iOS WKWebView browser (`Features/Explore/`). Secure-origin injection: spec 088 FR-004. |
| **`velawallet://` links** | `velawallet://pay` prefills a Send form (nothing is sent without the person's confirmation and passkey check). `velawallet://open?url=…` from outside the app **shows the site's host and asks before opening**, and opens `https` pages only. | Android `feature/wallet/core/PayLink.kt`, `MainActivity.kt`; iOS `Info.plist:65-83`. The confirmation is being implemented in spec 088 FR-004. |
| **QR scanning (camera)** | Scan a payment request or an address (Send, Explore, the Scan button). | `NSCameraUsageDescription` `Info.plist:93-94`; Android `CAMERA` `AndroidManifest.xml:28` (camera not required hardware). |
| **Save to Photos (share card)** | Receive → Save image writes the receive QR card to the album. iOS asks for **add-only** access; nothing is read back. | iOS `Core/ShareCardExport.swift:61-65`, `NSPhotoLibraryAddUsageDescription` `Info.plist:95-96`; Android `core/platform/Gallery.kt` (MediaStore insert, no permission). |
| **Feedback upload** with optional screenshots | Settings → Send feedback. Sends the typed text, a few device lines and up to five screenshots picked in the system photo picker (no photo permission), re-encoded on the device. It becomes a **public GitHub issue**; the sheet says so before Send. | Android `core/diagnostics/BugReport.kt:48, 116-143`; iOS `Core/ScreenshotPrep.swift`; privacy-evidence.md §3a. |
| **Bluetooth** — one use | "Phone or tablet" (sign in with another device, caBLE / hybrid): this device shows a QR code, the phone that scans it advertises over Bluetooth, and Vela **scans** for that advert and **connects** to finish the standard passkey exchange — over a Bluetooth L2CAP channel, or through the phone platform's standard end-to-end-encrypted tunnel server. Not used for a passkey on this device. The app never advertises. | iOS `Features/Onboarding/Core/HybridCableScanner.swift:45` (`CBCentralManager` only), `HybridCeremony.swift:142-150`; Android `AndroidManifest.xml:45-71`, `MainActivity.kt:93-102`. |
| **Background refresh** for a pending transaction | After a send, checks whether it confirmed and shows a **local** notification. Not an analytics or advertising tracker; no push service. | iOS `UIBackgroundModes = fetch`, `BGTaskSchedulerPermittedIdentifiers = app.getvela.VelaWallet.tracker` (`Info.plist:113-120`), `Features/Send/TrackerStore.swift:242`; Android WorkManager `feature/wallet/core/TrackerWork.kt:35-60`, `POST_NOTIFICATIONS` `AndroidManifest.xml:43`. |
| **Trusted Signer** (optional key method) | Opens Vela's signing page `sign.getvela.app` in the system browser view (SFSafariViewController / Custom Tab), which decodes the request itself before the passkey signs; the answer returns through `velawallet://sign-result`. | iOS `Features/Signing/TrustedSigner/TrustedSigner.swift:14-20`; Android `SignResultActivity` (`AndroidManifest.xml:185-198`). |
| **Erase This Device** | Settings → deletes everything the app stored on the device (§1, Account deletion). | iOS `Features/Settings/SettingsScreen.swift:464`; Android `feature/settings/SettingsScreen.kt:780`. |

Not in store builds: Settings' debug mode (spec 091) exists only in developer builds (Android `debug`, iOS `Debug`, desktop `dev-fixtures`), so a store build has no hidden feature.

---

## 4. App Review notes — Apple (App Store Connect → App Review Information → Notes)

Paste this (English — review is English-primary). The field takes **4,000 characters**; this block is **3,973** (all ASCII), so the video link that replaces the 29-character placeholder can be up to 56 characters; a longer one means shortening the `FEATURES` lines. Use the same text in TestFlight → Test Information → *Review notes* for external testing, with **Sign-in required** switched **off**.

```
Vela Wallet is a non-custodial (self-custody) wallet for Ethereum and other EVM networks, published by MONDAY LABS LTD (organization account). Each wallet is an unmodified Safe smart account, signed for by the user's own passkeys.

NO DEMO ACCOUNT: there is no username, password or server login, and a passkey cannot be shared. Please create a wallet on the review device (about 2 minutes). The device needs a passcode and iCloud Keychain turned on.
1. Open Vela, tap "Create Wallet".
2. Type any name, tick the three boxes, tap "Create Wallet".
3. On "Add passkeys", tap "This device". Approve the passkey sheet with Face ID or the passcode, then approve the second prompt, which confirms the key.
4. Tap "Create Wallet", then "Enter Wallet".
No iCloud Keychain? In step 3 choose "Phone or tablet" and scan the code with another phone that holds passkeys.

Then: Receive shows the address and QR code. Explore is the built-in browser: type a site address and use the site's Connect button. Send needs funds on a network; the video shows it, and on request we will send a small amount to the review wallet.
DEMO VIDEO (create, receive, send, dApp connect and sign): <owner to add unlisted link>

Creating a wallet writes a permanent, pseudonymous record to a public registry contract on Gnosis Chain: each key's public key and credential ID, the wallet name and key labels, and the address. It cannot move funds and nobody, including us, can delete it. The app says so before the wallet is created. Each test wallet adds one record.

FEATURES YOU WILL MEET
- Built-in dApp browser (WKWebView) with an injected EIP-1193 wallet provider, offered to https pages only. A velawallet://open link from outside the app shows the site's host and asks before opening, and opens https pages only. Every connection and signature needs the user's approval and a passkey check.
- Camera: scanning QR codes (payment requests, addresses).
- Photos (add-only): Receive > Save image saves the QR share card.
- Bluetooth: used only for "Phone or tablet" (sign in with another device). This iPhone shows a QR code; the phone that scans it advertises over Bluetooth, and Vela scans for it and connects to finish the standard passkey (caBLE/hybrid) exchange. A passkey on this iPhone needs no Bluetooth.
- Background app refresh (app.getvela.VelaWallet.tracker): checks whether a submitted transaction confirmed and posts a local notification. It is not an analytics or advertising tracker; the app has neither, and no push service.
- Feedback (Settings > Send feedback), optional: the typed text, a few device details and up to five screenshots chosen in the system photo picker, re-encoded on the device so no location or photo metadata leaves it. It becomes a public GitHub issue, which the sheet says before sending. No address, key or account ID is included.

GUIDELINE 3.1.5(i): Vela is a wallet offered by an organization. It runs no exchange, has no swap or trade feature of its own, no fiat on-ramp, no in-app purchase and no mining. Sites opened in the browser are third-party websites the user navigates to.

ENCRYPTION: ITSAppUsesNonExemptEncryption = NO. HTTPS/TLS, the OS's passkey cryptography, and AES used only inside authentication protocols (a security key's PIN protocol and the caBLE passkey tunnel).

DELETION: Settings > Erase This Device deletes everything the app stored on the device. Passkeys stay with the user's passkey provider. Our services keep copies for a limited time (relay up to 14 days, key index 7 to 30 days); requests to delete them sooner: hello@mondaylabs.ltd or https://getvela.app/delete. The on-chain registry record cannot be deleted by anyone.

PRIVACY: https://getvela.app/privacy. No analytics SDK, crash reporter or advertising identifier; the app never asks for a name, email or phone number. (The getvela.app website uses cookieless analytics; the app does not.)

Support: https://getvela.app/support
Contact for review: hello@mondaylabs.ltd
```

---

## 5. Google Play — App access instructions (App content → App access)

App access is where Play's reviewer reads how to get in. Choose **"All or some functionality is restricted"** and add **one** instruction set named *Create a wallet (no account)*. Leave username and password empty if the form allows it; otherwise write *not applicable — passkey wallet*. Paste the block below into the instructions field (research §G7: instructions in English). If the field is shorter than the block, keep the four steps and the video link and drop the rest.

```
No login and no demo account: Vela is a self-custody wallet whose keys are passkeys, and a passkey cannot be shared. Please create a wallet on the review device (about 2 minutes). The device needs a screen lock and a passkey provider (Google Password Manager signed in to a Google account, or another provider).
1. Open Vela, tap "Create Wallet".
2. Type any name, tick the three boxes, tap "Create Wallet".
3. On "Add passkeys", tap "This device". Approve the passkey prompt with the screen lock, then approve the second prompt, which confirms the key.
4. Tap "Create Wallet", then "Enter Wallet".
No passkey provider? In step 3 choose "Phone or tablet" (scan the code with another phone) or "USB security key".
Then: Receive shows the address and QR code; Explore is the built-in browser (type a site address, use the site's Connect button); Send needs funds - the video shows it, and on request we will send a small amount to the review wallet.
DEMO VIDEO: <owner to add unlisted link>
Each new wallet writes a permanent, pseudonymous record (public keys, wallet name, key labels, address) to a public registry contract on Gnosis Chain.
```

**Context to keep ready** (for a policy question, a permissions query or an appeal — Play has no general notes field to put it in up front):

```
Vela Wallet - a non-custodial (self-custody) EVM smart-account wallet by MONDAY LABS LTD (organization account).

Crypto: non-custodial software wallet. No exchange, no swap or trade feature of its own, no fiat on-ramp, no in-app purchases. Third-party dApps are used through the app's built-in browser, which injects an EIP-1193 provider into https pages only; a velawallet://open link from another app shows the site's host and asks before opening.

Bluetooth (BLUETOOTH_SCAN with neverForLocation, BLUETOOTH_CONNECT; on Android 11 and below the legacy BLUETOOTH/BLUETOOTH_ADMIN and ACCESS_FINE_LOCATION, all capped at maxSdkVersion=30) exists for one purpose: "Phone or tablet" sign-in, the standard caBLE/hybrid passkey flow. This device shows a QR code, then scans for and connects to the phone that scanned it. The app does not request BLUETOOTH_ADVERTISE and never advertises. No location is ever read or transmitted.

Background work: WorkManager checks whether a submitted transaction confirmed and posts a local notification (POST_NOTIFICATIONS, asked at the first send). No push service, no analytics, no advertising ID.

Bug reports (Settings > Send feedback) are optional and sent only when the user taps Send: the typed description, a few device details and up to five screenshots picked with the system Photo Picker (no storage or photo permission). Screenshots are re-encoded on the device (no location/EXIF). The report becomes a public GitHub issue, which the sheet states before sending; no wallet address, key or account id is included.

Deletion: Settings > Erase This Device (on-device data); https://getvela.app/delete or hello@mondaylabs.ltd for the off-chain index and relay copies, which also expire by themselves (index 7-30 days, relay up to 14 days). The on-chain registry record on Gnosis Chain is permanent and cannot be deleted by anyone.

Privacy policy: https://getvela.app/privacy
Support: https://getvela.app/support
Contact: hello@mondaylabs.ltd
```

> **Testing tracks and production access.** Google requires developers with **personal** accounts created after 13 November 2023 to run a closed test with **at least 12 testers opted in continuously for at least 14 days** before applying for production (answer/14151465; it was 20 testers until December 2024). The rule names personal accounts only, so **Organization** accounts fall outside its stated scope — no official page says "organizations are exempt" in so many words. A crypto wallet must be published from an Organization account anyway (§3). Two more timing facts: a Play app's **first** release is reviewed even on the internal track (a few hours to seven days), and if the app is **Paid**, closed and open testers must buy it while internal testers install it free (audit B8; research §G2, §G14).

---

## 6. Repo changes made so the forms stay true (2026-06-30) — HISTORICAL, PARTLY REVERSED

> **Read the 2026-09-23 note first.** This section describes the Expo-era binary. Three of its claims are no longer true of what ships:
> - **Bluetooth is back**, on both platforms, for the caBLE passkey flow (spec 019): `AndroidManifest.xml:59-71`, `app-ios/VelaWallet/VelaWallet/Info.plist:111-112`.
> - **`ACCESS_FINE_LOCATION` is back**, capped at `maxSdkVersion="30"`, as a pre-API-31 BLE-scan prerequisite.
> - **`ITSAppUsesNonExemptEncryption` was gone** — it lived in `app.json`, which spec 039 deleted — and is back in `Info.plist` as of 2026-09-23.
>
> Still true: no `RECORD_AUDIO`, no `SYSTEM_ALERT_WINDOW`, no microphone usage string, and the deletion channel.

Bluetooth was dropped entirely (dApp Connect then ran over the WalletPair HTTPS/WebSocket relay), along with the other unneeded permissions:

- Removed all `BLUETOOTH*` permissions + iOS Bluetooth usage strings + the `bluetooth-peripheral` background mode (`app.json`, `plugins/with-native-modules.js`, committed `Info.plist` + `AndroidManifest.xml`).
- Removed `RECORD_AUDIO` (disabled expo-camera mic via `recordAudioAndroid:false` + strip `NSMicrophoneUsageDescription`), `ACCESS_FINE_LOCATION` (only existed for BLE scanning), and `SYSTEM_ALERT_WINDOW`.
- Added `ITSAppUsesNonExemptEncryption=false`.
- Fixed the generic iOS photo-library usage string.
- Deleted legacy BLE code: `DAppScreen.tsx`, `(tabs)/dapps.tsx`, `src/modules/ble`, `walletpair-ble-transport.ts`, and native modules `modules/vela-ble` + `modules/walletpair-ble`; removed the BLE branch from `walletpair-transport.ts`.
- Deletion-request channel set to **hello@mondaylabs.ltd** across both forms (also add it to the privacy policy).

**Follow-up, restated for the native shells (2026-09-11, spec 039):** the Expo app and its config plugins are gone, so there is no prebuild step any more — the Xcode and Gradle projects under `app-ios/` and `app-android/` are hand-maintained. Before a store build, verify the binary directly: `Info.plist` has no `NSBluetooth*` / `UIBackgroundModes` / `NSMicrophoneUsageDescription` beyond what the shell actually uses, and `AndroidManifest.xml` has no `BLUETOOTH*` / `ACCESS_FINE_LOCATION` / `RECORD_AUDIO` / `SYSTEM_ALERT_WINDOW` beyond what the passkey transports need (the caBLE path on Android does use Bluetooth and location — see spec 019 and the OnePlus device notes before deciding what to declare). The `exp+vela-wallet` scheme, `_expo._tcp` Bonjour and the "Expo Dev Launcher" strings no longer exist in any build.
