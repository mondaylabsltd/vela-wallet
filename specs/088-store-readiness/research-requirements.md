# Store requirements for Vela Wallet's first test submissions (Part 1)

Researched 2026-10-01 from official Google and Apple pages. Every source below was **read 2026-10-01**.
"Q:" marks a quotation from the cited page. Quotes came through a fetch tool, so wording can differ slightly from the page; only the 16 KB quote was checked word for word against the raw page.
**UNCONFIRMED** means no official page states it; such points rest on inference or third-party reports, which are named.
Full research notes with more quotes: `notes-google-play.md`, `notes-apple.md`. Saved page copies: `raw/`.
How the native projects measure up against all this: `audit.md`. Section ids (§G…, §A…) are the ones `audit.md` cites.

**Plan being checked:** Android → Play internal testing, then closed testing. iOS → TestFlight internal, then external (Beta App Review).
**App:** non-custodial passkey-only Safe/ERC-4337 wallet, with an in-app dApp browser (injected EIP-1193), camera QR, an own relay, and a permanent on-chain public-key registry.
**Publisher:** Monday Labs Ltd. Its Apple account is an Organization with D-U-N-S. Its Play account type is not verified here.

---

## What changes the plan for tomorrow

| # | Fact | Store | Source |
|---|---|---|---|
| 1 | A Play app's **first** release is **reviewed even on the internal track**, which can take "a few hours or up to seven days". Only later internal uploads go live "within minutes". | Play | answer/9859654 |
| 2 | Since **2026-08-31**, new apps and updates on any track must target **API 36**. | Play | answer/11926878 |
| 3 | Native code must support **16 KB pages on 64-bit ABIs (arm64-v8a and x86_64)**. | Play | developer.android.com/guide/practices/page-sizes |
| 4 | Google re-signs new apps with a **Google-generated app-signing key**. Passkeys need `assetlinks.json` to list *that* key's SHA-256. | Play | answer/9842756; developer.android.com/training/app-links/configure-assetlinks |
| 5 | Crypto **software wallets must use an Organization** Play account. **Non-custodial wallets are out of scope** of the crypto country-licensing list. | Play | answer/10788890; answer/16329703 |
| 6 | **Free→paid is irreversible.** Closed and open testers must **buy** a paid app; internal testers install it free. | Play | answer/6334373; answer/9845334 |
| 7 | Every upload, TestFlight included, must be built with **Xcode 26 / iOS 26 SDK** (since 2026-04-28). | Apple | developer.apple.com/news/upcoming-requirements |
| 8 | A build with no export-compliance answer is "**Missing Compliance**" and can't be tested. | Apple | ASC Help, provide-export-compliance-information-for-beta-builds |
| 9 | Beta App Review applies the **full App Review Guidelines** (2.2). | Apple | App Review Guidelines |
| 10 | App Privacy label, screenshots and EU DSA trader status are **not** TestFlight prerequisites. They are App Store prerequisites. | Apple | app-privacy-details; DSA help page; testflight |

---

# Google Play

## G1. Target API level
- Q: "Starting August 31, 2026: New apps and app updates must target Android 16 (API level 36) or higher to be submitted to Google Play" (Wear OS and Automotive OS 35; TV and XR 34). — https://support.google.com/googleplay/android-developer/answer/11926878 and https://developer.android.com/google/play/requirements/target-sdk
- **Extension.** Q: "Developers will be able to request an extension to November 1, 2026". It is requested "through the details page of the warning or issue on the Policy status page in Play Console". A brand-new app has no warning to request it from (UNCONFIRMED whether a new app can get one).
- **Testing tracks.** The only exemption is "Permanently private apps … for internal distribution only". No testing-track exemption is stated, so treat the rule as applying to internal testing too.

## G2. Testing tracks, review, and the 12-tester rule
- **Internal testing.** Q: "An internal test can have up to 100 testers per app." Q: "When you publish a new Android App Bundle to the internal test track, it becomes available to testers within minutes." Q: "Internal tests might not be subject to standard Play policy or security reviews." Q: "You can start an internal test before completing app setup." — https://support.google.com/googleplay/android-developer/answer/9845334
- **First release is reviewed.** Q: "If your app's first release roll-out is on an Internal test track, the submission must be reviewed before it can be published." Q: "Reviews can take a few hours or up to seven days (or longer in exceptional cases)". — https://support.google.com/googleplay/android-developer/answer/9859654
- **First availability.** Q: "After publishing an open, closed, or internal test for the first time, the test link can take several hours to become available to testers." — answer/9845334
- **Closed testing.** Testers are managed by email lists or Google Groups: "up to 200 lists, and each list can contain up to 2,000 users"; "up to 50 lists per track". — answer/9845334
  - No official sentence says closed releases are reviewed. Changes to App content, store listing and targeting do go to review (answer/9859654). Plan for a review.
