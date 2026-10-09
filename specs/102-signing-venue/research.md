I traced the Trusted Signer through the core, the page and every client. The code already treats it as a place where you check what you sign, not a place a passkey lives. Four results:

- **Keys:** the apps and the official page use the same passkey domain, so nothing technical stops any key from signing in either place.
- **Page hash:** never verified against what the browser actually runs.
- **The repeated QR / this device / USB choice:** that is the browser's own passkey chooser. The page gives the browser nothing to say where the key is.
- **A security hole:** for signatures, the official page will send its answer to any address the opener names.

Citations below are file:line in this repo at commit 3c4ed387a. Short prefixes:
- `C/` = `rust/crates/vela-core/src/`
- `P/` = `app-web/trusted-signer/`
- `D/` = `app-desktop/vela-wallet/src/`
- `A/` = `app-android/vela-wallet/app/src/main/java/app/getvela/wallet/`
- `I/` = `app-ios/VelaWallet/VelaWallet/`


## 0. Bottom line

1. **The code already agrees with the owner.**
   - `C/trusted_signer.rs:4-6`: "three others answer *where the passkey is*; this one answers *where the person checks what they sign*".
   - Web `app-web/vela-wallet/src/lib/onboarding/core/passkey.ts:116`: "The Trusted Signer is not a place a passkey is".
   - `D/executor/trusted_signer.rs:4`: "a page, not a key".
   - Spec 071 named it this way at the start (`specs/071-clear-signer/spec.md:10-22`).
   - The "fourth key method" shape comes from the owner's ruling of 2026-09-22 (`specs/075-clear-signer-channel/spec.md:10`, 「…和 这台设备 手机或平板 USB 安全密钥 是平级的…另一种 passkey 通道」). That ruling was made while the page could still run on another device over a tunnel or Bluetooth. Both were cut on 2026-09-23. Since then the page always runs on the same device, so it is a place to review, not a place a key lives.
2. **The passkey domain is not a reason for a separate key type with the official page.** Every native app and the page use relying party `getvela.app` (details in §2). Only a page on your own domain needs different keys.
3. **Integrity is not verified for what the browser runs.**
   - The phones never hash the page.
   - The desktop hashes it once at launch, in a separate fetch, writes the verdict to stderr, and refuses nothing (`ENFORCE=false`). Signatures don't even open the version that was checked.
   - The app shows no hash anywhere.
4. **The confusion has a mechanical cause**, laid out in §4.
5. **Recommendation:** make "where you review and sign" (in Vela / on your trusted page) a per-account setting, separate from the key. Show only three key places. Pass the account's key route to the page so the browser goes straight to that key. Turn the in-app sheet into a hand-off card. Enforce the integrity check and add it to the phones. Close the open-signing hole. Make self-hosting a choice made at wallet creation.

## 1. How it works today

**What is stored (core)**
- `KeyMethod::TrustedSigner` sits beside Platform, Hybrid and SecurityKey (`C/app/mod.rs:634-660`). The "sign with" values are listed in `C/wallet_keys.rs:444-450`.
- **Per key:** `AccountKey.signer_origin`, "its origin, and so its rpId" (`C/app/mod.rs:174-178`).
- **Per account, per device:** `signed_in_with: SignInKey{credential_id, method, transports, signer_origin}` (`C/app/mod.rs:206-250`), read by `sign_in_route` (`:258-289`).
- It is chosen at create or sign-in and "never per signature" (founder, 2026-09-26; `C/wallet_keys.rs:440-443`, `C/app/sign_pref.rs:16-21`).
- **Per device:** `vela.trustedSignerUrl` (`C/app/sign_pref.rs`).
- Any key that has a `signer_origin` is displayed as "Trusted Signer" (`C/wallet_keys.rs:143-157`).
- Once the first key commits the wallet to a domain, only methods that mint keys for that domain stay on offer (`C/app/create_wallet.rs:84-180`). The key registry stores one domain per wallet (`C/trusted_signer.rs:181-213`).

**Where the person chooses**

