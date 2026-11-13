# VELA-2026-003: getvela.app's app-association files let development and demo apps request users' passkeys

| GHSA form field | Value |
| --- | --- |
| Title | getvela.app Digital Asset Links and AASA authorised non-release app identities for getvela.app passkeys |
| Ecosystem / package | Other / `getvela.app` website (`app-web/getvela.app`) |
| Affected | The production `getvela.app` site until 2026-11-07. The entries were in the site source from 2026-06-03 (see the table). |
| Patched | Site commit `0562c17bb`, deployed to getvela.app 2026-11-07 (the same change on the development branch: `1570bbeb9`) |
| Severity | Medium (Vela's assessment); CVSS 3.1 base 5.3 |
| CVSS vector | `CVSS:3.1/AV:N/AC:H/PR:N/UI:R/S:U/C:N/I:H/A:N` |
| Weakness | CWE-863: Incorrect Authorization |
| Credit | Found by Vela's internal security review, 2026-11-07 |
| Status | Fixed and deployed. No known exploitation. Advisory not yet published. |

## Summary

Vela's passkeys are created for the relying party `getvela.app`. Android lets an app use a
site's passkeys when the site's `/.well-known/assetlinks.json` grants that app's package name and
signing certificate `delegate_permission/common.get_login_creds`. iOS does the same for apps
listed under `webcredentials` in `/.well-known/apple-app-site-association`.

Besides the store app on its release key, the production files authorised these identities:

| Identity | Granted | In the site source since |
| --- | --- | --- |
| `app.getvela.wallet` signed with a developer machine's **debug keystore** (`24:EA:D0:02:…`) | passkeys and links | 2026-06-03 (site created, `4d6160a06`) |
| `app.getvela.wallet` signed with an **EAS-managed key** held by Expo for the retired Expo app (`BE:1E:CA:F3:…`) | passkeys and links | 2026-07-03 (`daac6c22d`) |
| `app.getvela.app_passkeys_demo` (a passkey demo app) signed with the debug keystore | passkeys and links | 2026-08-13 (`997c0575e`) |
| `F9W689P9NE.app.aikeys.apppasskeysdemo-ios` (a demo app, under Vela's own Apple team) | passkeys | 2026-08-14 (`3c4a9370c`) |
| `app.getvela.wallet.devicetest` signed with the debug keystore | passkeys | 2026-11-01 (`3721c522c`) |

The dates are when each entry entered the repository's site source. We have not reconstructed
every production deploy, so the time each one was served may be shorter.

## Impact

An Android app signed with either of the two non-release keys could ask Credential Manager for
an assertion from any user's `getvela.app` passkey. That includes the passkeys of web and
extension users, if those passkeys sync to Google Password Manager. The app could choose the
challenge, for example the hash of a SafeOp that empties the account. The system prompt names
`getvela.app` and does not show the transaction. One approved prompt would give the attacker a
valid signature, and with it control of the account.

To do this, an attacker needed **all** of the following:

1. **One of the two keys.**
   - The debug keystore is a single developer machine's `~/.android/debug.keystore`. It is not
     public, it was never committed to this repository (we checked the history for keystore
     files), and the CI workflows do not use it.
   - The EAS key is held by Expo for Vela's Expo account.

   We have no indication that either key was ever exposed. The iOS demo entry could only be used
   by an app signed by Vela's own Apple developer team.
2. **A user who installed the attacker's APK.** It would be side-loaded, because a store would
   not carry an app under these identities. Note that an APK with the package
   `app.getvela.wallet` and a different key cannot install over the real app, but
   `app.getvela.app_passkeys_demo` and `app.getvela.wallet.devicetest` can install alongside it.
3. **A user who approved that app's passkey prompt** with their biometric or PIN.

**Was any user exposed?** In the sense that matters, we believe not. Exploitation needed a key
we have no reason to think left its holder, plus the user's own install and approval. We know of
no exploitation. We could not detect exploitation
independently, though: an assertion is made on the user's device, and the transaction it signs
looks like any other on-chain. The protection rested on two private keys staying private, one
of them a developer debug key, which is usually protected only by Android's widely known default
password. It did not rest on the association. That is why we treat this as a vulnerability.

**Why Medium.** The impact, had a key leaked, was complete control of any Android user's
account (I:H). The likelihood was low (AC:H): it needed a private key and a side-loaded app.

## Fix

Site commit **`0562c17bb926829a5d68c97659dff2eb7d658da7`**, deployed to `getvela.app` on
2026-11-07. The same change on the development branch (`feat/multisig-account`) is
**`1570bbeb99a6bccdb0860e653969a39896169e1f`** ("site, apps: getvela.app passkeys and links only
for the store apps on release keys"). After it:

- `getvela.app/.well-known/assetlinks.json` lists one statement: `app.getvela.wallet` on the
  release certificate `A3:8E:36:FE:…:9D:53`.
- `getvela.app/.well-known/apple-app-site-association` lists one `webcredentials` app:
  `F9W689P9NE.app.getvela.VelaWallet`.
- Tests pin both files exactly (`assetlinks.test.ts`, `association.test.ts`). Since the final
  review (2026-11-12) they run in CI's `site` job on every pull request and before every
  `bun run deploy` of the site, so adding an identity fails CI and stops a deploy; a red `site`
  job stops a merge only if branch protection requires it. Before that change the site's tests
  ran only when run by hand. The `wallet.getvela.app` links file (links only, no passkeys) was
  reduced to the release key too.
- Moving test builds to a separate test RP ID, instead of `getvela.app`, is in progress.

Google's and Apple's verifiers cache these files on their own schedules. A device may have kept
the old grants for some time after the deploy. We have not measured how long.

## Workarounds

None are needed now. As general practice: install Vela only from the App Store, Google Play or
the project's GitHub Releases, and do not approve a `getvela.app` passkey prompt that an app
other than Vela or a browser on a getvela.app page raised.

## Timeline

| Date (UTC+8) | Event |
| --- | --- |
| 2026-06-03 to 2026-11-01 | Entries added, as in the table above |
| 2026-11-07 | Internal security review finds the issue (P0) |
| 2026-11-07 | `0562c17bb` deployed to getvela.app; `1570bbeb9` on the development branch |
| 2026-11-08 | Advisory drafted |
| *pending* | Advisory published on GitHub |
