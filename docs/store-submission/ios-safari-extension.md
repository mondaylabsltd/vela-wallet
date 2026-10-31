# iOS — the Safari Web Extension (native AUDIT §6.5)

The iOS app embeds `VelaSafariExtension.appex`
(`app-ios/VelaWallet/VelaSafariExtension`): the very web extension the Chrome
Web Store ships, built for Safari (`pnpm build:extension:safari` in
`app-web/vela-wallet`, release package `extension/dist-safari-release`, copied
in by `app-ios/scripts/bundle-safari-extension.sh`). iPhone Safari users get
what desktop Chrome users get: `window.ethereum` / EIP-6963 on every site,
connect, signing in the wallet's own sheet, and the trusted signing page.

Everything below was measured on the iOS 26.2 simulator; nothing here waits on
the owner except the production steps at the end.

## What the build needs (already in the tree)

- **Bundle id** `app.getvela.VelaWallet.SafariExtension` (team F9W689P9NE),
  product type app-extension, extension point `com.apple.Safari.web-extension`.
- **Entitlements: none.** No App Group (nothing is shared with the app — the
  wallet in Safari keeps its own state, per extension, and a key never leaves
  its passkey), no keychain group, no associated domain. The native handler
  (`SafariWebExtensionHandler`) refuses every message; the manifest does not ask
  for `nativeMessaging`. `check-ios-store-binary.sh` fails an archive whose
  extension is entitled to anything, links the core, calls the keychain,
  biometry or a passkey API, or carries the development package.
- **Privacy manifest** `VelaSafariExtension/PrivacyInfo.xcprivacy`: tracking
  false; the same collected types as the app (Other Financial Info and User ID,
  linked, app functionality — the relay and the public-key index; Diagnostic
  Data, Customer Support, Photos, not linked — the optional bug report). **No
  usage analytics** — the Safari package's pages are `safari-web-extension:`
  and the analytics gate answers `chrome-extension:` only. Required-reason APIs:
  none (the binary is one empty class; measured with `nm -u`).
- **Size:** the package is ~57 MB uncompressed (15 prerendered locales + the
  core wasm); the IPA grows by roughly a quarter of that compressed.
- **Release fails closed:** a Release build without the package, with the
  development package, or with a package version other than
  `MARKETING_VERSION` stops (`bundle-safari-extension.sh`); CI builds the
  package before the archive (`ios-package.yml`).

## App Store Connect (at go-live)

1. The extension rides the app's existing record: no separate listing. Its
   bundle id needs an App ID under the team (Xcode's automatic signing creates
   it on the first archive; no capability to enable).
2. **Privacy "nutrition label":** unchanged from the app's — the extension
   collects nothing the app does not, and adds no tracking or analytics.
3. **App Review notes for a wallet with a Safari extension** (paste):
   > Vela Wallet is a self-custodial smart-account wallet. It includes a Safari
   > Web Extension that lets websites (dApps) ask the wallet to connect and to
   > sign, the way desktop wallet extensions do. To try it: Settings → Apps →
   > Safari → Extensions → Vela Wallet → Allow Extension, then allow it on a
   > website (or All Websites), open a dApp such as https://app.uniswap.org and
   > choose Vela Wallet. The extension never holds a private key: keys are
   > passkeys in the iCloud Keychain, and every signature is confirmed by the
   > person with Face ID on Vela's signing page (sign.getvela.app), which shows
   > what is being signed. The extension has no native messaging and shares no
   > data with the app. No account, email or personal data is required.
   Add the demo instructions the app's own review notes already carry
   (`privacy-and-review.md`) — the extension uses the same wallet.
4. **Guideline 3.1.1 / 2.5.2:** the extension downloads and runs no code: the
   whole wallet (including the wasm core) is in the bundle, the CSP is
   `script-src 'self' 'wasm-unsafe-eval'`, and no page loads a remote script
   (`safari-package.test.ts`).
5. **Guideline 4.4.2 (Safari extensions):** it works on the current Safari and
   asks only for what it uses (`storage`, `tabs`, website access for the
   provider). Optional polish for review: a Settings row in the app telling
   people how to turn the extension on (iOS has no API to read whether it is
   on; this needs new corpus keys in 15 locales — not in this round).

## Go-live checklist (production only)

- [ ] **Core:** add signing page build `0d66b844…` to vela-core
      `trusted_signer::integrity::BUILD_ALLOWED` (front), regenerate, and deploy
      `app-web/trusted-signer/dist/` to `sign.getvela.app` — until then the
      deployed page refuses the Safari extension (it has no relay and no
      attestation rule), and the Safari wallet can sign only with what it can
      reach in the extension (nothing, for a real passkey: see below).
- [ ] **Core:** route a Safari extension's signatures AND key ceremonies to the
      page. WebKit refuses WebAuthn in an extension page (`SecurityError`,
      RPID), so the in-Vela venue cannot work in Safari for a real passkey:
      `signing_route` must answer `page` for every kind (create, sign in,
      proof, sign) when the surface is the Safari extension — today it answers
      `in_vela` for a `getvela.app` account by default and for every key
      ceremony (R3). The page already answers the attested extension for
      ceremonies (`walletRequester`).
- [ ] **Web:** the onboarding / sign-in flows in the Safari package must send
      their ceremonies to the page (follows from the core route; today they
      call WebAuthn and Safari refuses with `SecurityError`, which the wallet
      words as "site access withheld" — wrong for Safari).
- [ ] Archive, sign and submit as usual; the store-binary gate runs on the
      archive.
