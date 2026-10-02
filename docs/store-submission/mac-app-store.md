# Mac App Store — Vela Wallet for Mac (TestFlight first)

Spec 095. The desktop app (`app-desktop/vela-wallet`, Rust + gpui + WKWebView)
joins the **existing iOS app record** as a **Universal Purchase**: same bundle id
`app.getvela.VelaWallet`, same team `F9W689P9NE`, one price, one privacy label,
one age rating. Everything here is grounded in the code at the commit that added
this file and in [the claim ledger](../../specs/080-site-content-accuracy/claim-ledger.md)
(claim ids in brackets). The iPhone/Android documents are
[privacy-and-review.md](privacy-and-review.md) and
[store-listing-copy.md](store-listing-copy.md); this file only adds what is
different on the Mac.

## 1. Owner checklist (in order; only the account holder can do these)

1. **Agreements.** developer.apple.com and App Store Connect → accept every
   pending agreement (the updated Program License Agreement took effect
   2026-10-01). Paid Apps agreement + tax + banking must be active before a
   price applies to the Mac too.
2. **Provisioning profile.** developer.apple.com → Certificates, IDs & Profiles
   → Profiles → **+** → Distribution: **Mac App Store Connect** → App ID
   `app.getvela.VelaWallet` → the **Apple Distribution: MONDAY LABS LTD**
   certificate → download. (The App ID already has Associated Domains for the
   iPhone; nothing to add.) Leave the file in `~/Downloads` or note its path.
3. **Installer certificate.** Already in this Mac's keychain: *3rd Party Mac
   Developer Installer: MONDAY LABS LTD (F9W689P9NE)*. Nothing to do unless it
   expires.
4. **App record.** App Store Connect → Apps → Vela Wallet → the **+** next to
   "iOS App" in the sidebar → **macOS**. This makes the Mac a platform of the
   same record (Universal Purchase). Fill the macOS version page from §6 and
   upload Mac screenshots (§9).
5. **Credentials for validation/upload**, once: App Store Connect → Users and
   Access → Integrations → **App Store Connect API** → generate a key with the
   *Developer* (or App Manager) role, download `AuthKey_<KEYID>.p8` into
   `~/.appstoreconnect/private_keys/`, note the Key ID and the Issuer ID.
6. **Build, sign, package, validate** (≈5 min on this Mac; nothing is uploaded):
   ```sh
   cd app-desktop/vela-wallet
   VELA_MAS_PROFILE=~/Downloads/<the profile>.provisionprofile \
   VELA_ASC_KEY_ID=<KEYID> VELA_ASC_ISSUER=<ISSUER-UUID> \
     ./scripts/build-macos-mas.sh
   ```
   It stops with a sentence if the profile is the wrong kind, the wrong app,
   lacks Associated Domains or has expired, or if the keychain lacks the
   certificate the profile names. Output:
   `dist/macos/mas/VelaWallet-<version>-<build>-mas.pkg`.
7. **Upload**: drag the `.pkg` into **Transporter**, or
   `xcrun altool --upload-app -f <pkg> -t macos --api-key <KEYID> --api-issuer <ISSUER>`.
8. **TestFlight.** When the build has processed: export compliance is answered
   by the bundle (§4), so the build is not held at *Missing Compliance*.
   Internal testers can install at once; external testers need the Test
   Information in §7 and Beta App Review.

## 2. What the build does (`scripts/build-macos-mas.sh`)