- **Open testing.** A tester limit, if set, must be at least 1,000. — answer/9845334
- **Pre-review checks.** Q: "You must fix critical issues before you can proceed with publication." They check that "updates contain all mandatory declarations on your app's App content page". — https://support.google.com/googleplay/android-developer/answer/14807773
- **12 testers × 14 days.** Q: "Developers with personal accounts created after November 13, 2023, must run a closed test for their app with a minimum of 12 testers who have been opted in continuously for at least 14 days." — https://support.google.com/googleplay/android-developer/answer/14151465
  - The number went from 20 to 12 on 2024-12-11 (https://android-developers.googleblog.com/2023/11/ensuring-high-quality-apps-on-google-play.html). The "20 testers" line in `docs/store-submission/privacy-and-review.md:242` is out of date.
  - **Organization accounts** are outside the rule's stated scope (personal accounts only). No official page says outright that "organizations are exempt".

## G3. Crypto policy and the Financial features declaration
- **Non-custodial carve-out.** Q: "Note: Non-custodial wallets are out of scope of the Cryptocurrency Exchanges and Software Wallets policy." — https://support.google.com/googleplay/android-developer/answer/16329703
- **Country requirements for custodial wallets and exchanges** (same page):
  - US: FinCEN MSB plus state money transmitter, or a bank.
  - EU/EEA: MiCA CASP authorisation (Liechtenstein, Iceland and Norway from July 2026).
  - UK: FCA registration.
  - Japan: FSA registration.
  - South Korea: KoFIU VASP report.
  - Canada: FINTRAC.
  - Also listed: Bahrain, Indonesia, Israel, Philippines, South Africa, UAE.
  - Hong Kong and Thailand: software wallets "Not required".
- **How to declare.** Q: "Under App Content, declare that your app is a cryptocurrency exchange and/or software wallet in the Financial Features Declaration. If your app is targeting any of the countries/regions listed below, you will be served location-specific forms". — answer/16329703
  - Whether the live form asks custodial vs non-custodial is UNCONFIRMED; the Console wasn't visible.
- **Declaration is mandatory.** Q: "All developers that have an app published on Google Play must complete the Financial features declaration, including apps on closed testing, open testing, or production tracks." The category to pick is "Cryptocurrency wallet". — https://support.google.com/googleplay/android-developer/answer/13849271
- **Blockchain-based content policy.**
  - Q: "We don't allow apps that mine cryptocurrency on devices." Vela doesn't mine.
  - Selling or awarding tokenized digital assets must be declared; Vela does neither.
  - Google "may request you to provide additional information or documents regarding your compliance". — https://support.google.com/googleplay/android-developer/answer/13607354
  - The same text is in the Developer Program Policy effective 2026-09-30 (answer/16933379).
- **dApp browsers.** Whether access to third-party DEXs through a dApp browser makes an app an "exchange" is not addressed by any official page (UNCONFIRMED).

## G4. Data safety form
- Q: "All developers that have an app published on Google Play must complete the Data safety form, including apps on closed, open, or production testing tracks."
- Q: "Apps that are active on internal testing tracks are exempt from inclusion in the data safety section."
- Q: "Even developers with apps that do not collect any user data must complete this form and provide a link to their privacy policy."
- Source for the three quotes above: https://support.google.com/googleplay/android-developer/answer/10787469
- **What the form asks:**
  - collection ("Transmitting data from your app off a user's device") and sharing ("Transferring … to a third party"), SDKs included;
  - encryption in transit;
  - "Does your app provide a way for users to request deletion of their data?";
  - exemptions for on-device-only, end-to-end-encrypted and ephemeral processing, and for user-initiated transfers the user expects. — answer/10787469
- **Privacy policy.** Q: "All apps must post a privacy policy link in the designated field within Play Console, and a privacy policy link or text within the app itself." — https://support.google.com/googleplay/android-developer/answer/10144311

## G5. Account deletion
- Q: "If your app allows users to create an account from within your app, then it must also allow users to request for their account to be deleted." There must be a "readily discoverable option to initiate app account deletion from within your app and outside of your app (for example, by visiting your website)". A link to the web resource goes in Play Console. Q: "Temporary account deactivation, disabling, or 'freezing' the app account does not qualify as account deletion." — https://support.google.com/googleplay/android-developer/answer/10144311
- **Definition.** Q: "An app account is a unique user identity that developers provide as a user-facing feature to serve the user across applications and/or devices". It authenticates by, among other things, "biometric". Q: "accounts that are created and operated offline are not app accounts". — https://support.google.com/googleplay/android-developer/answer/13327111
- **What's required.** Q: "provide users with an in-app path to delete their app accounts and associated data; and provide a web link resource where users can request app account deletion and associated data deletion". The web page must reference the app or developer name as listed, and must work without sending the user back to the app. — answer/13327111
- **Retention.** Q: "may need to retain certain data for legitimate reasons such as security, fraud prevention or regulatory compliance … you must clearly inform users about your data retention practices". No text addresses blockchains. — answer/13327111
- **Applied to Vela (interpretation):**
  - Creating a wallet registers public keys, an address and a name with Vela's index, then publishes them on Gnosis.
  - The passkey identity serves the user across devices.
  - So the wallet is plausibly an "app account".
  - Provide: an in-app path (Erase This Device, plus a request route for off-chain records); a public web URL; and a privacy-policy statement that on-chain records are permanent.

## G6. App bundle, Play App Signing, 64-bit, 16 KB pages
- **AAB.** New apps must publish as an Android App Bundle (since Aug 2021). — https://developer.android.com/guide/app-bundle
- **Play App Signing.** Source for all items: https://support.google.com/googleplay/android-developer/answer/9842756
  - New apps are Q: "automatically enrolled in quantum-ready, hybrid signing with Google-generated keys" (RSA-4096 + ML-DSA-65).
  - The upload key "must be an RSA key of 2048 bits or more".
  - An existing key can be supplied as the app-signing key with PEPK.
  - The upload key can be reset.
- **Passkeys.**
  - Q: "Using Digital Asset Links is required for passkeys". — https://developer.android.com/identity/credential-manager/prerequisites
  - Q: "If you're using Play App Signing for your app, then the certificate fingerprint produced by running `keytool` locally will usually not match the one on users' devices … you'll also find the correct Digital Asset Links JSON snippet for your app on the same page [App signing]." — https://developer.android.com/training/app-links/configure-assetlinks
  - Internal app sharing builds "are automatically re-signed with an Internal App Sharing key". — https://support.google.com/googleplay/android-developer/answer/9844679
- **64-bit.** Q: "for each native 32-bit architecture you support you must include the corresponding 64-bit architecture". — https://developer.android.com/google/play/requirements/64-bit
- **16 KB pages:**
  - Q (page last updated 2026-09-16): "all apps targeting Android 15 (API level 35) and higher must support 16 KB memory page sizes on 64-bit devices on Google Play. Starting February 1, 2027, if your app updates don't support 16 KB memory page sizes, you won't be able to release these updates." Q: "If any `arm64-v8a` or `x86_64` shared libraries are `UNALIGNED`, you'll need to update the packaging". — https://developer.android.com/guide/practices/page-sizes
  - Earlier announcement. Q: "Starting November 1st, 2025, all new apps and updates to existing apps submitted to Google Play and targeting Android 15+ devices must support 16 KB page sizes." — https://android-developers.googleblog.com/2025/05/prepare-play-apps-for-devices-with-16kb-page-size.html
  - "Technical quality requirements" lists it as current: Q: "Apps that contain native code must support devices with 16 KB memory page sizes." — https://support.google.com/googleplay/android-developer/answer/17492799
  - **UNCONFIRMED:** whether a *new* app's non-compliant bundle gets a hard error or a warning today, and whether internal tracks are checked differently. Community reports, e.g. https://github.com/flutter/flutter/issues/175599, show the Console naming x86_64 libraries and blocking.
  - Tooling: AGP ≥ 8.5.1 for zip alignment. NDK r28+ aligns to 16 KB by default; on older NDKs pass `-Wl,-z,max-page-size=16384`. — page-sizes doc
- **JNA.** JNA fixed jnidispatch on 16 KB pages in 5.16.0 (#1618) and 5.17.0 (#1647). — https://github.com/java-native-access/jna/blob/master/CHANGES.md (project changelog, not Google)

## G7. App access (instructions for reviewers)
- Q: "If your entire app or parts of your app are restricted based on login credentials … or other forms of authentication, you must provide all required details to enable access to your app." Path: App content → Sign-in details, up to five sets. — https://support.google.com/googleplay/android-developer/answer/9859455
- Q: "Your sign-in details must be accessible at all times, reusable, and valid regardless of user location." Two-step and one-time-password flows must be bypassable. Instructions must be in English. — https://support.google.com/googleplay/android-developer/answer/15748846
- Without a working demo path, "your app may be rejected". — https://support.google.com/googleplay/android-developer/answer/15191715
- **Passkey-only apps.** No official page covers passkeys, biometrics or demo videos (UNCONFIRMED).
  - A passkey can't be shared, so the honest route is: no account to give, the reviewer creates a wallet on the review device (screen lock + Google Password Manager or another provider), plus a video.

## G8. Store listing assets and text
- **Icon:** 512×512 "32-bit PNG (with alpha)", ≤ 1024 KB. — https://support.google.com/googleplay/android-developer/answer/9866151
- **Feature graphic:** "You must provide a feature graphic to publish your store listing". 1024×500, JPEG or 24-bit PNG with no alpha. — answer/9866151
- **Screenshots:**
  - "minimum of two screenshots across different device types", up to 8 per type, JPEG or 24-bit PNG with no alpha.
  - 320–3840 px, long side at most 2× the short side.
  - For recommendations: ≥ 4 screenshots at ≥ 1080 px.
  - Tablet and Chromebook screenshots are optional (needed only for large-screen recommendations).
  - Source: answer/9866151
- **Text:** title 30 characters, short description 80, full description 4000. A contact email is required; a website is recommended. — https://support.google.com/googleplay/android-developer/answer/9859152

## G9. App content declarations
- **Content rating.** Q: "All apps must have a content rating from the IARC to be on Google Play." — https://support.google.com/googleplay/android-developer/answer/9898843 (steps: answer/188189)
  - IARC's "Unrestricted Internet" element ("browser") applies to the dApp browser. — https://www.esrb.org/ratings-guide/ (IARC member, not Google)
- **Target audience.** Required for new apps; "18 and over" is available. — https://support.google.com/googleplay/android-developer/answer/9867159
- **Ads.** Q: "You must declare whether or not your app contains ads." — answer/9859455
- **Health apps declaration.** Required "including apps on closed testing, open testing, or production tracks". — https://support.google.com/googleplay/android-developer/answer/14738291
- **Also on the App content page:** Financial features (G3), Data safety (G4), Sign-in details (G7), and Government / News / COVID-19 where relevant. — answer/9859455
- **Which declarations gate which track:**
  - Internal can start "before completing app setup". But the first internal release is reviewed, and pre-review checks require "all mandatory declarations". Completing every App content item before the first upload is the safe course.
  - Closed testing needs Data safety, Financial features, Health, content rating, target audience, ads, privacy policy and the store listing.

## G10. Permission pitfalls
- **QUERY_ALL_PACKAGES** is restricted and needs a declaration; use `<queries>` instead. — https://support.google.com/googleplay/android-developer/answer/10158779
- **REQUEST_INSTALL_PACKAGES** is restricted. — https://support.google.com/googleplay/android-developer/answer/12085295
- **READ_MEDIA_IMAGES / READ_MEDIA_VIDEO** are allowed only when the system pickers aren't sufficient, and need a declaration. Use the Photo Picker for "scan QR from image". — https://support.google.com/googleplay/android-developer/answer/14115180
- **Foreground-service types** (targeting 14+) need a declaration, including a video. This applies only if FGS types are declared. — https://support.google.com/googleplay/android-developer/answer/13392821
- **Exact alarms, full-screen intent, Accessibility, VpnService** are restricted. — https://support.google.com/googleplay/android-developer/answer/9888170
- **Camera, USE_BIOMETRIC, POST_NOTIFICATIONS** have no dedicated Play policy. Request them in context. — answer/9888170; https://developer.android.com/develop/ui/views/notifications/notification-permission

## G11. WebView and in-app browser
- Device and Network Abuse example violation. Q: "Apps or third party code … containing a webview with added JavaScript Interface that loads untrusted web content (for example, http:// URL) or unverified URLs obtained from untrusted sources (for example, URLs obtained with untrusted Intents)." — https://support.google.com/googleplay/android-developer/answer/9888379
- **Executable code.** JavaScript in a WebView is outside the "no downloaded executable code" rule. — answer/9888379
- **Webview spam.** Q: "We don't allow apps whose primary purpose is to … provide a webview of a website without permission". A wallet's primary purpose is not that. — https://support.google.com/googleplay/android-developer/answer/9899034
- **Broken functionality.** Q: "We don't allow apps that crash, force close, freeze, or otherwise function abnormally." — https://support.google.com/googleplay/android-developer/answer/9898783
- **dApp browsers specifically:** no Play text found (UNCONFIRMED).

## G12. Android developer verification (sideloading)
- From **2026-09-30**, apps must be registered by verified developers to install on certified devices in Brazil, Indonesia, Singapore and Thailand. — https://android-developers.googleblog.com/2026/03/android-developer-verification-rolling-out-to-all-developers.html
- Play apps that use Play App Signing are "part of the automatic registration process". — https://developer.android.com/developer-verification/guides/faq
- Whether a brand-new app is registered at creation is UNCONFIRMED. Check Play Console's verification page after creating it.

## G13. Account type, D-U-N-S, public details
- Q: "developers providing the following services must register as an Organization: … Financial products and services, including … cryptocurrency software wallets, and cryptocurrency exchanges". There is no custodial/non-custodial distinction. — https://support.google.com/googleplay/android-developer/answer/10788890
- An Organization account needs a D-U-N-S number, organization documents and website verification. — https://support.google.com/googleplay/android-developer/answer/13634885 and https://support.google.com/googleplay/android-developer/answer/10841920
- Q: "Google will display your legal name, legal address, developer email address, and developer phone number on Google Play." — https://support.google.com/googleplay/android-developer/answer/13628312

## G14. Paid download (~US$39.99, no IAP)
- Q: "Once your app has been offered for free, the app can't be changed to paid." — https://support.google.com/googleplay/android-developer/answer/6334373
  - Whether an internal or closed release counts as "offered" is UNCONFIRMED. Pricing "affect[s] all versions across all tracks" (answer/9845334), so choose Paid at creation.
- Q: "To sell paid apps … you need to set up a profile in the Google payments center." Q: "You can link a Play Console and payments profile only once." — https://support.google.com/googleplay/android-developer/answer/3092739
  - The merchant must be in a supported country. — https://support.google.com/googleplay/android-developer/answer/9306917
- Q: "Testers must purchase paid apps when participating in open or closed tests." Q: "For internal tests, testers can install paid apps for free." — answer/9845334

---

# Apple

## A1. TestFlight: internal vs external
- **Internal testers.** Up to **100** App Store Connect users with the Account Holder, Admin, App Manager, Developer or Marketing role. Builds are testable for 90 days. — https://developer.apple.com/help/app-store-connect/test-a-beta-version/add-internal-testers and https://developer.apple.com/testflight/
  - Builds uploaded as "TestFlight Internal Only" can't go to external groups. — add-internal-testers
- **External testers.** Up to **10,000**. An internal group must exist first. — https://developer.apple.com/help/app-store-connect/test-a-beta-version/invite-external-testers
  - Public link, optionally with a tester limit or device and OS criteria. — invite-external-testers
- **Beta App Review.**
  - Q: "When you add the first build of your app to a group, the build gets sent to App Review … A review is required only for the first build. Subsequent builds may not require a full review." — https://developer.apple.com/help/app-store-connect/test-a-beta-version/testflight-overview
  - "up to six builds for TestFlight App Review within a 24-hour period"; one build per version in review at a time. — invite-external-testers
- **Which rules apply.** Guideline 2.2. Q: "Any app submitted for beta distribution via TestFlight should be intended for public distribution and should comply with the App Review Guidelines … apps using TestFlight cannot be distributed to testers in exchange for compensation of any kind". — https://developer.apple.com/app-store/review/guidelines/
- **Timing.** No TestFlight SLA. The general figure is "90% of submissions are reviewed in less than 24 hours". — https://developer.apple.com/distribute/app-review/
- **Test Information (external):**
  - Beta App Description: "This field is required".
  - Feedback Email, which is also the invite reply-to.
  - Beta App Review contact: name, email, phone.
  - Demo account if required.
  - Review notes of up to 4,000 characters, with no credentials in them.
  - Sources: https://developer.apple.com/help/app-store-connect/test-a-beta-version/provide-test-information and https://developer.apple.com/documentation/appstoreconnectapi/betaappreviewdetail/attributes-data.dictionary
  - The beta Privacy Policy URL is called "recommended" in the API docs (betaAppLocalization). Whether the ASC form requires it is UNCONFIRMED. Provide it anyway (5.1.1(i)).
- **Demo access.** Q: "If your app includes account-based features, provide either an active demo account or fully-featured demo mode". 2.1(a): a built-in demo mode in place of a demo account needs "prior approval by Apple". Q: "If features require an environment that is hard to replicate or require specific hardware, be prepared to provide a demo video or the hardware." — guidelines; https://developer.apple.com/distribute/app-review/
  - No passkey-specific guidance exists (UNCONFIRMED).
- **Export compliance blocks testing.** Q: "Specify encryption use for your build to avoid the beta being marked as Missing Compliance." A build is testable only after that. — https://developer.apple.com/help/app-store-connect/test-a-beta-version/provide-export-compliance-information-for-beta-builds and https://developer.apple.com/help/app-store-connect/reference/app-uploads/app-build-statuses

## A2. App Review Guidelines relevant to a crypto wallet (page "Last Updated: June 8, 2026")
Source for this section unless another URL is given: https://developer.apple.com/app-store/review/guidelines/

- **3.1.5 Cryptocurrencies (exact text):**
  - "(i) Wallets: Apps may facilitate virtual currency storage, provided they are offered by developers enrolled as an organization."
  - "(ii) Mining: Apps may not mine for cryptocurrencies unless the processing is performed off device (e.g. cloud-based mining)."
  - "(iii) Exchanges: Apps may facilitate transactions or transmissions of cryptocurrency on an approved exchange, provided they are offered only in countries or regions where the app has appropriate licensing and permissions to provide a cryptocurrency exchange."
  - "(iv) Initial Coin Offerings: … must come from established banks, securities firms, futures commission merchants (“FCM”), or other approved financial institutions".
  - "(v) Cryptocurrency apps may not offer currency for completing tasks, such as downloading other apps, encouraging other users to download, posting to social networks, etc."
  - Note: the review-notes draft cites "3.1.5(b)". The current numbering is 3.1.5(i)–(v).
- **3.2.1(viii).** Q: "Apps used for financial trading, investing, or money management should be submitted by the financial institution performing such services". 3.1.5(i) is the explicit wallet permission to point to. — clarified 2025-06-09, https://developer.apple.com/news/?id=r9dcmrvs
- **5.1.1(ix).** Crypto exchanges are among the "highly regulated fields" that must be submitted by the legal entity providing the service. — 2025-11-13, https://developer.apple.com/news/?id=ey6d8onl
- **3.1.1.** Q: "Apps may not use their own mechanisms to unlock content or functionality, such as license keys, augmented reality markers, QR codes, cryptocurrencies and cryptocurrency wallets". A *paid download* is not an unlock. The NFT paragraph lets apps show the user's own NFTs as long as ownership doesn't unlock features.
- **2.1.** Q: "include demo account info (and turn on your back-end service!)". Apps that crash or show obvious problems are rejected.
- **2.3.** Metadata must reflect the app accurately. 2.3.1(a): new features must be "described with specificity in the Notes for Review". 2.3.6: answer age-rating questions honestly. 2.3.10: no other mobile platforms' names or icons in metadata.
- **2.5.2.** Apps must be self-contained. Downloaded interpreted code must not change the app's primary purpose (ADPLA §3.3.1(B)). — https://developer.apple.com/support/terms/apple-developer-program-license-agreement/
- **2.5.6.** Q: "Apps that browse the web must use the appropriate WebKit framework and WebKit JavaScript." WKWebView satisfies it.
- **2.5.14.** Recording consent and indication, including camera use.
- **4.2.** Apps must offer more than a repackaged website.
- **4.7 (mini apps, plug-ins).** HTML5/JavaScript mini apps are in scope (since 2025-11-13). 4.7.2: no exposing native platform APIs to such software without permission. 4.7.3: no sharing data or permissions "without explicit user consent in each instance".
  - Whether an open-web dApp browser counts as "mini apps" is UNCONFIRMED.
  - The risk rises with a curated, store-like dApp catalogue (3.2.2(i)). Vela's Explore has no curated catalogue.
- **4.8.** Not triggered: no third-party login.
- **5.1.1(i)** privacy policy, in ASC and in the app. **5.1.1(ii)** consent, and purpose strings must be accurate. **5.1.1(iii)** prefer out-of-process pickers.
- **5.1.2(i).** Disclose sharing. Tracking needs ATT. Vela doesn't track.
- **5.2.2.** Displaying third-party content needs permission (relevant to logos).
- **1.5.** The Support URL needs "an easy way to contact you".

## A3. Account deletion (5.1.1(v))
- Current wording. Q: "If your app doesn’t include significant account-based features, let people use it without a login. If your app supports account creation, you must also offer account deletion within the app." — guidelines
- **Apple's FAQ** (https://developer.apple.com/support/offering-account-deletion-in-your-app/):
  - Auto-created accounts need deletion too.
  - Deactivating an account is "insufficient".
  - Q: "If local laws or regulations require that you maintain some data, let your users know."
  - Apps under 5.1.1(ix) may add customer-service steps.
- **Applied to Vela (interpretation):** offer in-app deletion of local data, a way to request deletion of server-side copies, and a clear statement that the on-chain registry record is permanent. Mirror it in the privacy policy, which 5.1.1(i) requires to describe retention and deletion.

## A4. App Privacy label and privacy manifest
- **When the label is required.** Q: "This information is required to submit new apps and app updates to the App Store." Neither page mentions TestFlight. — https://developer.apple.com/app-store/app-privacy-details/ and https://developer.apple.com/help/app-store-connect/manage-app-information/manage-app-privacy
  - A Privacy Policy URL "is required for all apps". — manage-app-privacy
- **Definitions** (app-privacy-details):
  - "Collect" = transmitting off device and keeping it "longer than what is necessary to service the transmitted request in real time".
  - "Linked" = personal data under privacy laws.
  - Tracking = linking with third-party data for ads, or sharing with data brokers.
  - Optional disclosure (feedback forms) qualifies only if the user's name or account name is "prominently displayed in the submission form".
  - Web views: data collected through web traffic must be declared "unless you are enabling the user to navigate the open web".
- **Data types.** Contact Info, Health & Fitness, Financial Info (incl. "Other Financial Info"), Location, Sensitive Info, Contacts, User Content (Photos or Videos, Customer Support, Other User Content…), Browsing/Search History, Identifiers (User ID, Device ID), Purchases, Usage Data, Diagnostics, Other. — app-privacy-details
- **Privacy manifest** (https://developer.apple.com/documentation/bundleresources/privacy-manifest-files):
  - Keys: `NSPrivacyTracking`, `NSPrivacyTrackingDomains`, `NSPrivacyCollectedDataTypes`, `NSPrivacyAccessedAPITypes`.
- **Required-reason APIs are enforced at upload.** Q: "Starting May 1, 2024, apps that don’t describe their use of required reason API in their privacy manifest file aren’t accepted by App Store Connect." This applies to TestFlight uploads too (inference: they are ASC uploads). — https://developer.apple.com/documentation/bundleresources/describing-use-of-required-reason-api
  - **UserDefaults:** `CA92.1` (the app's own data).
  - **FileTimestamp** (`stat`/`fstat`/`lstat`/`modificationDate`…): `C617.1` (files inside the container), `DDA9.1`, `3B52.1`.
  - **SystemBootTime** (`systemUptime`, `mach_absolute_time`): `35F9.1`, `8FFB.1`, `3D61.1`.
  - **DiskSpace** (`statfs`, volume capacity keys…): `E174.1`, `85F4.1`, `7D9E.1`.
  - **ActiveKeyboards:** `3EC4.1`, `54BD.1`.
  - Source for the reason codes: https://developer.apple.com/documentation/bundleresources/app-privacy-configuration/nsprivacyaccessedapitypes/nsprivacyaccessedapitype
- Listed third-party SDKs (including Lottie) must ship their own manifest. — https://developer.apple.com/support/third-party-SDK-requirements/

## A5. Export compliance
- Q: "Set the value to `NO` if your app—including any third-party libraries it links against—doesn’t use encryption, or if it only uses forms of encryption that are exempt from export compliance documentation requirements. Otherwise, set it to `YES`."
- Q: "Typically, the use of encryption that’s built into the operating system … is exempt …, whereas the use of proprietary encryption is not."
- Q: "If your app uses exempt forms of encryption, you might alternatively be required to submit a year-end self-classification report to the U.S. government."
- Source for the three quotes above: https://developer.apple.com/documentation/security/complying-with-encryption-export-regulations
- **The plist key.** If `ITSAppUsesNonExemptEncryption` is missing, ASC asks the questions on every upload. `YES` needs `ITSEncryptionExportComplianceCode` from Apple. — https://developer.apple.com/documentation/bundleresources/information-property-list/itsappusesnonexemptencryption
- **Documentation by case:** OS-only encryption needs none. A standard algorithm not provided by the OS needs a French declaration, only if distributed on the App Store in France. Proprietary encryption needs CCATS plus the French declaration. — https://developer.apple.com/help/app-store-connect/reference/app-information/export-compliance-documentation-for-encryption
  - Documents go in before TestFlight review and take about 2 business days. — https://developer.apple.com/help/app-store-connect/manage-app-information/determine-and-upload-app-encryption-documentation
- **US carve-out (BIS, Technical Note 1 to 5A002.a).** "Cryptography for data confidentiality" excludes "a. Authentication; b. Digital signature; c. Data integrity; d. Non-repudiation; … g. Key management in support of" a–f. Q: "The use of cryptography limited to a-g … results in a classification of the product NOT in 5A002.a." — https://www.bis.gov/learn-support/encryption-controls/cryptography-for-data-confidentiality
- **Annual self-classification report.** Since the 2021 rule, it applies to mass-market *components*, executable software of certain kinds, and non-mass-market items. — 15 CFR 740.17(e)(3), https://www.ecfr.gov/current/title-15/subtitle-B/chapter-VII/subchapter-C/part-740/section-740.17 and https://www.bis.gov/learn-support/encryption-controls/annual-self-classification
  - Whether a consumer app needs it: interpretation, confirm with counsel.

## A6. Minimum Xcode / SDK for uploads
- Q: "Apps uploaded to App Store Connect must be built with Xcode 26 or later using an SDK for iOS 26 …" (since 2026-04-28).
- Since 2026-09-09: "iOS and iPadOS apps uploaded to App Store Connect must target iOS 13 or later".
- Next: "Starting April 2027 … built with the iOS 27 & iPadOS 27 SDK or later".
- Sources: https://developer.apple.com/news/upcoming-requirements/, https://developer.apple.com/help/app-store-connect/manage-builds/upload-builds, https://developer.apple.com/news/?id=k1mtkt1k
- Builds made with the iOS 27 SDK must declare a launch screen (`UILaunchScreen` etc.), or upload fails with ITMS-90870. — https://developer.apple.com/documentation/technotes/tn3208-preparing-your-apps-launch-screen-to-meet-app-store-requirements

## A7. Info.plist purpose strings
- Q: "App Review checks for the use of protected resources, and rejects apps that contain code accessing those resources without a purpose string" (ITMS-90683). Strings can be localized with `InfoPlist.xcstrings`. — https://developer.apple.com/documentation/uikit/requesting-access-to-protected-resources
- **NSCameraUsageDescription:** required for camera APIs.
- **NSPhotoLibraryAddUsageDescription:** required for write-only Photos access.
- **PHPicker** needs no read permission.
- Sources for the three items above: the respective Info.plist key pages and https://developer.apple.com/documentation/photokit/delivering-an-enhanced-privacy-experience-in-your-photos-app
- **NSFaceIDUsageDescription:** "required if your app uses APIs that access Face ID" (LocalAuthentication). Whether passkeys through AuthenticationServices need it: no Apple statement found (UNCONFIRMED); inference is no.
- **NSBluetoothAlwaysUsageDescription:** "required if your app uses the device’s Bluetooth interface".
- **NSLocalNetworkUsageDescription:** "Any app that uses the local network, directly or indirectly, should include this description."

## A8. Associated domains (passkeys) and custom URL schemes
- **Passkeys need the entitlement.** Q: "You need to have an associated domain with the `webcredentials` service type when making a registration or assertion request; otherwise, the request returns an error." — https://developer.apple.com/documentation/authenticationservices/supporting-passkeys
- **AASA hosting.** The file lives at `https://<domain>/.well-known/apple-app-site-association`, served over https with no redirects. Apps are listed as `TEAMID.bundleid`. — https://developer.apple.com/documentation/xcode/supporting-associated-domains
  - Since iOS 14, devices fetch through Apple's CDN, which picks up changes "within 24 hours". — supporting-associated-domains
  - `?mode=developer` works only with development-signed builds, so TestFlight always uses the CDN. — https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.developer.associated-domains
- **Live check (2026-10-01):** `getvela.app` and `app-site-association.cdn-apple.com/a/v1/getvela.app` both return `webcredentials.apps = ["F9W689P9NE.app.getvela.VelaWallet", …]` and `applinks` for `/sign`.
- **Custom schemes.** Allowed. Q: "URL schemes offer a potential attack vector into your app, so make sure to validate all URL parameters and discard any malformed URLs". — https://developer.apple.com/documentation/xcode/defining-a-custom-url-scheme-for-your-app

## A9. Age rating
- **The 2025 system** adds 13+, 16+ and 18+. The new questions were due by 2026-01-31. — https://developer.apple.com/news/?id=ks775ehf
- **Social-media capability questions** are required for new apps and updates from Sep 2026. — https://developer.apple.com/news/?id=tlur8uvi
- **Unrestricted Web Access:** "Users can navigate to any webpage within the app or freely browse the web. May include: embedded browser functionality". It gives **16+** on iOS 26+ (17+ on earlier OS; Korea 15+; Brazil A16). — https://developer.apple.com/help/app-store-connect/reference/app-information/age-ratings-values-and-definitions
- **Override.** A higher rating can be set, and must be if a EULA sets a higher minimum age. An unrated app can't be published. — https://developer.apple.com/help/app-store-connect/manage-app-information/set-an-app-age-rating
- Whether TestFlight requires the age rating is UNCONFIRMED.

## A10. Screenshots, icon, launch screen
- **Screenshots** (https://developer.apple.com/help/app-store-connect/reference/app-information/screenshot-specifications):
  - iPhone 6.9": 1320×2868, 1290×2796 or 1260×2736. 6.5" is required only if 6.9" is not provided.
  - 13" iPad screenshots are required only if the app runs on iPad.
  - Format: no alpha; 1–10 screenshots.
  - TestFlight needs no screenshots. — https://developer.apple.com/testflight/
- **App icon.** 1024×1024 single image or Icon Composer, with dark and tinted appearances. — https://developer.apple.com/design/human-interface-guidelines/app-icons and https://developer.apple.com/documentation/xcode/configuring-your-app-icon
  - "No alpha in the App Store icon" (ITMS-90717) is attested only in developer forums (UNCONFIRMED as policy text).

## A11. iPad, Mac and Vision Pro
- `UIRequiresFullScreen` is deprecated in iPadOS 26. With the iOS 27 SDK it no longer opts out of resizing. This matters only if iPad is supported. — https://developer.apple.com/documentation/technotes/tn3192-migrating-your-app-from-the-deprecated-uirequiresfullscreen-key
- iPhone apps are offered on Apple-silicon Macs and on Vision Pro unless availability is edited. — https://developer.apple.com/help/app-store-connect/manage-your-apps-availability/manage-availability-of-iphone-and-ipad-apps-on-macs-with-apple-silicon and …-on-apple-vision-pro

## A12. EU Digital Services Act trader status
- A trader status declaration is required to distribute on the App Store, and "Apps without trader status will be removed from the App Store in the European Union".
- TestFlight-only distribution is **not** trader activity.
- For organizations, the D-U-N-S address is displayed automatically.
- Sources: https://developer.apple.com/help/app-store-connect/manage-compliance-information/manage-european-union-digital-services-act-trader-requirements and https://developer.apple.com/news/upcoming-requirements/

## A13. Agreements and recent news
- The updated Apple Developer Program License Agreement (EU Attachment 14) is effective **2026-10-01**. The Account Holder should accept it.
  - That an unaccepted agreement blocks uploads is commonly reported but UNCONFIRMED. — https://developer.apple.com/news/?id=gmws0jgp
- Guideline changes since 2025:
  - 3.2.1(viii) financial licensing (2025-06-09).
  - US-storefront link and NFT changes (2025-05-01).
  - 4.7 mini apps and 5.1.1(ix) crypto exchanges (2025-11-13).
  - 1.2 chat (2026-02-06).
  - Kid/teen safety (2026-06-08).
  - None restricts non-custodial wallets or dApp browsers specifically.
  - Sources: news ids r9dcmrvs, 9txfddzf, ey6d8onl, d75yllv4, a233fmpw at https://developer.apple.com/news/
- Press reports of Apple pressing wallets about dApp browsers are not backed by any official Apple statement (UNCONFIRMED).
