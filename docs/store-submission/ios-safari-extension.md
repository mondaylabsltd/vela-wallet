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
   provider). The app's Settings has a "Turn on the Safari extension" row
   (`SafariExtensionRow.swift`, `settings.safariExtension.*` in 15 locales):
   where Safari keeps the switch, and Open Settings — iOS has no API to read
   whether it is on, nor a link into Safari's own settings.

## Go-live checklist (production only)

- [ ] **Deploy** `app-web/trusted-signer/dist/` to `sign.getvela.app`: the
      newest page build, `64363925…` (§16: the multisig owner proof's card),
      is the front of vela-core `trusted_signer::integrity::BUILD_ALLOWED`;
      the Safari relay and attestation rule have been in every build since
      `879e715f…`. The deployed `index.json` lists none of them yet — until
      it does, the deployed page refuses the Safari extension (no relay, no
      attestation rule), and since every Safari signature and ceremony goes to
      the page (below), the Safari wallet can neither create nor sign. The
      wallet opens the first `BUILD_ALLOWED` build the index lists, so deploy
      `64363925…` for the Safari wallet to create multisigs.
      Check: `curl -I https://sign.getvela.app/b/64363925…/sign` is 200 +
      immutable + `access-control-allow-origin: *`.
- [x] **Core:** a Safari extension's signatures AND key ceremonies go to the
      page (spec 102 R8, `signing_venue::SigningSurface::SafariExtension`):
      `signing_route_on` answers `page` for every kind — the account's page,
      or the official page for a `getvela.app` account that signs in Vela —
      and the create / sign-in machines, told `surface_known`, name a page on
      every ceremony (`ceremony_page_on`). Every other surface is unchanged.
      Shared vectors: `rust/crates/vela-core/tests/vectors/signing-route/`.
- [x] **Web:** the Safari package's onboarding (create, add a key, confirm,
      sign in, a recovery's live proof) runs each ceremony on the page through
      the existing relay (`src/lib/onboarding/core/page-ceremony.ts`), and a
      passkey Safari refuses is said as that (`onboarding.common.
      safariPasskeyBody`), never as "site access withheld". Proven on an iOS
      26 simulator with a real simulator passkey against a loopback page
      (`SafariRouteDeviceTests`); not yet against the deployed official page.
- [x] **Multisig creation on the page** (`docs/multisig-machines.md` §16): a
      multisig's owner proof (`vela_multisigCreate`) has its card on page
      `64363925…` — the multisig's genesis handed over, its address, members,
      this signer's place, "Needs N of M" and the challenge read there by the
      core's rule (`trusted_signer::owner_proof`) — so the Safari wallet
      creates and shares multisigs through the page like every other
      signature. Proven on the published bytes in Chrome through the Safari
      relay (`answer-test.mjs` S); not yet on a simulator's Safari, nor
      against the deployed official page.
- [ ] Archive, sign and submit as usual; the store-binary gate runs on the
      archive.