| Step | What | Why |
|---|---|---|
| Profile | The one input: `VELA_MAS_PROFILE` or `--profile`; else the first fitting profile in `~/Downloads` and the two profile folders. Checked: kind (App Store vs development vs Developer ID, from what it provisions), app id, Associated Domains, expiry. | A wrong profile is otherwise found out by App Store Connect, after the upload. |
| Identities | App signature = the keychain certificate **the profile names**, by SHA-1 (this Mac holds several same-named certificates). Installer = *3rd Party Mac Developer Installer* / *Mac Installer Distribution* of the team, or `VELA_MAS_INSTALLER_IDENTITY`. | — |
| Build | `build-macos-app.sh --arch universal --no-dmg` — the Developer ID pipeline's own release build (no `dev-fixtures`), arm64 + x86_64. | One way to build. |
| Store check | `scripts/check-store-binary.sh`: no `_CGS*`/`_SLS*` import, none of gpui's three private selectors, none of the 27 developer switches (their names are compiled out of a release build). Also in CI on the universal binary every package carries. | 2.5.1 private API; owner rule "no developer features in release". |
| Info.plist | `CFBundleShortVersionString` = Cargo version; `CFBundleVersion` = `git rev-list --count HEAD` (monotonic); `LSMinimumSystemVersion` 12.0 (platform passkeys); `ITSAppUsesNonExemptEncryption` false; `CFBundleSupportedPlatforms`, `DT*` toolchain keys. `NSAppTransportSecurity` (web content + local networking, as on iOS), the Bluetooth and camera purpose strings, the `velawallet://` scheme and the copyright line come from `packaging/macos/Info.plist.in`. | — |
| Privacy manifest | `packaging/macos/PrivacyInfo.xcprivacy` → `Contents/Resources/`. | §5 |
| Migration | `packaging/macos/container-migration.plist` (store build only). | §3 |
| Sign | `embedded.provisionprofile`, `xattr -cr`, `codesign --options runtime --entitlements packaging/macos/entitlements-mas.plist` (not `--deep`), then the signed entitlements are compared with the file. | §3 |
| Package | `productbuild --component … /Applications --sign <installer>`; `pkgutil --check-signature`. | — |
| Validate | `xcrun altool --validate-app -f <pkg> -t macos` with the API key (or `VELA_ASC_APPLE_ID` + `VELA_ASC_PASSWORD_ITEM`). `--no-validate` stops before. | Never uploads. |
| `--dev` | Same bundle, development-signed with the *Mac Team Provisioning Profile* that lists this Mac, no migration file, no package: the sandboxed build a developer can run (§10). | — |

The vendored `app-desktop/vendor/gpui_macos` (upstream at the pinned commit
minus the private API: `CGSSetWindowBackgroundBlurRadius`,
`_windowResize…Cursor`, `_opaqueRectForWindowMoveWhenInTitlebar`) is what
makes the binary pass. Its `Cargo.toml` says how to refresh it.

## 3. The sandbox

**Entitlements** (`entitlements-mas.plist`, nothing more): `app-sandbox`,
`application-identifier`/`team-identifier`, `associated-domains
webcredentials:getvela.app`, `network.client`, `files.user-selected.read-write`,
`device.camera`, `device.bluetooth`, `device.usb`, `smartcard`.
Not requested: `network.server` (the app listens on nothing on macOS),
Downloads/Pictures folder access (every file goes through a panel), JIT.

**Where the wallet lives.** The app keeps one file,
`<config dir>/VelaWallet/wallet.json` (`executor/storage.rs`). The Developer ID
copy keeps it at `~/Library/Application Support/VelaWallet`; sandboxed, the same
code resolves it inside `~/Library/Containers/app.getvela.VelaWallet/Data/…`.

**Testers moving from the .dmg.** `container-migration.plist` moves
`~/Library/Application Support/VelaWallet` into the container on the App Store
copy's **first** launch (when macOS creates the container; never again).
Measured on 2026-10-02 with a throwaway sandboxed bundle: the folder is
**moved**, nothing is left behind. So on a Mac that had the .dmg version:
- the App Store copy opens signed in, with the wallet list, contacts and
  settings;
- the .dmg copy then starts signed out — signing in with the same key brings
  the wallet back there (C-sync-1), but not the contacts and settings, which
  moved.
Say so in TestFlight's *What to Test* (§7). Deleting the .dmg copy is the
simple advice.

## 4. Export compliance — `ITSAppUsesNonExemptEncryption = false`

Owner decision 2026-10-02: false — encryption only for standard HTTPS and for
authentication. What the Mac binary encrypts:
- **TLS** for every request the wallet makes: `ureq` and the caBLE WebSocket
  (`tungstenite`) over **rustls 0.23 with the `ring` provider** and compiled-in
  web PKI roots (`Cargo.toml`; no OpenSSL in the macOS binary). WKWebView's
  pages use the system's TLS. Standard protocols, standard algorithms.