| | Create | Sign-in | Settings | Keys list |
|---|---|---|---|---|
| Desktop | `D/hardware.rs:160-165`, `D/onboarding_flow.rs:927-1047` | `D/hardware.rs:170-175, 240-324` | `D/wallet/page.rs:11301-11450` | `D/wallet/page.rs:20170-20212` |
| Android | `A/feature/onboarding/flow/KeysScreen.kt:90, 393-481` | `SignInMethodSheet.kt:76-80` | `A/feature/settings/SettingsScreen.kt:2362-2408` | `SettingsLive.kt:1008-1028` |
| iOS | `I/Features/Onboarding/CreatePanel.swift:415-499` | `WelcomeScreen.swift:86-133` | `SettingsLive.swift:676-716` | `SettingsLive.swift:486-533` |
| Web | removed (`AddMethodPicker.svelte:52-60`) | — | dead code | a record with a page origin throws when signing (`sign-challenge.ts:49-57`) |

**How a signature travels**
1. The core builds `{intent, context}`. The context holds the account, its name, `allowCredentials` (credential ids only, no transports), the site (only when Vela's own browser saw it), and the assembled user operation with its fee-leg index, EntryPoint and module (`C/trusted_signer.rs:317-377`).
2. `url_launch` produces `https://sign.getvela.app/b/<LAUNCH>/sign?ch=url#i=<deflated base64url>&cb=velawallet://sign-result&t=<16-byte token>&z=1` (`:555-602`). A custom page gets `<base>sign.html`.
3. The page opens in:
   - the default browser on desktop (`D/wallet/money.rs:1216-1221`);
   - a Custom Tab on Android (`A/.../TrustedSignerTab.kt:23-43`);
   - an `SFSafariViewController` on iOS (`I/Features/Signing/TrustedSigner/TrustedSigner.swift:360-367`).
4. The page reads and wipes the fragment (`P/src/lib/intake.js:269-321`), works out the digest itself, waits for the slide, then calls `navigator.credentials.get` (`P/src/lib/signer.js:101-131`).
5. It answers by navigating to `velawallet://sign-result?t=…&result=…`. Closing the page sends `user_rejected` by beacon (`P/src/sign.js:436-441`).
6. The callback arrives at:
   - desktop: `on_open_urls`, or argv plus a single-instance relay on Windows/Linux (`D/main.rs:338-389`);
   - Android: `SignResultActivity` (`AndroidManifest.xml:185-198`);
   - iOS: `onOpenURL` (`I/App/RootView.swift:737, 840-845`).
7. `parse_callback` checks the token. `verify` checks user verification, that the challenge equals the wallet's own digest, that the key is one of the wallet's, and the P-256 signature (`C/trusted_signer.rs:475-549`).
   - It does **not** check the origin or the domain hash. `P/README.md:156` claims it checks the domain hash; that is wrong.
   - Key ceremonies do check the origin (`C/trusted_signer/ceremony.rs:246-260`).
8. Every client times out after 5 minutes. "Reopen" reuses the same URL and token.

**What the page shows.** It decodes the operation's own calldata, including batched legs. It checks that the call the site asked for is inside the operation. It reads the fee from the fee leg, names the chain from its id, and draws the identicon locally. Values the requester supplies are labelled as such (`P/PROTOCOL.md` §8.1, §9). It has two languages only, en and zh.

**Which authenticators the page offers: none of its own.**
- `get()` passes credential ids with no transports and no hints (`signer.js:103-113`).
- `create()` passes no attachment and no hints (`P/src/lib/ceremony.js:255-272`).
- So the browser draws its generic chooser. By contrast, the web wallet already passes hints (`passkey.ts:118-131`), and the core itself warns that a request without transports makes Android guess "security key" (`C/app/shell.rs:127-136`).

**Hosting**
- Cloudflare Pages, deployed by hand by copying `dist/`. There is no CI (`P/samples/build-single.mjs:35-47`, `P/HANDOVER.md:13-22`).
- `dist/_headers` marks `/b/*` as cacheable for a year and immutable. There is no CSP header (the CSP is only a `<meta>` tag in the page) and no `frame-ancestors`.
- The root `sign.html` is byte-identical to the current version. I confirmed with shasum that all 12 `b/<hash>/sign.html` files hash to their own names.

## 2. Passkey domain (RP ID)

| Surface | Domain | What binds it |
|---|---|---|
| iOS | `getvela.app` (`I/Features/Onboarding/Core/PasskeyExecutor.swift:62-65`) | `webcredentials:getvela.app` (`VelaWallet.entitlements:23-27`) plus the apple-app-site-association file (`app-web/getvela.app/src/routes/.well-known/apple-app-site-association/+server.ts:5-10`) |
| macOS | `getvela.app` (`D/executor/passkey.rs:43-52`, `D/executor/platform_macos.rs:234-238`) | same App ID `F9W689P9NE.app.getvela.VelaWallet` |
| Windows / Linux / USB / phone-scan | `getvela.app` via `webauthn.dll` or the core's own CTAP client | nothing: any app may name any domain |
| Android | `getvela.app` (`A/feature/onboarding/core/PasskeyExecutor.kt:822`) | assetlinks.json (`.well-known/assetlinks.json/+server.ts:5-44`). The Play signing key is missing — a blocker per `specs/088-store-readiness/audit.md:28` |
| Web wallet (`wallet.getvela.app`) | `getvela.app` | parent domain |
| Official page | `sign.getvela.app`, folded to `getvela.app` (`signer.js:23-27`) | parent domain |
| Self-hosted page | its own host | — |

- **Can a key made in the app sign on the official page, and the reverse?** In code, yes, both ways. The on-chain verifier ignores the domain (`specs/071-clear-signer/research.md:59-62`). Whether the browser and the app actually see the same credential depends on the passkey provider. The repo only tests this in an iOS simulator round trip.
- **So the separate key type exists because of the 075 ruling, not because of the domain.** The domain constraint only bites for self-hosted pages, and the code handles that: `uses_wallet_passkeys`, `registry_rp_id`, `methods_for`, and the reachability check in `C/wallet_keys.rs:513-520`.
- **A self-hosted page makes keys for its own domain.**
  - The phone apps can't use those keys natively.
  - They can't join a `getvela.app` wallet, because the registry holds one domain per wallet.
  - So a self-hosted page creates a separate wallet on your own domain (the public doc says so: `clear-signing-self-host.md:111-118`).
  - There is no `/.well-known/webauthn` related-origins file anywhere, even though `self-hosting.md:51-52` mentions related origins.
  - Related origins wouldn't solve self-hosting anyway (general WebAuthn facts, not checked in the repo): Vela controls the list, it is capped at 5 domains, only Chrome 128+ and Safari 18+ support it, and every listed origin can request `getvela.app` signatures.
- **A consequence the owner should weigh.** Because both places share one domain, the app — or a compromised library inside it — can request a native passkey signature over any digest without ever opening the page. The OS sheet shows only "getvela.app", never the transaction; the site's own header file says exactly this (`app-web/getvela.app/_headers:1-4`).
  - So today the page protects against a preview that lies. It does not protect against a compromised app.
  - Making that cryptographic would need keys under a domain the apps are not bound to (for example `sign.getvela.app` without folding, and no association files there).
  - That would work on iOS, Android and macOS platform passkeys, but not on Windows or with USB keys and phone-scan from the desktop. The exact iOS subdomain rule needs checking on a device.

## 3. Integrity — the question to verify ("需要求证")

**What is actually checked, by whom**
- **Desktop only.** `prime_in_background` runs once at launch (`D/executor/trusted_signer.rs:590-599`). It fetches `index.json`, picks a version, fetches `/b/<h>/sign.html` with the wallet's own HTTP client, hashes it, decides, and calls `eprintln!`.
  - The chosen hash is remembered before the page is fetched (`D/executor/signer_integrity.rs:200`).
  - The trusted and blocked lists are never written. "Verification off" is hard-coded to false (`:257`).
  - **No UI string or screen shows the verdict.**
- **Desktop signatures open the core's fixed `LAUNCH` path, not the checked one** (`D/executor/send.rs:668` → `chosen()` → `C/trusted_signer.rs:559-574`). Only key ceremonies use the checked URL (`D/executor/trusted_signer.rs:956-977`).
  - Today the two paths match only because `LAUNCH` equals `BUILD_ALLOWED[0]` (`C/trusted_signer/integrity.rs:66` and `:148`). That is a coincidence, not by design.
  - Signatures with a custom page open the unhashed root.
- **Android and iOS hash nothing.** Task T034 is still open (`specs/076-signer-page-integrity/tasks.md:124`). The bindings exist but are never called. A custom URL gets only a format check and the "other domain" warning.
- **The browser** never checks the hash in the URL path — it is just a name. The page's CSP stops fetch, XHR, WebSocket and remote images, but not a top-level navigation, which is how the answer leaves.
- **Service Worker pinning (FR-008), the only measure aimed at a server that serves bad bytes to one victim, was dropped** (`specs/079-android-dapp-browser-stability/research.md:225-227`: the hash-only CSP blocks registering it). Even if it existed, a browser re-fetches the worker script from the server, so it would not stop a compromised host. That second point is general web-platform behaviour, not tested here.
- Spec 076's status line and task T022 are stale. Tasks T040–T041 and T050–T053 are open.

**The honest guarantee today.** Any signature the app accepts was user-verified, over the digest the app computed, by one of the wallet's keys. Whether the page shown was the published code rests entirely on TLS and on whoever controls the `sign.getvela.app` Cloudflare project and DNS. A replaced build would be caught only by someone running curl and shasum, or appear in a desktop stderr log.

**What a real guarantee would need, cheapest first**
1. **Catch a build swapped for everyone:**
   - Phones fetch and hash (T034), at a random time (T040).
   - One shared rule gives the URL that is opened, and it equals the checked version.
   - Turn `ENFORCE` on, and fail closed.
   - Show the status before hand-off.
   - Re-check afterwards (T041).
   - Run a public monitor that fetches every `/b/*` against `dist/`.
2. **Verify it yourself:** publish a short fingerprint in the app and the docs; the curl + `build-single --check` steps already exist. Caveat: the bytes you fetch may not be the bytes your tab got.
3. **Stop a server that targets one victim:** this needs a verifier on the device that sees the executed bytes and is not the server. Options are a browser extension (desktop only, and the extension was removed), Chrome's signed Isolated Web Apps (not on phones), or self-hosting. Loading verified bytes into an app-owned WebView was ruled out (076 spec:58-64), and a page reporting its own hash proves nothing (`:66-69`).

Copy should say "matches Vela's published, reproducible build list (checked at …)", never "verified, untampered".

## 4. The UX today and where the confusion comes from

**The paths**
- **Create:** the picker opens automatically with This device / Phone or tablet / USB security key / Trusted Signer ("Check and sign on a separate page — what you see is what you sign.").
  - Tapping Trusted Signer opens the page's create card with its slider.
  - Then the browser's "where to save your passkey" sheet appears: **this device / phone (QR) / security key**.
  - Then a second page visit for the member proof: another slide, another prompt.
- **Sign-in:** the same four rows, then the page's sign-in card, then the browser chooser.
- **Signing:**
  - The full in-app preview, with the button `componentsUi.signing.openSigner` ("Continue to signing page" / 「去签名页确认」).
  - The same preview again on the page, then the slide, then the browser prompt.
  - A waiting card: `trustedSignerWaiting`, plus a stale hint about "reach other apps on this device".
- **Errors:** `trustedSigner{Closed,Refused,Mismatch,Timeout}`, `signerDown`, `trustedSignerReopen`.
- **Settings:** `settings.signing.page*`.
- **Keys list:** keys made on the page are captioned "Trusted Signer".

**Causes of the confusion**
1. A non-place sits in a list of places. The page then asks the browser "where is your key?" with no hints, so the browser asks the person again.
2. "Trusted" points at nothing visible: no version or hash on screen, and no check at all on the phones.
3. There are two previews, and nothing says which one is authoritative. The owner already said 「可信签名器页面看起来也很复杂，小白都不敢用了」 (`specs/079-android-dapp-browser-stability/spec.md:37`).
4. Ordinary iCloud or Google passkeys made on the page are labelled "Trusted Signer".

**Stale strings and bugs found along the way**
- `settings.signing.subtitle` still promises a per-signature choice that no longer exists.
- The page's zh text says the member challenge is fetched from the registry; it is computed locally now.
- iOS has hard-coded English errors (`I/Core/UserOpSpine.swift:572, 584, 599, 603`).
- Android drops the sub-path of a self-hosted page (`TrustedSignerChannel.kt:285`).

## 5. Design options

The new axis is the venue, separate from where the key lives:

| How the venue is chosen | For | Against |
|---|---|---|
| **A. Per account, on this device (recommended)** | Matches the "vault vs spending wallet" mental model; changeable without touching keys | One more account setting |
| B. One setting for the whole device | Simplest | A multi-wallet person can't have both |
| C. Per transaction | Flexible | Contradicts the 2026-09-26 ruling and adds a decision every time |
| D. Per key (today) | — | The wrong shape: the same key works in both places |

**Recommended model**
- Key choosers show three places only.
- For the official page, create, sign-in and proofs stay in the app: the challenge is random or derived, so there is nothing to preview.
- The venue applies only to transactions and messages.
- Self-hosting becomes an advanced choice at creation ("my own signing page"). The wallet is bound to that domain and its venue is locked to the page. A Settings URL field you can edit at any time is a social-engineering door.
- Optionally (owner decides), a one-way "Review this one on the trusted page" link in the in-app sheet.

**The in-app sheet when the venue is the page.** A hand-off card instead of a second full preview:
- "Review and sign on your trusted page".
- Minimal context only.
- The key you will confirm with.
- An integrity line ("version 0ba8ee8c · matches the published list · checked …", or a refusal).
- "Open".

**The page**
- New context field `keyRoute{credentialId, transports, method}`, with `hints` set, so the browser goes straight to the right key. Only the signed-in key should be allowed; iOS and the desktop already narrow to it.
- Refuse signatures whose answer address is not `velawallet://`.
- Add `frame-ancestors 'none'` to `_headers`.
- 15 languages.

**What changes where**
- **Core:**
  - A venue type plus account storage.
  - Retire `KeyMethod::TrustedSigner` for new records, keeping a reader for old ones.
  - `request()` carries the key route.
  - `method_words` loses the fourth row and gains venue words.
  - `methods_for` stops treating the official page as a method.
  - `row_method` stops labelling `getvela.app` keys as "Trusted Signer".
  - One rule produces both the launch URL and the checked version.
  - Turn on `ENFORCE`.
- **Each client (×3):**
  - Remove the fourth row from the create and sign-in choosers.
  - Add the venue setting and the hand-off card.
  - Phones: add the integrity check.
  - Desktop: open the checked URL for signatures.
- **i18n:**
  - Retire `trustedSignerTitle`/`Body` as a key method.
  - New keys for the venue, the hand-off card and about 6 integrity verdicts.
  - Fix the stale strings.
  - 15 app locales, and the page goes from 2 to 15 (every page change is a new hash).

**Migration** (alpha rule: old builds must keep working)
- Accounts on the official page or an empty page origin: venue = page; the key route comes from the key's stored transports.
- Accounts on a custom page origin: self-host mode, venue locked to the page.
- Keep `signer_origin` written, so old builds keep routing to the page. New fields default safely when an old build reads them.
- Nothing changes in the registry or on-chain.

**Security risks**
1. **The open signing hole (found by reading the code, not yet exercised).**
   - For signatures, the page accepts any opener and any answer address (`P/src/lib/intake.js:269-321`).
   - The "answer must reach Vela" rule applies only to key ceremonies (`P/src/lib/resolve.js:975-987, 1068`).
   - So a phishing site can open the **real** `sign.getvela.app` with a crafted operation and `cb=https://attacker`. The person sees a truthful card on Vela's real domain; if they slide, the attacker receives a usable signature.
   - Framing makes this worse, since `frame-ancestors` is absent.
   - This should be fixed before the page is promoted.
2. **Lookalike pages** can't use `getvela.app` keys. The real risks are a custom page set in Settings and any widening of related origins.
3. **The shared domain:** any `*.getvela.app` page, and the native apps, can request signatures. The venue is an assurance about what you see, not key isolation (§2).
4. **Replay:** limited by the operation nonce and the one-time token. Callbacks with an unknown token are dropped.
5. **Another app registering `velawallet://`:** a documented loss of availability or confidentiality, not integrity (`C/trusted_signer.rs:67-78`).
6. **Tampering between app and page** ends in a refusal or a mismatch. The context fields the app supplies are labelled on the page.

## 6. Open questions for the owner

1. **How is the venue chosen?** Per account on this device (recommended), one setting per device, or per transaction? Is a one-way per-transaction escape allowed despite the 2026-09-26 ruling?
2. **Ceremonies:** should key creation, sign-in and proofs ever run on the official page? I recommend only for self-hosted pages.
3. **Hand-off card:** show a summary in the app, which could disagree with the page, or only "open the page"?
4. **Self-hosting:** a wallet bound to your own domain from creation, a curated related-origins list, or drop it?
5. **Integrity:** accept "catches a swapped build plus public transparency", enforce it now, and finish the phone check before using the word "trusted"? What exact wording?
6. **Key isolation:** do you want a cryptographic "only the page can sign" mode — keys on a separate domain, never signing in the app, with the desktop caveat?
7. **Name:** keep 可信签名器? Whatever name is used needs something visible on screen that backs up "trusted".
8. **Page languages:** all 15?
9. **The open hole:** refuse answer addresses other than `velawallet://` for signatures? This needs a test-only exception for the desktop loopback demo harness.
10. **Existing page-made `getvela.app` keys:** relabel them silently, or tell the person?