- **The system's passkey cryptography** (AuthenticationServices).
- In the Rust core, **AES only inside two authentication protocols**: the
  CTAP2 PIN/UV protocol that protects a security key's PIN (AES-256-CBC,
  `rust/crates/vela-core/src/ctap/pin_uv.rs`) and the caBLE/hybrid passkey
  tunnel to a phone (AES / AES-256-GCM in the Noise channel,
  `src/cable/`).
- Nothing encrypts user data for confidentiality: `wallet.json` holds public
  keys, addresses and preferences, no secret.

Same reasoning as the iPhone's (privacy-and-review.md, "Export compliance"):
BIS Note 1 to 5A002.a excludes cryptography limited to authentication and the
key management that supports it, and TLS is a standard, exempt use. It is a
legal attestation; take counsel's view before production, as for iOS.

## 5. Privacy (shared label, Mac manifest)

Universal Purchase means **one App Privacy answer** for iPhone and Mac. The Mac
app talks to the same services in the same way (relay, public-key index, chain
data, rates, the feedback proxy), so the label does not change; the desktop
evidence is in [privacy-evidence.md §9](privacy-evidence.md#9-desktop--the-mac-app-store-build-spec-095).
`PrivacyInfo.xcprivacy` declares the iPhone's five collected types and the
required-reason APIs **measured** on the Mac binary (`nm -u`, strings):
FileTimestamp (C617.1 container files, 3B52.1 files the person picked),
SystemBootTime 35F9.1, UserDefaults CA92.1. No DiskSpace, no ActiveKeyboards.
Settings → About now links the privacy policy, the terms and support on every
shell (5.1.1(i)).

## 6. Mac listing (App Store Connect → the macOS version)

Name, subtitle, privacy URL, category, price and age rating are the record's
(shared). Per platform: description, keywords, promotional text, screenshots,
What's New. Counts measured with Python `len` (keywords: UTF-8 bytes).
Rules kept from store-listing-copy.md: no "audited", no "beta/alpha", no price
or ROI talk, fees = "shown before you sign" (C-fee-1), safety stated
positively, no other platform's name in App Store metadata (2.3.10).

**Promotional Text** (≤170) — 162:
> An Ethereum wallet you actually own, on your Mac. No seed phrase: sign with Touch ID, a phone or tablet, or a security key, and see the exact fee before you sign.

**Keywords** (≤100 bytes) — 96:
```
crypto,ethereum,web3,self-custody,passkey,seedless,defi,smart account,erc-4337,base,arbitrum,evm
```

**Description** (≤4000) — 3,732:
```
Vela is a self-custody wallet for Ethereum and other EVM networks, with no seed phrase to write down. Your wallet is controlled by keys you choose, and each signature is unlocked by that key's own check: Touch ID or your Mac's login password, your phone's Face ID or fingerprint, or a touch and PIN on a security key.

SIGNING STAYS ON YOUR DEVICE
Signing is done on your device. Your passkey's private key never goes to Vela. Vela holds no key and no role on your wallet, so it can't move or freeze your funds by itself.

YOUR KEYS, YOUR CHOICE
When you create a wallet, choose up to seven keys. Any one of them can sign on its own:
• a passkey on this Mac, kept by iCloud Keychain or another passkey provider you choose, and synced to your other devices if that provider syncs it;
• a phone or tablet nearby, which scans a code on your screen (Bluetooth confirms it is close by);
• a USB security key.
Or choose Vela's Trusted Signer page as the way you sign: it opens in your browser, decodes each request and works out what to sign by itself, then asks your passkey.
The set is fixed when the wallet is created. Keys can't be added, removed or replaced later, so add a spare before you finish.

NEW MAC? SIGN IN WITH ANY KEY
Your wallet list stays on each device, and Vela doesn't sync or back up your passkeys. On another device, sign in with any one of your wallet's keys: Vela finds the wallet in its public key registry and rebuilds it.

Creating a wallet writes each key's public key, the wallet's name and key labels, and its address to a public registry on Gnosis Chain. That record is permanent and readable by anyone, so choose a name you're happy to make public.

ONE ADDRESS, 24 NETWORKS
Your address comes from all the keys you create the wallet with, and it's the same on every network: Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain, Arc, X Layer, Stable, Soneium, MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume and XRPL EVM, all mainnets. You can add other networks that meet its requirements.

A STANDARD SMART ACCOUNT
Every Vela wallet is an unmodified Safe v1.4.1 smart account, run through ERC-4337 (EntryPoint v0.7). No contract in the funds path was written by Vela.

WHAT YOU SEE IS WHAT YOU SIGN
The confirm screen is decoded from the exact transaction you sign, using ERC-7730 descriptors, token standards and a public function-signature database. Anything Vela can't decode carries a clear blind-signing warning. Unlimited token approvals are shown in red, and an on-chain approval can be capped before you sign. Requests that would hand control of your wallet to someone else are refused.

THE EXACT FEE, BEFORE YOU SIGN
The exact fee is shown before you sign, and it is part of what you sign. Pay it from your wallet in the network's coin or a supported USD stablecoin, with no deposit and no gas account. Fees go to the relay your wallet is set to: Vela's by default, or one you run yourself.

BUILT FOR THE DESK
Send to many people in one transaction, typed in or imported from a spreadsheet. Keep an address book with groups, and import or export it as a file. Open web3 apps in tabs in Vela's built-in browser: sites connect through an injected wallet provider, and every request is decoded and shown to you before you approve it with your key.

OPEN SOURCE, RUN IT YOURSELF
Everything is MIT-licensed: the app, the relay, the public-key index, the exchange-rate service and the chain-data directory. In Settings you can point the app at your own relay, index, data services and RPC nodes.

No seed phrase. No email or phone number. No ads, and no analytics or tracking SDKs in the app.
A wallet that does less, on purpose.
```
Claims: C-keys-1/2, C-auth-1, C-sign-1, C-custody-1, C-sync-1, C-reg-1, C-net-1,
C-addr-1, C-acct-1, C-clear-1/2, C-approve-1, C-selfcall-1, C-fee-1/2,
C-relay-1, C-dapp-1, C-signpage-1, C-lic-1, C-selfhost-2. Desktop facts: batch
send with spreadsheet import (`calamine`, `executor/batch.rs`), contacts groups
and CSV/JSON import/export (`contacts_io.rs`), browser tabs, Trusted Signer in
the default browser with the `velawallet://` callback (Info.plist.in).

## 7. TestFlight → Test Information (macOS)

- **Sign-in required:** off. **Feedback email:** hello@mondaylabs.ltd.
- **Beta App Description** (388):
  ```
  Vela Wallet for Mac: a self-custody wallet for Ethereum and other EVM networks with no seed phrase. Sign with a passkey on this Mac (Touch ID or your login password, through iCloud Keychain), a phone or tablet nearby, or a USB security key. This is the same app as Vela Wallet for iPhone; your wallet list stays on each device, and you sign in on each one with any of your wallet's keys.
  ```
- **What to Test** (799):
  ```
  Thank you for testing Vela on the Mac.
  • Create a wallet with "This device" (Touch ID or your login password), or sign in to your existing wallet with any of its keys.
  • Receive: copy the address, save the QR card (it should offer your Downloads folder).
  • Send a small amount, alone or to several people at once; check the fee shown before you sign.
  • Explore: open a dApp, connect, sign. Links that open a new window should open a new tab.
  • Contacts: add, group, import and export.
  • Settings > About: the privacy policy, terms and support links.
  Already using the downloaded (.dmg) Vela on this Mac? The App Store version takes over its wallet list on first launch; the downloaded copy then starts signed out (sign in again with the same key).
  Report anything odd from Settings > Send feedback.
  ```
- **Review notes** (external testing): the §8 block.

## 8. App Review notes — Mac (App Review Information → Notes)

3,873 characters, ASCII; the 29-character video placeholder may grow to 156.

```
Vela Wallet for Mac is the same app as Vela Wallet for iPhone (Universal Purchase): a non-custodial wallet for Ethereum and other EVM networks, published by MONDAY LABS LTD (organization account). Each wallet is an unmodified Safe smart account, signed for by the user's own passkeys.

NO DEMO ACCOUNT: there is no username, password or server login, and a passkey cannot be shared. Please create a wallet on the review Mac (about 2 minutes). The Mac needs macOS 12 or later, a login password, and iCloud Keychain turned on (System Settings > your Apple Account > iCloud > Passwords & Keychain). Touch ID is optional; the login password works too.
1. Open Vela, choose "Create Wallet".
2. Type any name, tick the three boxes, choose "Create Wallet".
3. On "Add passkeys", choose "This device". Approve the system passkey sheet with Touch ID or the login password, then approve the second prompt, which confirms the key.
4. Choose "Create Wallet", then "Enter Wallet".
No iCloud Keychain? In step 3 choose "Phone or tablet" and scan the code with a phone that holds passkeys (Bluetooth on), or "USB security key" with a FIDO2 key.

Then: Receive shows the address and QR code. Explore is the built-in browser: type a site address and use the site's Connect button. Send needs funds on a network; the video shows it, and on request we will send a small amount to the review wallet.
DEMO VIDEO: <owner to add unlisted link>

Creating a wallet writes a permanent, pseudonymous record to a public registry contract on Gnosis Chain (each key's public key and credential ID, the wallet name and key labels, the address). It cannot move funds and nobody, including us, can delete it. The app says so before the wallet is created.

FEATURES YOU WILL MEET
- Built-in dApp browser (WKWebView, tabs) with an injected EIP-1193 provider, offered to https pages only. Every connection and signature needs the user's approval and a passkey check. Mail and phone links open the system's apps.
- Camera: scanning QR codes. Without camera access, "From photos" reads a code from a picture the user picks.
- Bluetooth: used only for "Phone or tablet". This Mac shows a QR code; the phone that scans it advertises over Bluetooth, and Vela scans for it and connects to finish the standard passkey (caBLE/hybrid) exchange.
- USB: a FIDO2 security key over USB (HID or smart card).
- Files, only through the system's open and save panels: import/export the address book (CSV or JSON), import a recipient list for a batch send, save the receive QR card, attach screenshots to feedback.
- Trusted Signer (optional sign-in method): opens sign.getvela.app in the default browser; the answer returns through the velawallet:// link.
- Feedback (Settings > Send feedback), optional: the typed text, a few device details and up to five screenshots, re-encoded on the Mac so no metadata leaves it. It becomes a public GitHub issue, which the sheet says before sending.

GUIDELINE 3.1.5(i): Vela is a wallet offered by an organization. It runs no exchange, has no swap or trade feature of its own, no fiat on-ramp, no in-app purchase and no mining. Sites opened in the browser are third-party websites the user navigates to.

ENCRYPTION: ITSAppUsesNonExemptEncryption = NO. HTTPS/TLS (rustls), the system's passkey cryptography, and AES used only inside authentication protocols (a security key's PIN protocol and the caBLE passkey tunnel).

DELETION: Settings > Erase This Device deletes everything the app stored on this Mac. Our services keep copies for a limited time (relay up to 14 days, key index up to 30 days); requests to delete them sooner: hello@mondaylabs.ltd or https://getvela.app/delete.

PRIVACY: https://getvela.app/privacy (also in Settings > About). No analytics SDK, crash reporter or advertising identifier.
Support: https://getvela.app/support
Contact for review: hello@mondaylabs.ltd
```

## 9. Screenshots

Mac App Store: 16:10, one of 1280×800, 1440×900, 2560×1600, 2880×1800; 1–10;
no alpha. **2560×1600 from the app's 1280×800-point window on a Retina
display**, which is the window's own size, so no scaling:

1. Run the store build (or `cargo run` for a developer build), signed in to a
   real wallet with some history, light appearance, English.
2. Window id: `CGWindowListCopyWindowInfo` (any small Swift/Python helper),
   the layer-0 window of the app's pid that is 1280×800.
3. `screencapture -x -o -l <window id> shot.png` (`-o`: no shadow).
4. Flatten the rounded corners' transparency onto the window background:
   `python3 -c "from PIL import Image; i=Image.open('shot.png').convert('RGBA'); b=Image.new('RGBA', i.size, (248,248,246,255)); b.alpha_composite(i); b.convert('RGB').save('mas-1.png')"`.
   Check: `2560×1600`, mode RGB.
Suggested five: wallet home; Receive with the QR card; the send confirm with
the decoded call and the fee; Explore with a dApp connected; Contacts with
groups. Drafts from the design fixtures (no wallet behind them, and some
fixture names are Chinese) were made for spec 095's results; use real screens
for the store.

## 10. Sandbox smoke results (2026-10-02, development-signed, this Mac)

`build-macos-mas.sh --dev` bundle in `/Applications`, launched through
LaunchServices; a second copy built with `dev-fixtures` (same entitlements and
signature) for what needs a signed-in wallet (parallel space) and for the
developer probe (`VELA_SANDBOX_PROBE=1`).

| Area | Result |
|---|---|
| Launch, window, menus | Works. Menu bar: About, Edit (Undo, Redo, Cut, Copy, Paste, Select All), View, Window, localized. |
| State across relaunch | Works: the intro is not shown again; the state file is `~/Library/Containers/app.getvela.VelaWallet/Data/Library/Application Support/VelaWallet/wallet.json`, write+read OK. |
| Passkey "This device" | The system sheet appears with the getvela.app passkeys (Associated Domains honoured under the sandbox). Cancelled; no ceremony completed. |
| Bluetooth | The system prompt appears with the purpose string when "Phone or tablet" starts scanning. Not answered. |
| Camera | The system prompt appears with the purpose string; declined → the scanner says camera access is needed and offers "From photos". |
| Open panel | Works (Powerbox; the person's folders). |
| Save panel | Works and writes the file; opens in Downloads (it ignored a directory outside the container and opened in Documents — fixed: the container's `Downloads` link is passed). |
| USB HID | Enumeration works: 23 HID devices listed. |
| System proxy | CFNetwork's per-URL proxy read works (this Mac: HTTP CONNECT 127.0.0.1:1088, SOCKS 127.0.0.1:1080). |
| Network | HTTPS from the app: 200; balances, RPC, relay and index load. |
| Browser | https loads; LAN http loads with no wallet (`isSecureContext` false, no `window.ethereum`); `target=_blank` and `window.open` open new tabs; a timer's `window.open` is blocked; mailto:/tel: reach the opener; Edit → Select All reaches the page. |
| Denied by the sandbox | Nothing the app uses. |

Left on this Mac by the test: nothing — the test bundle, its container and the
throwaway migration probe were removed; camera and Bluetooth permission rows
for `app.getvela.VelaWallet` were reset with `tccutil`. **The .dmg app's
earlier Bluetooth permission was reset with them: it asks again on its next
phone sign-in.**

## 11. Known residue and follow-ups

- **wry's WebKit preference keys.** The binary carries five key-value-coding
  strings from wry 0.56: `allowsPictureInPictureMediaPlayback` and
  `tabFocusesLinks` (set on every web view), `fullScreenEnabled` (only on
  macOS 12.0–12.2), `drawsBackground` and `inactiveSchedulingPolicy` (only for
  options the wallet never sets). They are strings, not selectors or
  imported symbols — what `check-store-binary.sh` and App Store analysis look
  for — and Mac apps built on wry ship on the store with them. Noted, not
  changed; if review ever names one, a wry patch is the place.
- **CoreLocation is linked** (through `objc2-user-notifications`, a gpui
  dependency) but no CoreLocation API is called; no location purpose string is
  needed.
- **Languages.** Both bundles declare the 15 corpus locales in
  `CFBundleLocalizations` (owner, 2026-10-02), as the core names them for Apple
  (`vela_core::i18n::apple_localizations`: `zh` → `zh-Hans`, `zh-TW` →
  `zh-Hant`, `zh-HK`, `es-MX`, `pt-BR`, …), each held to the core by a test
  (desktop `loc.rs`, iOS `LocaleMappingTests`). The system's own UI (panels,
  share sheets, the menu items AppKit adds) follows the person's language and
  the store page lists all 15. The wallet's strings are unchanged: they come
  from the corpus, in the language picked in Settings or followed from the
  system. One visible addition on iPhone: iOS shows a per-app *Language* row
  in Settings → Vela; choosing one there is what "follow the system" then
  follows.
- **Price (owner, 2026-10-02: keep).** Universal Purchase gives the Mac the
  iOS price (one-time purchase); the Developer ID .dmg stays a free download
  (C-plat-1). Decided: both stay as they are.
